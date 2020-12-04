//! Simple HTTP server, which provides a meta information for
//! [`kaltura/nginx-vod-module`][1] to play a scheduled [VOD] playlists, and
//! allows to change playlists via [HTTP API].
//!
//! [HTTP API]: crate::api::vod::meta
//! [VOD]: https://en.wikipedia.org/wiki/Video_on_demand
//! [1]: https://github.com/kaltura/nginx-vod-module

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
pub mod util;
pub mod vod;

/// Runs application.
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
pub fn run() -> Result<(), cli::Failure> {
    let opts = cli::Opts::from_args();

    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = ephyr_log::init(opts.verbose);

    server::run(opts)
}
