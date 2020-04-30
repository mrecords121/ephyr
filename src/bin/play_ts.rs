use std::{
    process::{self, Stdio},
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
};

use anyhow::anyhow;
use ephyr::{cli, filter::silence, input::teamspeak};
use futures::{future, FutureExt as _};
use slog_scope as log;
use tokio::process::Command;

fn main() {
    let mut opts = cli::Opts::default();
    opts.verbose = Some(slog::Level::Debug);

    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = slog_scope::set_global_logger(ephyr::main_logger(&opts));

    let exit_code = Arc::new(AtomicI32::new(0));
    let exit_code_ref = exit_code.clone();

    tokio_compat::run_std(
        future::select(
            async move {
                if let Err(e) = run().await {
                    log::crit!("Cannot run: {}", e);
                    exit_code_ref.compare_and_swap(0, 1, Ordering::SeqCst);
                }
            }
            .boxed(),
            async {
                match ephyr::shutdown_signal().await {
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
    let code = Arc::try_unwrap(exit_code).unwrap().into_inner();
    process::exit(code);
}

#[allow(clippy::non_ascii_literal)]
async fn run() -> Result<(), anyhow::Error> {
    let ts_input = teamspeak::Input::new("ts3.ts3.online:8722")
        .channel("[cspacer]Best-of-Trance-Radio")
        .name_as("🤖 ephyr::play_ts")
        .build();
    let mut ts_input = silence::Filler::new(ts_input, 8000);

    let ffmpeg = Command::new("ffplay")
        .arg("-f")
        .arg("f32be")
        .arg("-sample_rate")
        .arg("48000")
        .arg("-use_wallclock_as_timestamps")
        .arg("true")
        .arg("-i")
        .arg("pipe:0")
        .arg("-af")
        .arg("aresample=async=1")
        .arg("-loglevel")
        .arg("debug")
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn FFmpeg: {}", e))?;
    let ffmpeg_stdin = &mut ffmpeg
        .stdin
        .ok_or_else(|| anyhow!("FFmpeg's STDIN hasn't been captured"))?;

    tokio::io::copy(&mut ts_input, ffmpeg_stdin)
        .await
        .map_err(|e| anyhow!("Failed to write data: {}", e))?;

    Ok(())
}
