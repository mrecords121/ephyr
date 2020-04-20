//! CLI (command line interface).

use std::str::FromStr as _;

use anyhow::anyhow;
use smart_default::SmartDefault;
use structopt::StructOpt;

/// CLI (command line interface) of application.
#[derive(Clone, Debug, SmartDefault, StructOpt)]
#[structopt(about = "FFmpeg-based mixer of live streams.")]
pub struct Opts {
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

    /// Path to [`Spec`] file of application.
    ///
    /// [`Spec`]: crate::Spec
    #[default = "spec.json"]
    #[structopt(
        short,
        long,
        env = "SPEC.FILE",
        default_value = "spec.json",
        help = "Path to spec file",
        long_help = "Path to spec file"
    )]
    pub spec: String,

    /// Verbosity level of application logs.
    #[structopt(
        short,
        long,
        parse(try_from_str = parse_log_level),
        help = "Logs verbosity level: \
                OFF | CRIT | ERRO | WARN | INFO | DEBG | TRCE"
    )]
    pub verbose: Option<slog::Level>,
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
fn parse_log_level(lvl: &str) -> Result<slog::Level, anyhow::Error> {
    slog::Level::from_str(lvl).map_err(|_| {
        anyhow!(
            "'{}' is invalid verbosity level, allowed levels are: \
         OFF | CRIT | ERRO | WARN | INFO | DEBG | TRCE",
            lvl
        )
    })
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
}
