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

pub mod cli;
pub mod filter;
pub mod input;
pub mod mixer;
pub mod spec;

use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};

use anyhow::anyhow;
use futures::{future, FutureExt as _};
use slog_scope as log;
use tokio::io;

use self::mixer::ffmpeg;

#[doc(inline)]
pub use self::spec::Spec;

/// Runs application, returning its exit code.
#[must_use]
pub fn run() -> i32 {
    let opts = cli::Opts::from_args();

    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = slog_scope::set_global_logger(main_logger(&opts));

    let schema = match Spec::parse(&opts) {
        Ok(s) => s,
        Err(e) => {
            log::crit!("Failed to parse specification: {}", e);
            return 2;
        }
    };

    log::info!("Schema: {:?}", schema);

    let exit_code = Arc::new(AtomicI32::new(0));
    let exit_code_ref = exit_code.clone();

    tokio_compat::run_std(
        future::select(
            async move {
                if let Err(e) = run_mixers(&opts, &schema).await {
                    log::crit!("Cannot run: {}", e);
                    exit_code_ref.compare_and_swap(0, 1, Ordering::SeqCst);
                }
            }
            .boxed(),
            async {
                match shutdown_signal().await {
                    Ok(s) => log::info!("Received OS signal {}", s),
                    Err(e) => log::error!("Failed to listen OS signals: {}", e),
                }
                log::info!("Shutting down...")
            }
            .boxed(),
        )
        .map(|_| ()),
    );

    // Unwrapping is OK here, because at this moment `exit_code` is not shared
    // anymore, as runtime has finished.
    Arc::try_unwrap(exit_code).unwrap().into_inner()
}

/// Runs all mixers of the application defined in [`Spec`] for the given
/// [`cli::Opts::app`].
///
/// # Errors
///
/// - If [`Spec`] doesn't contain [`cli::Opts::app`].
/// - If at least one mixer fails to run.
pub async fn run_mixers(
    opts: &cli::Opts,
    schema: &Spec,
) -> Result<(), anyhow::Error> {
    let mixers_spec = schema.spec.get(&opts.app).ok_or_else(|| {
        anyhow!("Spec doesn't allows '{}' live stream app", opts.app)
    })?;

    if mixers_spec.is_empty() {
        future::pending::<()>().await;
        return Ok(());
    }

    future::try_join_all(mixers_spec.iter().map(|(name, cfg)| {
        ffmpeg::Mixer::new(&opts.ffmpeg, &opts.app, &opts.stream, name, cfg)
            .run()
    }))
    .await?;

    Ok(())
}

/// Creates, configures and returns main [`Logger`] of the application.
///
/// [`Logger`]: slog::Logger
#[must_use]
pub fn main_logger(opts: &cli::Opts) -> slog::Logger {
    use slog::Drain as _;
    use slog_async::OverflowStrategy::Drop;

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();

    let level = opts.verbose.unwrap_or(slog::Level::Error);
    let drain = drain.filter_level(level).fuse();

    let drain = slog_async::Async::new(drain)
        .overflow_strategy(Drop)
        .build()
        .fuse();

    slog::Logger::root(drain, slog::o!())
}

/// Awaits the first OS signal for shutdown and returns its name.
///
/// # Errors
///
/// If listening to OS signals fails.
pub async fn shutdown_signal() -> io::Result<&'static str> {
    #[cfg(unix)]
    #[allow(clippy::mut_mut)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut hangup = signal(SignalKind::hangup())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        let mut pipe = signal(SignalKind::pipe())?;
        let mut quit = signal(SignalKind::quit())?;
        let mut terminate = signal(SignalKind::terminate())?;

        Ok(futures::select! {
            _ = hangup.recv().fuse() => "SIGHUP",
            _ = interrupt.recv().fuse() => "SIGINT",
            _ = pipe.recv().fuse() => "SIGPIPE",
            _ = quit.recv().fuse() => "SIGQUIT",
            _ = terminate.recv().fuse() => "SIGTERM",
        })
    }

    #[cfg(not(unix))]
    {
        use tokio::signal;

        signal::ctrl_c().await;
        Ok("ctrl-c")
    }
}
