//! Ephyr [RTMP] re-streaming server.
//!
//! [RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol

#![deny(
    broken_intra_doc_links,
    missing_debug_implementations,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code
)]
#![warn(
    deprecated_in_future,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]

pub mod api;
pub mod cli;
pub mod server;
pub mod state;

use ephyr_log::slog;

pub use self::state::State;

/// Runs application.
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
pub fn run() -> Result<(), cli::Failure> {
    let cfg = cli::Opts::from_args();

    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = ephyr_log::init(cfg.verbose.or_else(|| {
        if cfg.debug {
            Some(slog::Level::Debug)
        } else {
            None
        }
    }));

    server::run(cfg)
}
