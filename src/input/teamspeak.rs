//! [TeamSpeak] audio capture.
//!
//! [TeamSpeak]: https://teamspeak.com

use std::{
    collections::HashMap,
    fmt,
    future::Future,
    hint::unreachable_unchecked,
    mem,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use audiopus::coder::{Decoder as OpusDecoder, GenericCtl as _};
use byteorder::{BigEndian, ByteOrder as _};
use derive_more::{Display, Error};
use futures::{
    compat::{Compat01As03, Future01CompatExt as _, Stream01CompatExt as _},
    ready,
    stream::{BoxStream, StreamExt as _},
    Stream,
};
use slog_scope as log;
use tokio::{
    io::{self, AsyncRead},
    time::{delay_for, Delay},
};
use tsclientlib::{
    ConnectOptions, Connection, Error as TsClientError, PHBox, PacketHandler,
    ServerAddress,
};
use tsproto_packets::packets::{AudioData, CodecType, InAudio, InCommand};

/// Helper alias for declaring [`Box`]ed [`futures_01::Future`]s, which are
/// [`Send`].
type BoxFuture01<I, E> =
    Box<dyn futures_01::Future<Item = I, Error = E> + Send>;

/// Helper alias for declaring [`Box`]ed [`futures_01::Stream`]s, which are
/// [`Send`].
type BoxStream01<I, E> =
    Box<dyn futures_01::Stream<Item = I, Error = E> + Send>;

/// Helper alias for [`Future`] of establishing [`Connection`].
type ConnectionFuture = Compat01As03<BoxFuture01<Connection, TsClientError>>;

/// Helper alias for [`Stream`] of [`InCommand`] packets,
type InCommandsStream = BoxStream<'static, Result<InCommand, tsproto::Error>>;

/// Helper alias for [`Stream`] of [`InAudio`] packets,
type InAudioStream = BoxStream<'static, Result<InAudio, tsproto::Error>>;

/// Configuration for creating connections to [TeamSpeak] server.
///
/// [TeamSpeak]: https://teamspeak.com
#[derive(Clone, Debug)]
pub struct Config {
    /// Address of [TeamSpeak] server.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    pub server_addr: ServerAddress,

    /// Channel to join on [TeamSpeak] server.
    ///
    /// Sub-channels can be specified via `<parent>/<sub>` scheme.
    ///
    /// If [`None`] then default server channel is chosen to join.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    pub channel: Option<String>,

    /// Name to represent established connections on [TeamSpeak] server with.
    ///
    /// If [`None`] then `TeamSpeakBot` is used.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    pub name_as: Option<String>,
}

impl From<Config> for ConnectOptions {
    fn from(cfg: Config) -> Self {
        use slog::Drain as _;

        let mut out = Self::new(cfg.server_addr)
            .name(cfg.name_as.unwrap_or_else(|| "TeamSpeakBot".into()));
        if let Some(v) = cfg.channel {
            out = out.channel(v);
        }

        let lgr = slog_scope::logger();
        let is_debug = lgr.is_debug_enabled();
        let is_trace = lgr.is_trace_enabled();
        out.logger(lgr).log_commands(is_debug).log_packets(is_trace)
    }
}

/// [`Input`] builder.
#[derive(Clone, Debug)]
pub struct InputBuilder {
    /// [`Config`] to build [`Input`] with.
    cfg: Config,
}

impl InputBuilder {
    /// Sets channel to join on [TeamSpeak] server and to capture audio from.
    ///
    /// Sub-channels can be specified via `<parent>/<sub>` scheme.
    ///
    /// If not given, then default server channel will be chosen to join.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    #[inline]
    pub fn channel<S: Into<String>>(mut self, name: S) -> Self {
        self.cfg.channel = Some(name.into());
        self
    }

    /// Sets name for representing established connections on [TeamSpeak]
    /// server.
    ///
    /// If not given, then `TeamSpeakBot` will be used.
    ///
    /// Beware, that [TeamSpeak] limits client names by
    /// [30 UTF-8 characters max][1]. If the provided `name` is longer, then it
    /// will be automatically truncated to fit into the requirement.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    /// [1]: https://tinyurl.com/y7z3nkpx
    #[inline]
    pub fn name_as<S: Into<String>>(mut self, name: S) -> Self {
        let mut name = name.into();
        name = name.chars().take(30).collect();
        self.cfg.name_as = Some(name);
        self
    }

    /// Builds out the configured [`Input`], ready for use.
    #[inline]
    #[must_use]
    pub fn build(self) -> Input {
        Input {
            cfg: self.cfg,
            state: State::Disconnected,
            backoff: Duration::from_millis(50),
        }
    }
}

/// Audio input captured from [TeamSpeak] server.
///
/// It produces [PCM 32-bit floating-point big-endian][1] encoded audio samples
/// (`f32be` format in [FFmpeg]'s [notation][2]).
///
/// In case of connection lost it automatically reconnects to the server with an
/// exponential backoff up to 1 minute.
///
/// [FFmpeg]: https://ffmpeg.org
/// [TeamSpeak]: https://teamspeak.com
/// [1]: https://wiki.multimedia.cx/index.php/PCM
/// [2]: https://trac.ffmpeg.org/wiki/audio%20types
#[derive(Debug)]
pub struct Input {
    /// Configuration for establishing connections with [TeamSpeak] server.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    cfg: Config,

    /// Current [`State`] of this [`Input`].
    state: State,

    /// Delay of the next backoff for reconnection.
    backoff: Duration,
}

impl Input {
    /// Maximum supported size of a decoded [Opus] audio frame received from
    /// [TeamSpeak] server.
    ///
    /// Use 48 kHz, maximum of 120 ms frames (3 times 40 ms frames of which
    /// there are 25 per second) and stereo data (2 channels).
    /// This is a maximum of 11520 samples and 45 kiB.
    ///
    /// [Opus]: https://opus-codec.org
    /// [TeamSpeak]: https://teamspeak.com
    const OPUS_MAX_FRAME_SIZE: usize = 48000 / 25 * 3 * 2;

    /// Usual size of a decoded [Opus] audio frame received from [TeamSpeak]
    /// server.
    ///
    /// Use 48 kHz, 20 ms frames (50 per second) and mono data (1 channel).
    /// This means 1920 samples and 7.5 kiB.
    ///
    /// [Opus]: https://opus-codec.org
    /// [TeamSpeak]: https://teamspeak.com
    const OPUS_USUAL_FRAME_SIZE: usize = 48000 / 50;

    /// Starts creation of the new [`Input`].
    #[allow(clippy::new_ret_no_self)]
    #[inline]
    pub fn new<A: Into<ServerAddress>>(server_address: A) -> InputBuilder {
        InputBuilder {
            cfg: Config {
                server_addr: server_address.into(),
                channel: None,
                name_as: None,
            },
        }
    }

    /// Increases backoff delay of this [`Input`] twice if it's not more than
    /// 1 minute.
    #[inline]
    fn increase_backoff_delay(&mut self) {
        if self.backoff < Duration::from_secs(60) {
            self.backoff *= 2;
        }
    }

    /// Resets backoff delay of this [`Input`] to the default value of
    /// 100 milliseconds.
    #[inline]
    fn reset_backoff_delay(&mut self) {
        self.backoff = Duration::from_millis(100);
    }
}

impl AsyncRead for Input {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        use State::*;

        match self.state {
            Disconnected => self.poll_disconnected(cx, buf),
            Connecting { .. } => self.poll_connecting(cx, buf),
            Connected { .. } => self.poll_read_connected(cx, buf),
            Backoff { .. } => self.poll_backoff(cx, buf),
        }
        .map(|res| res.map_err(Into::into))
    }
}

/// Possible states of audio [`Input`].
enum State {
    /// Not connected to the server and is ready to establish a [`Connection`].
    Disconnected,

    /// Connecting to the server and negotiating with it.
    Connecting {
        /// [`Future`] that will be resolved once [`Connection`] is established
        /// and ready.
        conn: ConnectionFuture,

        /// [`Stream`] of [`InCommand`] packets to be injected and handled for
        /// performing negotiation with server.
        commands: Arc<Mutex<Option<InCommandsStream>>>,

        /// [`Stream`] of [`InAudio`] packets to be injected and handled for
        /// preserving correct packets processing order.
        audio: Arc<Mutex<Option<InAudioStream>>>,
    },

    /// Connected to the server and receiving audio data.
    Connected {
        /// Established [`Connection`] with the server. Closes on drop.
        _conn: Connection,

        /// [`Stream`] of received [`InCommand`] packets.
        commands: InCommandsStream,

        /// [`Stream`] of received and decoded [`InAudio`] packets.
        audio: InAudioStream,

        /// Set of [`OpusDecoder`]s for each member on [TeamSpeak] channel.
        ///
        /// Because [Opus] decoding is a stateful process, a single
        /// [`OpusDecoder`] cannot process packets from multiple members
        /// simultaneously. That's why each member should have its own instance
        /// of [`OpusDecoder`].
        ///
        /// [`time::Instant`] is updated each time concrete [`OpusDecoder`] is
        /// used, so allows to cleanup outdated [`OpusDecoder`]s.
        ///
        /// [Opus]: https://opus-codec.org
        /// [TeamSpeak]: https://teamspeak.com
        decoders: OpusDecoders,

        /// Raw [PCM 32-bit floating-point][1] data decoded by [`OpusDecoder`].
        ///
        /// It's reused for decoding each received packet instead of allocating
        /// new memory each time.
        ///
        /// [1]: https://wiki.multimedia.cx/index.php/PCM
        data: Vec<f32>,

        /// Cursor that indicates how much from `data` was already read.
        ///
        /// Zero value indicates that all `data` was already read and it should
        /// be filled with new [Opus] frame.
        ///
        /// [Opus]: https://opus-codec.org
        cursor: usize,
    },

    /// Not connected to the server and is delayed before reconnecting.
    Backoff {
        /// [`Future`] that will be resolved once backoff delay is finished.
        delay: Delay,
    },
}

// TODO #4: Spawn `InCommandsStream` and `InAudioStream` on `Drop` to disconnect
//          normally.
//      https://github.com/tyranron/ephyr/issues/4

/// Type of [TeamSpeak] channel member ID.
///
/// [TeamSpeak]: https://teamspeak.com
type MemberId = u16;

/// Helper alias for [`OpusDecoder`]s collection used by [`State::Connected`],
/// with the last garbage collection time.
///
/// Garbage collection happens with [`Input::OPUS_DECODERS_GC_PERIOD`].
type OpusDecoders = HashMap<MemberId, OpusDecoder>;

impl Input {
    /// Polls [`Input`] in [`State::Disconnected`].
    fn poll_disconnected(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, InputError>> {
        debug_assert!(
            matches!(self.state, State::Disconnected),
            "Called poll_disconnected() for state {:?}",
            self.state,
        );

        let commands = Arc::new(Mutex::new(None));
        let audio = Arc::new(Mutex::new(None));

        let opts = ConnectOptions::from(self.cfg.clone()).handle_packets(
            Box::new(InPacketsInjector {
                commands: commands.clone(),
                audio: audio.clone(),
            }),
        );
        let conn = Connection::new(opts).compat();

        let new_state = State::Connecting {
            conn,
            commands,
            audio,
        };

        let _ = mem::replace(&mut self.state, new_state);
        log::debug!("Connecting to TeamSpeak server...");
        self.poll_connecting(cx, buf)
    }

    /// Polls [`Input`] in [`State::Connecting`].
    fn poll_connecting(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, InputError>> {
        let new_state = if let State::Connecting {
            conn,
            commands,
            audio,
        } = &mut self.state
        {
            match Pin::new(conn).poll(cx) {
                // If `Connection` is not ready yet, then perform negotiation
                // with the server, until it does.
                Poll::Pending => {
                    let ok = {
                        // TODO #5: Out-of-order errors still happens rarely.
                        //      https://github.com/tyranron/ephyr/issues/5

                        // When `State::Connecting` we still need to poll and
                        // process `InAudio` packets for preserving correct
                        // packets processing order inside `tsclientlib`
                        // implementation.
                        if let Some(aud) = audio.lock().unwrap().as_mut() {
                            poll_next_packet("InAudio", aud, cx)
                        } else {
                            // If `InPacketsInjector` hasn't injected
                            // `InAudioStream` yet, then it's OK at the moment.
                            true
                        }
                    };
                    if ok {
                        // We should poll `InCommand` packets to make them
                        // processed inside `tsclientlib` and perform the
                        // necessary negotiation with the server.
                        if let Some(cmds) = commands.lock().unwrap().as_mut() {
                            if poll_next_packet("InCommand", cmds, cx) {
                                return Poll::Pending;
                            }
                        } else {
                            // If `InPacketsInjector` hasn't injected
                            // `InCommandsStream`s yet, let's just wait until
                            // it does.
                            return Poll::Pending;
                        }
                    }
                    // If polling packets failed, then reconnect with backoff.
                    State::Backoff {
                        delay: delay_for(self.backoff),
                    }
                }

                // If `Connection` has been established successfully, then
                // transit to `State::Connected`.
                Poll::Ready(Ok(conn)) => State::Connected {
                    _conn: conn,
                    commands: {
                        commands.lock().unwrap().take().expect(
                            "InCommandsStream must be injected when Connected",
                        )
                    },
                    audio: {
                        audio.lock().unwrap().take().expect(
                            "InAudioStream must be injected when Connected",
                        )
                    },
                    decoders: OpusDecoders::new(),
                    data: vec![0_f32; Self::OPUS_USUAL_FRAME_SIZE],
                    cursor: 0,
                },

                // If `Connection` establishing fails, then reconnect with
                // backoff.
                Poll::Ready(Err(e)) => {
                    log::error!("Connecting to TeamSpeak server failed: {}", e);
                    State::Backoff {
                        delay: delay_for(self.backoff),
                    }
                }
            }
        } else {
            debug_assert!(
                false,
                "Called poll_connecting() for state {:?}",
                self.state,
            );
            #[allow(unsafe_code)]
            unsafe {
                unreachable_unchecked()
            }
        };

        let _ = mem::replace(&mut self.state, new_state);
        if matches!(self.state, State::Connected { .. }) {
            self.reset_backoff_delay();
            log::debug!("Successfully Connected to TeamSpeak server");
            self.poll_read_connected(cx, buf)
        } else {
            log::debug!("Backoff connection to TeamSpeak server");
            self.poll_backoff(cx, buf)
        }
    }

    /// Polls [`Input`] in [`State::Connected`].
    fn poll_read_connected(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, InputError>> {
        if let State::Connected {
            commands,
            audio,
            decoders,
            data,
            cursor,
            ..
        } = &mut self.state
        {
            // If not all `data` was read yet, then read it to the end.
            if *cursor > 0 {
                return Poll::Ready(Ok(write_audio_be(data, buf, cursor)));
            }

            // We still need to poll and process `InCommand` packets for
            // preserving correct packets processing order inside `tsclientlib`
            // implementation and react as server requires.
            if poll_next_packet("InCommand", commands, cx) {
                // Once `InCommand` packet is polled, we can now poll the actual
                // `InAudio` packet we're interested in.
                match ready!(Pin::new(audio).poll_next(cx)) {
                    Some(Ok(aud)) => {
                        if decode_audio_packet(&aud, data, decoders)?.is_some()
                        {
                            return Poll::Ready(Ok(write_audio_be(
                                data, buf, cursor,
                            )));
                        }

                        // If empty or irrelevant `InAudio` packet is received,
                        // then don't write anything and just continue waiting
                        // for new data.
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    Some(Err(e)) => {
                        log::error!("Receiving InAudio packet failed: {}", e)
                    }
                    None => log::error!("Receiving InAudio packets finished"),
                }
            }
        // If we failed to receive packets or their `Stream`s have finished,
        // then reconnect with backoff and start again.
        } else {
            debug_assert!(
                false,
                "Called poll_read_connected() for state {:?}",
                self.state,
            );
            #[allow(unsafe_code)]
            unsafe {
                unreachable_unchecked()
            }
        };

        let new_state = State::Backoff {
            delay: delay_for(self.backoff),
        };

        let _ = mem::replace(&mut self.state, new_state);
        log::debug!("Backoff connection to TeamSpeak server");
        self.poll_backoff(cx, buf)
    }

    /// Polls [`Input`] in [`State::Backoff`].
    fn poll_backoff(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, InputError>> {
        let new_state = if let State::Backoff { delay } = &mut self.state {
            ready!(Pin::new(delay).poll(cx));

            // After backoff delay is finished, transit to
            // `State::Disconnected` for reconnection.
            State::Disconnected
        } else {
            debug_assert!(
                false,
                "Called poll_backoff() for state {:?}",
                self.state,
            );
            #[allow(unsafe_code)]
            unsafe {
                unreachable_unchecked()
            }
        };

        self.increase_backoff_delay();

        let _ = mem::replace(&mut self.state, new_state);
        self.poll_disconnected(cx, buf)
    }
}

/// Polls next packet out of the given [`Stream`] making it being processed by
/// [`tsclientlib`], and then just drops it, reporting whether packet polling
/// was successful or not.
fn poll_next_packet<P: fmt::Debug>(
    typ: &'static str,
    stream: &mut BoxStream<'static, Result<P, tsproto::Error>>,
    cx: &mut Context<'_>,
) -> bool {
    match Pin::new(stream).poll_next(cx) {
        Poll::Pending => true,
        Poll::Ready(Some(Ok(cmd))) => {
            log::trace!("Received {} packet: {:?}", typ, cmd);
            true
        }
        Poll::Ready(Some(Err(e))) => {
            log::error!("Receiving {} packet failed: {}", typ, e);
            false
        }
        Poll::Ready(None) => {
            log::error!("Receiving {} packets finished", typ);
            false
        }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => f.debug_tuple("State::Disconnected").finish(),
            Self::Connecting {
                commands, audio, ..
            } => f
                .debug_struct("State::Connecting")
                .field("conn", &"BoxFuture<Connection>")
                .field("commands", &{
                    if commands.lock().unwrap().is_some() {
                        "Arc<Mutex<Some(InCommandsStream)>>"
                    } else {
                        "Arc<Mutex<None>>"
                    }
                })
                .field("audio", &{
                    if audio.lock().unwrap().is_some() {
                        "Arc<Mutex<Some(InAudioStream)>>"
                    } else {
                        "Arc<Mutex<None>>"
                    }
                })
                .finish(),
            Self::Connected { .. } => f
                .debug_struct("State::Connected")
                .field("conn", &"Connection")
                .field("commands", &"InCommandsStream")
                .field("audio", &"InAudioStream")
                .finish(),
            Self::Backoff { delay } => f
                .debug_struct("State::Backoff")
                .field("delay", &delay)
                .finish(),
        }
    }
}

/// [`PacketHandler`] that injects received [`InCommandsStream`] and
/// [`InAudioStream`] directly into [`Input`] itself.
#[derive(Clone)]
struct InPacketsInjector {
    /// [`Stream`] of [`InCommand`] packets to be injected.
    commands: Arc<Mutex<Option<InCommandsStream>>>,

    /// [`Stream`] of [`InAudio`] packets to be injected.
    audio: Arc<Mutex<Option<InAudioStream>>>,
}

impl PacketHandler for InPacketsInjector {
    fn new_connection(
        &mut self,
        commands: BoxStream01<InCommand, tsproto::Error>,
        audio: BoxStream01<InAudio, tsproto::Error>,
    ) {
        let _ = self
            .commands
            .lock()
            .unwrap()
            .replace(commands.compat().boxed());
        let _ = self.audio.lock().unwrap().replace(audio.compat().boxed());
    }

    #[inline]
    fn clone(&self) -> PHBox {
        Box::new(Clone::clone(self))
    }
}

/// Writes decoded [PCM 32-bit floating-point][1] audio data into the given
/// `dst` buffer in big endian, and returns the number of written bytes.
///
/// It writes as much data as fits into `dst`, mutating `cursor` to point the
/// position in `src` to perform next write from. If `cursor` is set to `0`,
/// then it means that the whole data was written.
///
/// [1]: https://wiki.multimedia.cx/index.php/PCM
fn write_audio_be(src: &[f32], dst: &mut [u8], cursor: &mut usize) -> usize {
    // `f32` takes 4 bytes in big endian, so we should fit in there.
    let dst_size = dst.len() / 4;
    // How much data in `src` left to be read.
    let src_size = src.len() - *cursor;

    let size = src_size.min(dst_size);
    let size_in_bytes = size * 4;

    BigEndian::write_f32_into(
        &src[(*cursor)..(*cursor + size)],
        &mut dst[..size_in_bytes],
    );

    *cursor += size;
    if *cursor >= src.len() {
        // Once no data left in `src` we reset the `cursor` indicating that.
        *cursor = 0;
    }

    // We should return the number of written bytes.
    size_in_bytes
}

/// Decodes the given [`InAudio`] packet received from [TeamSpeak] server into
/// [PCM 32-bit floating-point][1] audio data, returning the number of decoded
/// samples.
///
/// If no number is returned, the no decoding happened, indicating that given
/// [`InAudio`] packet is not the one could be decoded.
///
/// At the moment, only [Opus]-encoded [`InAudio`] packets are supported.
///
/// [Opus]: https://opus-codec.org
/// [TeamSpeak]: https://teamspeak.com
/// [1]: https://wiki.multimedia.cx/index.php/PCM
fn decode_audio_packet(
    src: &InAudio,
    dst: &mut Vec<f32>,
    decoders: &mut OpusDecoders,
) -> Result<Option<usize>, InputError> {
    use InputError::*;

    if let AudioData::S2C {
        from, codec, data, ..
    }
    | AudioData::S2CWhisper {
        from, codec, data, ..
    } = src.data()
    {
        if !matches!(codec, CodecType::OpusVoice | CodecType::OpusMusic) {
            return Err(UnsupportedCodec(*codec));
        }

        if !decoders.contains_key(from) {
            let dcdr = OpusDecoder::new(
                audiopus::SampleRate::Hz48000,
                // TODO #2: Use stereo?
                //      https://github.com/tyranron/ephyr/issues/2
                audiopus::Channels::Mono,
            )
            .map_err(DecoderCreationFailed)?;
            decoders.insert(*from, dcdr);
        }
        let decoder = decoders.get_mut(from).unwrap();

        if data.is_empty() {
            // Note: In practice, this situation wasn't detected to happen ever.
            decoder.reset_state().map_err(DecoderResetFailed)?;
            log::debug!("Decoder of client {} reset it state", from);
            return Ok(None);
        }
        // When audio stream of member ends (for example, when Push-to-Talk
        // button is released) TeamSpeak server sends 1-byte control frame,
        // which represents an invalid Opus data and breaks the decoding.
        // However, instead of decoding, we can use it to remove the appropriate
        // `OpusDecoder` as it's not necessary anymore.
        if data.len() == 1 {
            let _ = decoders.remove(from);
            return Ok(None);
        }

        let samples_num = loop {
            // TODO #3: Use `fec` for decoding?
            //      https://github.com/tyranron/ephyr/issues/3
            match decoder.decode_float(Some(*data), &mut dst[..], false) {
                Ok(n) => break n,
                Err(audiopus::Error::Opus(
                    audiopus::ErrorCode::BufferTooSmall,
                )) => {
                    // Enlarge the `dst` buffer.
                    let dst_len = dst.len();
                    if dst_len >= Input::OPUS_MAX_FRAME_SIZE {
                        return Err(MaxBufferSizeExceeded(dst_len));
                    } else if dst_len * 2 > Input::OPUS_MAX_FRAME_SIZE {
                        dst.resize(Input::OPUS_MAX_FRAME_SIZE, 0_f32);
                    } else {
                        dst.resize(dst_len * 2, 0_f32);
                    }
                }
                Err(e) => {
                    log::error!("Failed to decode Opus data: {:?}", data);
                    return Err(DecodingFailed(e));
                }
            }
        };
        // Shrink the `dst` buffer to fit the decoded data exactly.
        if samples_num < dst.len() {
            dst.truncate(samples_num);
        }
        Ok(Some(samples_num))
    } else {
        Ok(None)
    }
}

/// Possible errors of capturing audio [`Input`] from [TeamSpeak] server.
///
/// [TeamSpeak]: https://teamspeak.com
#[derive(Clone, Display, Debug, Error)]
pub enum InputError {
    /// Received [`InAudio`] packet from [TeamSpeak] server is encoded with
    /// unsupported codec.
    ///
    /// At the moment, only [Opus]-encoded [`InAudio`] packets are supported.
    ///
    /// [Opus]: https://opus-codec.org
    /// [TeamSpeak]: https://teamspeak.com
    #[display(
        fmt = "Unsupported audio codec {:?}, only Opus is supported",
        _0
    )]
    UnsupportedCodec(#[error(not(source))] CodecType),

    /// Failed to instantiate new [`OpusDecoder`].
    #[display(fmt = "Creating OpusDecoder failed: {}", _0)]
    DecoderCreationFailed(audiopus::Error),

    /// Failed to reset state of existing [`OpusDecoder`].
    #[display(fmt = "OpusDecoder failed to reset its state: {}", _0)]
    DecoderResetFailed(audiopus::Error),

    /// [`OpusDecoder`] failed to decode [Opus] data from received [`InAudio`]
    /// packet.
    ///
    /// [Opus]: https://opus-codec.org
    #[display(fmt = "OpusDecoder failed to decode Opus packet: {}", _0)]
    DecodingFailed(audiopus::Error),

    /// Size of received [Opus] data in [`InAudio`] packet exceeds the maximum
    /// allowed one.
    #[display(
        fmt = "Received Opus packet size {} exceeds maximum allowed size {}",
        _0,
        Input::OPUS_MAX_FRAME_SIZE
    )]
    MaxBufferSizeExceeded(#[error(not(source))] usize),
}

impl From<InputError> for io::Error {
    fn from(e: InputError) -> Self {
        use InputError::*;
        io::Error::new(
            match e {
                UnsupportedCodec(_)
                | DecodingFailed(_)
                | MaxBufferSizeExceeded(_) => io::ErrorKind::InvalidData,
                DecoderCreationFailed(_) | DecoderResetFailed(_) => {
                    io::ErrorKind::Other
                }
            },
            e,
        )
    }
}
