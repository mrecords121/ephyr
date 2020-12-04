//! CLI (command line interface).

use std::{fmt, net::IpAddr, path::PathBuf, str::FromStr as _};

use anyhow::anyhow;
use byte_unit::Byte;
use ephyr_log::slog;
use structopt::StructOpt;

/// CLI (command line interface) of the server.
#[derive(Clone, Debug, StructOpt)]
#[structopt(about = "VOD playlists server")]
pub struct Opts {
    /// IP address for the server to listen HTTP requests on.
    #[structopt(
        long,
        env = "EPHYR_VOD_META_HTTP_IP",
        default_value = "0.0.0.0",
        help = "IP to listen HTTP on",
        long_help = "IP address for the server to listen HTTP requests on"
    )]
    pub http_ip: IpAddr,

    /// Port for the server to listen HTTP requests on.
    #[structopt(
        long,
        env = "EPHYR_VOD_META_HTTP_PORT",
        default_value = "8080",
        help = "Port to listen HTTP on",
        long_help = "Port for the server to listen HTTP requests on"
    )]
    pub http_port: u16,

    /// Path to the file with a persisted [`vod::meta::State`].
    ///
    /// [`vod::meta::State`]: crate::vod::meta::State
    #[structopt(
        short,
        long,
        env = "EPHYR_VOD_META_STATE_PATH",
        default_value = "state.vod-meta.json",
        help = "Path to a file to persist state in",
        long_help = "Path to a file to persist state of the server in"
    )]
    pub state: PathBuf,

    /// [`argon2`] hash of [Bearer HTTP token] authorizing the `PUT` HTTP
    /// request which modifies [`vod::meta::State`].
    ///
    /// [`vod::meta::State`]: crate::vod::meta::State
    /// [1]: https://tools.ietf.org/html/rfc6750#section-2.1
    #[structopt(
        short,
        long,
        env = "EPHYR_VOD_META_AUTH_TOKEN_HASH",
        default_value = "$argon2i$v=19$m=1024,t=1,p=1$Nm11fkVNWUxncWhqMy5cYD85a\
                         yY$ueazmtaC7ypqTPCCQAJ+8nIhPqvG4ZW5+ufVhrqN/Hc",
        help = "Argon2 hash of authorization token to modify state",
        long_help = "Argon2 hash of authorization token accepted by PUT HTTP \
                     endpoint, which modifies state of the server \
                     (default value represents Argon2i hash of `qwerty`)"
    )]
    pub auth_token_hash: String,

    /// Path to the directory where [VOD] files should be downloaded from
    /// upstream servers, and cached.
    ///
    /// [VOD]: https://en.wikipedia.org/wiki/Video_on_demand
    #[structopt(
        short,
        long,
        env = "EPHYR_VOD_META_CACHE_PATH",
        default_value = "/var/lib/ephyr/vod/cache",
        help = "Path to directory with cached VOD files",
        long_help = "Path to directory with cached VOD files of the server"
    )]
    pub cache_dir: PathBuf,

    /// Maximum allowed size of the JSON body accepted by `PUT` HTTP request,
    /// which modifies [`vod::meta::State`].
    ///
    /// [`vod::meta::State`]: crate::vod::meta::State
    #[structopt(
        long,
        env = "EPHYR_VOD_META_REQUEST_MAX_SIZE",
        default_value = "1MB",
        help = "Maximum allowed size of request to modify state",
        long_help = "Maximum allowed size of the JSON body accepted by PUT \
                     HTTP endpoint, which modifies state of the server"
    )]
    pub request_max_size: Byte,

    /// Verbosity level of the server logs.
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
        #[allow(clippy::map_err_ignore)]
        slog::Level::from_str(lvl).map_err(|_| {
            anyhow!(
                "'{}' is invalid verbosity level, allowed levels are: \
                 OFF | CRIT | ERRO | WARN | INFO | DEBG | TRCE",
                lvl,
            )
        })
    }
}

/// Error type indicating non-zero process exit code.
pub struct Failure;

impl fmt::Debug for Failure {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "")
    }
}

impl From<()> for Failure {
    #[inline]
    fn from(_: ()) -> Self {
        Self
    }
}
