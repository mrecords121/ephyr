//! Mixing application to be used as [SRS] `exec.publish` command.
//!
//! It pulls RTMP stream from [SRS], and mixes it with other sources described
//! in [`Spec`], republishing the result to specified endpoints.
//!
//! [SRS]: https://github.com/ossrs/srs

#![deny(
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code
)]
#![warn(
    deprecated_in_future,
    missing_docs,
    unused_import_braces,
    unused_labels,
    unused_qualifications,
    unreachable_pub
)]

pub mod api;
pub mod cli;
pub mod input;
pub mod mixer;
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
    let _log_guard = slog_scope::set_global_logger(main_logger(opts.verbose));

    match opts.cmd {
        cli::Command::Mix(opts) => cli::command::mix::run(&opts),
        cli::Command::Serve { cmd } => match cmd {
            cli::ServeCommand::VodMeta(opts) => {
                cli::command::serve::vod_meta::run(opts)
            }
        },
    }
}

/// Creates, configures and returns main [`Logger`] of the application.
///
/// [`Logger`]: slog::Logger
#[must_use]
pub fn main_logger(level: Option<slog::Level>) -> slog::Logger {
    use slog::Drain as _;
    use slog_async::OverflowStrategy::Drop;

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();

    let drain = drain
        .filter_level(level.unwrap_or(slog::Level::Error))
        .fuse();

    let drain = slog_async::Async::new(drain)
        .overflow_strategy(Drop)
        .build()
        .fuse();

    slog::Logger::root(drain, slog::o!())
}
