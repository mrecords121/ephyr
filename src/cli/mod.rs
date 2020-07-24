//! CLI (command line interface).

pub mod command;

use std::{fmt, net::IpAddr, path::PathBuf, str::FromStr as _};

use anyhow::anyhow;
use structopt::StructOpt;

/// CLI (command line interface) of the application.
#[derive(Clone, Debug, StructOpt)]
#[structopt(about = "Live/VOD streaming solutions kit")]
pub struct Opts {
    /// Command to execute by the application.
    #[structopt(subcommand)]
    pub cmd: Command,

    /// Verbosity level of the application logs.
    #[structopt(
        short,
        long,
        parse(try_from_str = Self::parse_log_level),
        help = "Logs verbosity level: \
                OFF | CRIT | ERRO | WARN | INFO | DEBG | TRCE"
    )]
    pub verbose: Option<slog::Level>,
}

impl Opts {
    /// Parses CLI [`Opts`] from command line arguments.
    ///
    /// Prints the error message and quits the program in case of failure.
    #[inline]
    #[must_use]
    pub fn from_args() -> Self {
        <Self as StructOpt>::from_args()
    }

    /// Parses [`slog::Level`] from the given string.
    ///
    /// This function is required, because [`slog::Level`]'s [`FromStr`]
    /// implementation returns `()`, which is not [`Display`] as [`StructOpt`]
    /// requires.
    ///
    /// # Errors
    ///
    /// If [`slog::Level`] failed to parse from the string.
    ///
    /// [`Display`]: std::fmt::Display
    /// [`FromStr`]: std::str::FromStr
    pub fn parse_log_level(lvl: &str) -> Result<slog::Level, anyhow::Error> {
        slog::Level::from_str(lvl).map_err(|_| {
            anyhow!(
                "'{}' is invalid verbosity level, allowed levels are: \
                 OFF | CRIT | ERRO | WARN | INFO | DEBG | TRCE",
                lvl,
            )
        })
    }
}

/// Possible commands, supported by the application.
#[derive(Clone, Debug, StructOpt)]
pub enum Command {
    /// `mix` command, performing [FFmpeg]-based mixing of live streams.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    #[structopt(about = "Mixes live streams with FFmpeg")]
    Mix(MixOpts),

    /// `serve` command, running some server solution.
    #[structopt(about = "Runs server solution")]
    Serve {
        /// Sub-command to execute by the `serve` command.
        #[structopt(subcommand)]
        cmd: ServeCommand,
    },
}

/// CLI (command line interface) of the `mix` [`Command`].
#[derive(Clone, Debug, StructOpt)]
#[structopt(about = "FFmpeg-based mixer of live streams")]
pub struct MixOpts {
    /// RTMP application of live stream to be mixed.
    ///
    /// This one is referred as `[app]` in [SRS] configuration.
    ///
    /// [SRS]: https://github.com/ossrs/srs
    #[structopt(
        help = "RTMP application of live stream to be mixed \
                ([app] in SRS)",
        long_help = "RTMP application of live stream to be mixed \
                     ([app] in SRS)"
    )]
    pub app: String,

    /// RTMP key of live stream to be mixed.
    ///
    /// This one is referred as `[stream]` in [SRS] configuration.
    ///
    /// [SRS]: https://github.com/ossrs/srs
    #[structopt(
        help = "RTMP key of live stream to be mixed ([stream] in SRS)",
        long_help = "RTMP key of live stream to be mixed ([stream] in SRS)"
    )]
    pub stream: String,

    /// Path to mixing [`Spec`] file of.
    ///
    /// [`Spec`]: crate::Spec
    #[structopt(
        short,
        long,
        env = "EPHYR.MIX.SPEC.FILE",
        default_value = "mix.spec.json",
        help = "Path to mixing spec file",
        long_help = "Path to spec file"
    )]
    pub spec: String,

    /// Path to [FFmpeg] binary.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    #[structopt(
        short,
        long,
        env = "FFMPEG_PATH",
        default_value = "/usr/local/bin/ffmpeg",
        help = "Path to FFmpeg binary",
        long_help = "Path to FFmpeg binary"
    )]
    pub ffmpeg: String,
}

/// Possible commands, supported by the `serve` [`Command`].
#[derive(Clone, Debug, StructOpt)]
pub enum ServeCommand {
    /// `vod-meta` command, running a server of VOD (video on demand) metadata.
    #[structopt(about = "Runs VOD playlists server")]
    VodMeta(VodMetaOpts),
}

/// CLI (command line interface) of the `vod-meta` [`ServeCommand`].
#[derive(Clone, Debug, StructOpt)]
#[structopt(about = "Server of VOD (video on demand) metadata")]
pub struct VodMetaOpts {
    /// IP address for the `vod-meta` server to listen HTTP requests on.
    #[structopt(
        long,
        env = "EPHYR.VOD_META.HTTP.IP",
        default_value = "0.0.0.0",
        help = "IP to listen HTTP on",
        long_help = "IP address for `vod-meta` server to listen HTTP requests \
                     on"
    )]
    pub http_ip: IpAddr,

    /// Port for the `vod-meta` server to listen HTTP requests on.
    #[structopt(
        long,
        env = "EPHYR.VOD_META.HTTP.PORT",
        default_value = "8080",
        help = "Port to listen HTTP on",
        long_help = "Port for `vod-meta` server to listen HTTP requests on"
    )]
    pub http_port: u16,

    /// Path to the file with a persisted [`vod::meta::State`].
    ///
    /// [`vod::meta::State`]: crate::vod::meta::State
    #[structopt(
        short,
        long,
        env = "EPHYR.VOD_META.STATE.FILE",
        default_value = "state.vod-meta.json",
        help = "Path to file with persisted state",
        long_help = "Path to file with persisted state of `vod-meta` server"
    )]
    pub state: PathBuf,
}

/// Error type indicating non-zero process exit code.
pub struct Failure;

impl fmt::Debug for Failure {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "")
    }
}
