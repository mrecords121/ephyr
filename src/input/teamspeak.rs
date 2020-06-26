//! [TeamSpeak] audio capture.
//!
//! [TeamSpeak]: https://teamspeak.com

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    pin::Pin,
    str,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use audiopus::coder::Decoder as OpusDecoder;
use byteorder::{BigEndian, ByteOrder as _};
use derive_more::{Display, Error};
use futures::{future, ready, sink, Stream, StreamExt as _};
use once_cell::sync::Lazy;
use rand::Rng as _;
use slog_scope as log;
use tokio::{
    io::{self, AsyncRead},
    task::JoinHandle,
};
use tsclientlib::{Connection, DisconnectOptions, StreamItem};
use tsproto_packets::packets::{AudioData, CodecType, InAudioBuf};

pub use tsclientlib::ConnectOptions as Config;

/// Type of [TeamSpeak] channel member ID.
///
/// [TeamSpeak]: https://teamspeak.com
type MemberId = u16;

/// Helper alias for [`OpusDecoder`]s collection used by [`Input`]. Each decoder
/// is dedicated to a concrete [TeamSpeak] channel member.
///
/// [TeamSpeak]: https://teamspeak.com
type OpusDecoders = HashMap<MemberId, OpusDecoder>;

/// Collection of buffers for storing [PCM 32-bit floating-point][1] data
/// decoded by [`OpusDecoder`]. Each buffer is dedicated to a concrete
/// [TeamSpeak] channel member, which transmits any audio at the moment.
///
/// [1]: https://wiki.multimedia.cx/index.php/PCM
/// [TeamSpeak]: https://teamspeak.com
type PcmDataBuffers = HashMap<MemberId, VecDeque<f32>>;

/// Audio input captured from [TeamSpeak] server.
///
/// It produces [PCM 32-bit floating-point big-endian][1] encoded audio samples
/// (`f32be` format in [FFmpeg]'s [notation][2]).
///
/// [FFmpeg]: https://ffmpeg.org
/// [TeamSpeak]: https://teamspeak.com
/// [1]: https://wiki.multimedia.cx/index.php/PCM
/// [2]: https://trac.ffmpeg.org/wiki/audio%20types
pub struct Input {
    /// [`Config`] for establishing new [`Connection`] with.
    cfg: Config,

    /// Established [`Connection`] with a [TeamSpeak] server.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    conn: Option<Connection>,

    /// Set of [`OpusDecoder`]s for each member of [TeamSpeak] channel.
    ///
    /// Because [Opus] decoding is a stateful process, a single [`OpusDecoder`]
    /// cannot process packets from multiple members simultaneously. That's why
    /// each member should have its own instance of [`OpusDecoder`].
    ///
    /// [Opus]: https://opus-codec.org
    /// [TeamSpeak]: https://teamspeak.com
    decoders: OpusDecoders,

    /// Buffer to temporarily hold a raw [PCM 32-bit floating-point][1] data
    /// decoded by [`OpusDecoder`].
    ///
    /// It's reused for decoding each received packet instead of allocating new
    /// memory each time. It's required, because the current API of
    /// [`OpusDecoder`] doesn't allow to decode directly into a [`VecDeque`].
    ///
    /// [1]: https://wiki.multimedia.cx/index.php/PCM
    decoding_buff: Vec<f32>,

    /// Raw [PCM 32-bit floating-point][1] data decoded by [`OpusDecoder`].
    ///
    /// It stores decoded audio data of each [TeamSpeak] channel member
    /// separately, so then it can be mixed into a single audio data stream with
    /// a correct sample rate.
    ///
    /// [1]: https://wiki.multimedia.cx/index.php/PCM
    /// [TeamSpeak]: https://teamspeak.com
    data: PcmDataBuffers,
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

    /// Starts creation of a new [`Input`].
    pub fn new<C: Into<Config>>(cfg: C) -> Self {
        let cfg = {
            use slog::Drain as _;

            let lgr = slog_scope::logger();
            let is_debug = lgr.is_debug_enabled();
            let is_trace = lgr.is_trace_enabled();

            // TODO #6: Memoize TeamSpeak Identity and reuse.
            //      https://github.com/ALLATRA-IT/ephyr/issues/6
            let mut cfg = cfg
                .into()
                .hardware_id(Self::new_hwid())
                .logger(lgr)
                .log_commands(is_debug)
                .log_packets(is_trace);
            // TeamSpeak limits client names by 30 UTF-8 characters max. If the
            // provided name is longer, then we should truncate it to fit into
            // the requirement.
            if cfg.get_name().chars().count() > 30 {
                let n = cfg.get_name().chars().take(30).collect::<String>();
                cfg = cfg.name(n);
            }
            cfg
        };

        Self {
            cfg,
            conn: None,
            decoders: OpusDecoders::new(),
            decoding_buff: vec![0_f32; Input::OPUS_USUAL_FRAME_SIZE],
            data: PcmDataBuffers::new(),
        }
    }

    /// Generates a new random HWID (hardware identification string).
    #[must_use]
    pub fn new_hwid() -> String {
        const BYTES: usize = 16;
        const HEX_BYTES: usize = 2 * BYTES;

        let mut rng = rand::thread_rng();

        let mut first = [0_u8; HEX_BYTES];
        hex::encode_to_slice(&rng.gen::<[u8; BYTES]>(), &mut first).unwrap();

        let mut second = [0_u8; HEX_BYTES];
        hex::encode_to_slice(&rng.gen::<[u8; BYTES]>(), &mut second).unwrap();

        // This is totally safe, because hex-encoded data is guaranteed to be
        // a valid UTF-8 string.
        #[allow(unsafe_code)]
        unsafe {
            format!(
                "{},{}",
                str::from_utf8_unchecked(&first),
                str::from_utf8_unchecked(&second),
            )
        }
    }

    /// Writes decoded [PCM 32-bit floating-point][1] audio data of this
    /// [`Input`] into the given `dst` buffer in big endian, and returns the
    /// number of written bytes.
    ///
    /// # Sample rate preservation
    ///
    /// We cannot simply write into the `dst` buffer all the decoded audio data
    /// "as is", because multiple [TeamSpeak] channel members may transmit audio
    /// at the same moment, so for each member we receive a separate audio
    /// stream and decode it with some sample rate separately. Writing decoded
    /// audio data "as is" for each member will result in a broken sample rate
    /// of the resulting audio data stream (given 2 transmitting members at the
    /// same moment we will produce a 2x48kHz sample rate instead of the
    /// expected 48kHz).
    ///
    /// That's why, if multiple members transmit audio at the same moment, we
    /// should mix it, sample by sample, and produce data with an expected 48kHz
    /// sample rate. We can do that only once we have a decoded data for all
    /// [TeamSpeak] channel members transmitting at the moment.
    ///
    /// [1]: https://wiki.multimedia.cx/index.php/PCM
    /// [TeamSpeak]: https://teamspeak.com
    fn write_mixed_audio_be(&mut self, dst: &mut [u8]) -> Option<usize> {
        let Self {
            data: src,
            decoders,
            ..
        } = self;

        // If there are any empty buffers left for the members, which don't
        // transmit anymore, we should remove them, to not stuck eternally by
        // waiting new data for them.
        src.retain(|k, data| !data.is_empty() || decoders.contains_key(k));

        // Detect how much samples we can mix and write into `dst`.
        let src_size = src.iter().min_by_key(|(_, data)| data.len())?.1.len();
        if src_size == 0 {
            // If there is not enough samples for mixing, or no samples at all,
            // then just don't write anything and wait for the data being
            // enough.
            return None;
        }

        // `f32` takes 4 bytes in big endian, so we should fit in there.
        let dst_size = dst.len() / 4;
        if dst_size == 0 {
            // If there is no enough space to write data, then just don't write
            // anything.
            return None;
        }

        // We only can write as much data as we have, or as much as `dst` buffer
        // can contain.
        let size = src_size.min(dst_size);
        let size_in_bytes = size * 4;

        let mut src_iter = src.iter_mut();
        // First, choose data buffer as the one where we will do the mixing.
        let (_, mixed_data) = src_iter.next()?;
        for (_, data) in src_iter {
            // Then, mix into the resulting audio data stream, sample by sample.
            for (i, f) in &mut mixed_data.iter_mut().take(size).enumerate() {
                *f += data[i];
            }
        }

        let (head, tail) = mixed_data.as_slices();
        if head.len() < size {
            let head_size = head.len();
            let head_size_in_bytes = head_size * 4;
            BigEndian::write_f32_into(
                &head[..head_size],
                &mut dst[..head_size_in_bytes],
            );
            BigEndian::write_f32_into(
                &tail[..(size - head_size)],
                &mut dst[head_size_in_bytes..size_in_bytes],
            );
        } else {
            BigEndian::write_f32_into(&head[..size], &mut dst[..size_in_bytes]);
        }

        // Finally, strip all the written data from the buffers.
        for (_, data) in src.iter_mut() {
            data.drain(..size);
        }

        // We should return the number of written bytes, not samples.
        Some(size_in_bytes)
    }

    /// Decodes the given [`InAudio`] packet received from [TeamSpeak] server
    /// into a [PCM 32-bit floating-point][1] audio data, returning the number
    /// of decoded samples.
    ///
    /// If no number is returned, the no decoding happened, indicating that
    /// given [`InAudio`] packet is not the one could be decoded.
    ///
    /// At the moment, only [Opus]-encoded [`InAudio`] packets are supported.
    ///
    /// [`InAudio`]: tsproto_packets::packets::InAudio
    /// [Opus]: https://opus-codec.org
    /// [TeamSpeak]: https://teamspeak.com
    /// [1]: https://wiki.multimedia.cx/index.php/PCM
    fn decode(
        &mut self,
        src: &InAudioBuf,
    ) -> Result<Option<usize>, InputError> {
        use InputError::{
            DecoderCreationFailed, DecodingFailed, MaxBufferSizeExceeded,
            UnsupportedCodec,
        };

        if let AudioData::S2C {
            from, codec, data, ..
        }
        | AudioData::S2CWhisper {
            from, codec, data, ..
        } = src.data().data()
        {
            if !matches!(codec, CodecType::OpusVoice | CodecType::OpusMusic) {
                return Err(UnsupportedCodec(*codec));
            }

            // When audio stream of member ends (for example, when Push-to-Talk
            // button is released) TeamSpeak server sends 1-byte (or empty)
            // control frame, which represents an invalid Opus data and breaks
            // the decoding. However, instead of decoding, we can use it to
            // remove the appropriate `OpusDecoder` as it's not necessary
            // anymore.
            if data.len() <= 1 {
                let _ = self.decoders.remove(from);
                // Also, remove decoded data buffer of this member, if it
                // doesn't contain any unread data anymore.
                if let Some(true) = self.data.get(from).map(VecDeque::is_empty)
                {
                    self.data.remove(from);
                }
                return Ok(None);
            }

            if !self.decoders.contains_key(from) {
                let dcdr = OpusDecoder::new(
                    audiopus::SampleRate::Hz48000,
                    // TODO #2: Use stereo?
                    //      https://github.com/ALLATRA-IT/ephyr/issues/2
                    audiopus::Channels::Mono,
                )
                .map_err(DecoderCreationFailed)?;
                self.decoders.insert(*from, dcdr);
            }
            let decoder = self.decoders.get_mut(from).unwrap();

            let samples_num = loop {
                // TODO #3: Use `fec` for decoding?
                //      https://github.com/ALLATRA-IT/ephyr/issues/3
                match decoder.decode_float(
                    Some(*data),
                    &mut self.decoding_buff[..],
                    false,
                ) {
                    Ok(n) => break n,
                    Err(audiopus::Error::Opus(
                        audiopus::ErrorCode::BufferTooSmall,
                    )) => {
                        // Enlarge the `self.decoding_buff` buffer.
                        let buff_len = self.decoding_buff.len();
                        if buff_len >= Input::OPUS_MAX_FRAME_SIZE {
                            return Err(MaxBufferSizeExceeded(buff_len));
                        } else if buff_len * 2 > Input::OPUS_MAX_FRAME_SIZE {
                            self.decoding_buff
                                .resize(Input::OPUS_MAX_FRAME_SIZE, 0_f32);
                        } else {
                            self.decoding_buff.resize(buff_len * 2, 0_f32);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to decode Opus data: {:?}", data);
                        return Err(DecodingFailed(e));
                    }
                }
            };
            // Shrink the `self.decoding_buff` buffer to fit the decoded data
            // exactly.
            if samples_num < self.decoding_buff.len() {
                self.decoding_buff.truncate(samples_num);
            }

            // Append decoded data to the member's buffer.
            self.data
                .entry(*from)
                .or_default()
                .extend(self.decoding_buff.iter().copied());

            Ok(Some(samples_num))
        } else {
            Ok(None)
        }
    }
}

impl AsyncRead for Input {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        use InputError as E;

        if self.conn.is_none() {
            log::debug!("Connecting to TeamSpeak server...");
            self.conn = Some(
                Connection::new(self.cfg.clone())
                    .map_err(E::InitializationFailed)?,
            )
        }

        loop {
            // If not all `data` was read yet, then read it as much as possible.
            if let Some(num) = self.write_mixed_audio_be(buf) {
                return Poll::Ready(Ok(num));
            }

            let audio_packet = {
                let mut events = self.conn.as_mut().unwrap().events();
                match ready!(Pin::new(&mut events).poll_next(cx))
                    .ok_or_else(|| E::ReceivingFinished)?
                    .map_err(E::ConnectionFailed)?
                {
                    StreamItem::Audio(packet) => packet,
                    _ => continue,
                }
            };

            let _ = self.decode(&audio_packet)?;
        }
    }
}

impl fmt::Debug for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Input")
            .field("cfg", &self.cfg)
            .field("conn", &self.conn.as_ref().map(|_| "Connection"))
            .field("data", &self.data)
            .field("decoders", &self.decoders)
            .field("decoding_buff", &self.decoding_buff)
            .finish()
    }
}

impl Drop for Input {
    /// Spawns the inner [`Connection`] to be fully drained for disconnecting
    /// from [TeamSpeak] server normally.
    ///
    /// This is required, because disconnecting from [TeamSpeak] server implies
    /// some handshake.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    #[inline]
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            spawn_disconnect(conn)
        }
    }
}

/// Possible errors of capturing audio [`Input`] from [TeamSpeak] server.
///
/// [TeamSpeak]: https://teamspeak.com
#[derive(Debug, Display, Error)]
pub enum InputError {
    /// Initializing [`Connection`] with [TeamSpeak] server failed.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    #[display(
        fmt = "Initializing connection with TeamSpeak server failed: {}",
        _0
    )]
    InitializationFailed(tsclientlib::Error),

    /// Establishing connection with [TeamSpeak] server failed.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    #[display(fmt = "Connecting to TeamSpeak server failed: {}", _0)]
    ConnectionFailed(tsclientlib::Error),

    /// Receiving packets from [TeamSpeak] server finished unexpectedly.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    #[display(
        fmt = "Receiving packets from TeamSpeak server finished unexpectedly"
    )]
    ReceivingFinished,

    /// Received [`InAudio`] packet from [TeamSpeak] server is encoded with
    /// unsupported codec.
    ///
    /// At the moment, only [Opus]-encoded [`InAudio`] packets are supported.
    ///
    /// [`InAudio`]: tsproto_packets::packets::InAudio
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

    /// [`OpusDecoder`] failed to decode [Opus] data from received [`InAudio`]
    /// packet.
    ///
    /// [`InAudio`]: tsproto_packets::packets::InAudio
    /// [Opus]: https://opus-codec.org
    #[display(fmt = "OpusDecoder failed to decode Opus packet: {}", _0)]
    DecodingFailed(audiopus::Error),

    /// Size of received [Opus] data in [`InAudio`] packet exceeds the maximum
    /// allowed one.
    ///
    /// [`InAudio`]: tsproto_packets::packets::InAudio
    /// [Opus]: https://opus-codec.org
    #[display(
        fmt = "Received Opus packet size {} exceeds maximum allowed size {}",
        _0,
        Input::OPUS_MAX_FRAME_SIZE
    )]
    MaxBufferSizeExceeded(#[error(not(source))] usize),
}

impl From<InputError> for io::Error {
    fn from(e: InputError) -> Self {
        use InputError as E;

        let (kind, is_permanent) = match e {
            E::ConnectionFailed(_) => (io::ErrorKind::ConnectionRefused, false),

            E::ReceivingFinished => (io::ErrorKind::BrokenPipe, false),

            E::UnsupportedCodec(_) => (io::ErrorKind::InvalidData, true),

            E::DecodingFailed(_) | E::MaxBufferSizeExceeded(_) => {
                (io::ErrorKind::InvalidData, false)
            }

            E::InitializationFailed(_) | E::DecoderCreationFailed(_) => {
                (io::ErrorKind::Other, true)
            }
        };

        if is_permanent {
            io::Error::new(
                kind,
                backoff::Error::Permanent(io::Error::new(kind, e)),
            )
        } else {
            io::Error::new(kind, e)
        }
    }
}

/// Collection of [`Connection`]s being disconnecting at the moment.
///
/// [1]: https://github.com/tokio-rs/tokio/issues/2053
#[allow(clippy::type_complexity)]
static IN_PROGRESS_DISCONNECTS: Lazy<
    Arc<Mutex<HashMap<u64, JoinHandle<Result<(), tsclientlib::Error>>>>>,
> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// [`tokio::spawn`]s disconnection of the given [`Connection`] and tracks its
/// completion via [`IN_PROGRESS_DISCONNECTS`].
///
/// All disconnects can be awaited to be completed via
/// [`finish_all_disconnects`] function.
fn spawn_disconnect(mut conn: Connection) {
    let mut disconnects = IN_PROGRESS_DISCONNECTS.lock().unwrap();

    let id = loop {
        let id = rand::thread_rng().gen::<u64>();
        if !disconnects.contains_key(&id) {
            break id;
        }
    };

    let _ = disconnects.insert(
        id,
        tokio::spawn(async move {
            conn.disconnect(DisconnectOptions::default())?;
            let _ = conn.events().map(Ok).forward(sink::drain()).await;

            let _ = IN_PROGRESS_DISCONNECTS.lock().unwrap().remove(&id);
            Ok(())
        }),
    );
}

/// Awaits for all disconnections from [TeamSpeak] servers happening at the
/// moment to be completed.
///
/// Call this function __before__ shutting down the [`tokio::runtime`],
/// otherwise disconnects won't proceed normally.
///
/// This is required due to [`tokio::runtime`] [doesn't wait][1] all
/// [`tokio::spawn`]ed tasks to be fully processed when shutting down.
///
/// [TeamSpeak]: https://teamspeak.com
/// [1]: https://github.com/tokio-rs/tokio/issues/2053
pub async fn finish_all_disconnects() {
    let disconnects = {
        IN_PROGRESS_DISCONNECTS
            .lock()
            .unwrap()
            .drain()
            .map(|(_, hndl)| hndl)
            .collect::<Vec<_>>()
    };

    future::join_all(disconnects).await;
}
