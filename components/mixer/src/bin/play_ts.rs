use std::process::Stdio;

use anyhow::anyhow;
use ephyr_log::log;
use ephyr_mixer::{cli, input::teamspeak};
use futures::future;
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<(), cli::Failure> {
    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = ephyr_log::init(None);

    let res = future::select(
        Box::pin(async move {
            run().await.map_err(|e| log::crit!("Cannot run: {}", e))
        }),
        Box::pin(async {
            let res = ephyr_mixer::shutdown_signal()
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

#[allow(clippy::non_ascii_literal)]
async fn run() -> Result<(), anyhow::Error> {
    let conn = teamspeak::Connection::build("ts3.ts3.online:8722")
        .channel("[cspacer]Best-of-Trance-Radio")
        .name("🤖 ephyr::play_ts");
    let mut ts_input = teamspeak::Input::new(conn);

    let ffmpeg = Command::new("ffplay")
        .args(&["-loglevel", "debug"])
        .args(&["-f", "f32be", "-sample_rate", "48000", "-channels", "2"])
        .args(&["-use_wallclock_as_timestamps", "true"])
        .args(&["-i", "pipe:0"])
        .stdin(Stdio::piped())
        .stderr(Stdio::inherit())
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
