//! Implementation of [`cli::Command::Mix`].
//!
//! [`cli::Command::Mix`]: crate::cli::Command::Mix

use anyhow::anyhow;
use futures::{future, FutureExt as _};
use slog_scope as log;
use tokio::io;

use crate::{
    cli,
    input::teamspeak,
    mixer::{self, ffmpeg},
};

/// Runs [`cli::Command::Mix`].
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
#[tokio::main]
pub async fn run(opts: &cli::MixOpts) -> Result<(), cli::Failure> {
    let schema = match mixer::Spec::parse(opts) {
        Ok(s) => s,
        Err(e) => {
            log::crit!("Failed to parse specification: {}", e);
            return Err(cli::Failure);
        }
    };

    log::info!("Schema: {:?}", schema);

    let res = future::select(
        Box::pin(async move {
            run_mixers(opts, &schema).await.map_err(|e| {
                log::crit!("Cannot run: {}", e);
                cli::Failure
            })
        }),
        Box::pin(async {
            let res = shutdown_signal()
                .await
                .map(|s| log::info!("Received OS signal {}", s))
                .map_err(|e| {
                    log::error!("Failed to listen OS signals: {}", e);
                    cli::Failure
                });
            log::info!("Shutting down...");
            res
        }),
    )
    .await
    .factor_first()
    .0;

    teamspeak::finish_all_disconnects().await;

    res
}

/// Runs all mixers of the application defined in [`Spec`] for the given
/// [`cli::Opts::app`].
///
/// # Errors
///
/// - If [`Spec`] doesn't contain [`cli::Opts::app`].
/// - If at least one mixer fails to run.
pub async fn run_mixers(
    opts: &cli::MixOpts,
    schema: &mixer::Spec,
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
