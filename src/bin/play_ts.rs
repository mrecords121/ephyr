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

#[tokio::main]
async fn main() {
    let mut opts = cli::Opts::default();
    opts.verbose = Some(slog::Level::Debug);

    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = slog_scope::set_global_logger(ephyr::main_logger(&opts));

    let exit_code = Arc::new(AtomicI32::new(0));
    let exit_code_ref = exit_code.clone();

    let _ = future::select(
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
    .await;

    // Unwrapping is OK here, because at this moment `exit_code` is not shared
    // anymore, as runtime has finished.
    let code = Arc::try_unwrap(exit_code).unwrap().into_inner();
    process::exit(code);
}

#[allow(clippy::non_ascii_literal)]
async fn run() -> Result<(), anyhow::Error> {
    let cfg = teamspeak::Config::new("ts3.ts3.online:8722")
        .channel("[cspacer]Best-of-Trance-Radio")
        .name("🤖 ephyr::play_ts");
    let mut ts_input = silence::Filler::new(teamspeak::Input::new(cfg), 8000);

    let ffmpeg = Command::new("ffmpeg")
        .args(&["-loglevel", "debug"])
        .args(&["-thread_queue_size", "512"])
        .args(&["-i", "rtmp://127.0.0.1:1935/input/live"])
        .args(&["-thread_queue_size", "512"])
        .args(&["-f", "f32be", "-sample_rate", "48000"])
        .args(&["-use_wallclock_as_timestamps", "true"])
        .args(&["-i", "pipe:0"])
        .args(&[
            "-filter_complex",
            "[0:a]adelay=delays=0|all=1,\
                  volume@org=0.7,\
                  azmq=bind_address=tcp\\\\\\://0.0.0.0\\\\\\:6001[org];\
             [1:a]volume@trn=2,\
                  aresample=async=1,\
                  adelay=delays=7000|all=1,\
                  azmq=bind_address=tcp\\\\\\://0.0.0.0\\\\\\:6002[trn];\
             [org][trn]amix=inputs=2:duration=longest[out]",
        ])
        .args(&["-map", "[out]", "-map", "0:v"])
        .args(&["-max_muxing_queue_size", "50000000"])
        .args(&["-c:a", "libfdk_aac", "-c:v", "copy", "-shortest"])
        .args(&["-f", "tee", "[f=flv]rtmp://127.0.0.1:1935/output/live"])
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
