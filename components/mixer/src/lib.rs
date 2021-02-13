//! Application to capture audio from [TeamSpeak] server, and feed it to
//! [FFmpeg] for mixing with [RTMP] stream.
//!
//! At the moment, it's intended to be called as [SRS] `exec.publish` directive,
//! so performs mixing on-demand (when [RTMP] stream is pushed to [SRS]).
//!
//! [FFmpeg]: https://ffmpeg.org
//! [RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
//! [SRS]: https://github.com/ossrs/srs
//! [TeamSpeak]: https://teamspeak.com

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

pub mod cli;
pub mod input;
pub mod mixer;

use anyhow::anyhow;
use ephyr_log::log;
use futures::{future, FutureExt as _};
use tokio::io;

use crate::{input::teamspeak, mixer::ffmpeg};

/// Runs application.
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
#[tokio::main]
pub async fn run() -> Result<(), cli::Failure> {
    let opts = cli::Opts::from_args();
    let opts = &opts;

    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = ephyr_log::init(opts.verbose);

    let schema = mixer::Spec::parse(opts)
        .map_err(|e| log::crit!("Failed to parse specification: {}", e))?;

    log::info!("Schema: {:?}", schema);

    let res = future::select(
        Box::pin(async move {
            run_mixers(&opts, &schema)
                .await
                .map_err(|e| log::crit!("Cannot run: {}", e))
        }),
        Box::pin(async {
            let res = shutdown_signal()
                .await
                .map(|s| log::info!("Received OS signal {}", s))
                .map_err(|e| log::error!("Failed to listen OS signals: {}", e));
            log::info!("Shutting down...");
            res
        }),
    )
    .await
    .factor_first()
    .0;

    teamspeak::finish_all_disconnects().await;

    res.map_err(Into::into)
}

/// Runs all mixers of the application defined in [`Spec`] for the given
/// [`cli::Opts::app`].
///
/// # Errors
///
/// - If [`Spec`] doesn't contain [`cli::Opts::app`].
/// - If at least one mixer fails to run.
///
/// [`Spec`]: crate::mixer::Spec
pub async fn run_mixers(
    opts: &cli::Opts,
    schema: &mixer::Spec,
) -> Result<(), anyhow::Error> {
    let mixers_spec = schema.spec.get(&opts.app).ok_or_else(|| {
        anyhow!("Spec doesn't allows '{}' live stream app", opts.app)
    })?;

    if mixers_spec.is_empty() {
        future::pending::<()>().await;
        return Ok(());
    }

    drop(
        future::try_join_all(mixers_spec.iter().map(|(name, cfg)| {
            ffmpeg::Mixer::new(
                opts.ffmpeg.as_path(),
                &opts.app,
                &opts.stream,
                name,
                cfg,
            )
            .run()
        }))
        .await?,
    );

    Ok(())
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
