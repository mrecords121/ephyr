use derive_more::{Display, Error, From};
use futures::compat::Future01CompatExt as _;
use tsclientlib::{ConnectOptions, Connection, PHBox, PacketHandler};
use tsproto_packets::packets::{InAudio, InCommand};

use crate::State;

use super::TeamSpeakSettings;

pub fn new() -> State<Builder, Empty> {
    State::default()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Builder {
    ts: Option<TeamSpeakSettings>,
    cmd: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct Empty;

impl State<Builder, Empty> {
    #[inline]
    pub fn ts_audio(
        mut self,
        settings: TeamSpeakSettings,
    ) -> State<Builder, WithTeamSpeakSettings> {
        self.inner.ts = Some(settings);
        self.transit()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WithTeamSpeakSettings;

impl State<Builder, WithTeamSpeakSettings> {
    #[inline]
    pub fn cmd<C: Into<String>>(
        mut self,
        cmd: C,
    ) -> State<Builder, WithCommand> {
        self.inner.cmd = Some(cmd.into());
        self.transit()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WithCommand;

impl State<Builder, WithCommand> {
    #[inline]
    pub fn build(self) -> State<Mixer, Initialized> {
        State::wrap(Mixer {
            ts: self.inner.ts.unwrap(),
            cmd: self.inner.cmd.unwrap(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mixer {
    ts: TeamSpeakSettings,
    cmd: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Initialized;

impl State<Mixer, Initialized> {
    pub async fn start(&mut self) -> Result<Connection, Error> {
        let cfg = ConnectOptions::new(self.inner.ts.server_addr.into_owned())
            .channel(self.inner.ts.channel.into())
            .name(self.inner.ts.name_as.into())
            .log_commands(true)
            .log_packets(true)
            .handle_packets(Box::new(self.inner)); // TODO

        let conn = Connection::new(cfg).compat().await?;

        // TODO: catch streams as PacketHandler

        // TODO: start child process

        // T

        Ok(conn)
    }
}

/// Helper alias for declaring [`Box`]ed [`futures_01::Stream`]s,
/// which are [`Send`].
pub type BoxStream01<I, E> =
    Box<dyn futures_01::Stream<Item = I, Error = E> + Send>;

impl PacketHandler for Mixer {
    fn new_connection(
        &mut self,
        commands: BoxStream01<InCommand, tsproto::Error>,
        audio: BoxStream01<InAudio, tsproto::Error>,
    ) {
        // TODO: inject both streams into Mixer
    }

    fn clone(&self) -> PHBox {
        Box::new(Clone::clone(self))
    }
}

#[derive(Debug, Display, Error, From)]
pub enum Error {
    #[display(fmt = "TeamSpeak connection failed: {}", _0)]
    TeamSpeakConnectionFailed(tsclientlib::Error)
}
