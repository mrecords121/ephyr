use std::borrow::Cow;

use futures::compat::Future01CompatExt as _;
use futures_01::{Future as _, Stream as _};
use slog_scope as log;
use tokio::process::Command;
use tsclientlib::{ConnectOptions, Connection, PHBox, PacketHandler};
use tsproto_packets::packets::{InAudio, InCommand};

pub use tsclientlib::Error;

#[derive(Clone, Debug)]
pub struct TeamSpeakSettings {
    pub server_addr: Cow<'static, str>,
    pub channel: Cow<'static, str>,
    pub name_as: Cow<'static, str>,
}

pub struct FFmpegMixerBuilder {}

#[derive(Clone, Debug)]
pub struct FFmpegMixer {
    ts: TeamSpeakSettings,
}

impl FFmpegMixer {
    pub fn new() -> Self {}
}

impl AudioFetcher {
    pub async fn start(self) -> Result<Connection, Error> {
        let cfg = ConnectOptions::new(self.server_addr.into_owned())
            .channel(self.channel.into())
            .name(self.name.into())
            .log_commands(self.verbose >= 1)
            .log_packets(self.verbose >= 2)
            .log_udp_packets(self.verbose >= 3)
            .handle_packets(Box::new(FFmpegSink));

        Connection::new(cfg).compat().await
    }
}

/// Helper alias for declaring [`Box`]ed [`futures_01::Stream`]s,
/// which are [`Send`].
pub type OldBoxStream<I, E> =
    Box<dyn futures_01::Stream<Item = I, Error = E> + Send>;

/// [`PacketHandler`] which spawns [FFmpeg] process and feeds it with raw [Opus]
/// audio-data decoded from handled [`InAudio`] packets.
///
/// [FFmpeg]: https://ffmpeg.org
/// [Opus]: https://opus-codec.org
#[derive(Clone, Copy, Debug)]
pub struct FFmpegSink;

impl PacketHandler for FFmpegSink {
    fn new_connection(
        &mut self,
        commands: OldBoxStream<InCommand, tsproto::Error>,
        audio: OldBoxStream<InAudio, tsproto::Error>,
    ) { /*
         // Drop commands, as we do not require to interact with
         // TeamSpeak3 server.
         let _ = tokio_01::spawn(
             commands
                 .for_each(|packet| Ok(drop(packet)))
                 .map_err(|e| log::error!("BlackHole failed: {}", e)),
         );

         let ffmpeg = Command::new("ffplay")
             .args(&["http://radio.casse-tete.solutions/salut-radio-64.mp3"])
             .kill_on_drop(true)
             .spawn();
         */

        // TODO: prepare audio (decode with Opus)

        // TODO: spawn FFmpeg process and feed data into
    }

    fn clone(&self) -> PHBox {
        Box::new(*self)
    }
}

/// [`PacketHandler`] which just drops every single handled packet.
#[derive(Clone, Copy, Debug)]
pub struct BlackHole;

impl PacketHandler for BlackHole {
    fn new_connection(
        &mut self,
        commands: OldBoxStream<InCommand, tsproto::Error>,
        audio: OldBoxStream<InAudio, tsproto::Error>,
    ) {
        use futures_01::future::Either;

        let _ = tokio_01::spawn(
            audio
                .map(Either::A)
                .select(commands.map(Either::B))
                .for_each(|packet| Ok(drop(packet)))
                .map_err(|e| log::error!("BlackHole failed: {}", e)),
        );
    }

    fn clone(&self) -> PHBox {
        Box::new(*self)
    }
}
