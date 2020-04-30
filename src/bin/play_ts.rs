use std::process::Stdio;

use ephyr::{filter::silence, input::teamspeak};
use tokio::process;

fn main() {
    tokio_compat::run_std(async {
        let ts_input = teamspeak::Input::new("ts3.ts3.online:8722")
            .channel("[cspacer]Best-of-Trance-Radio")
            .name_as("🤖 ephyr::play_ts")
            .build();
        let mut ts_input = silence::Filler::new(ts_input, 8000);

        let ffmpeg = process::Command::new("ffplay")
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
            .kill_on_drop(true)
            .spawn()
            .expect("Failed to spawn FFmpeg");
        let ffmpeg_stdin =
            &mut ffmpeg.stdin.expect("FFmpeg's STDIN hasn't been captured");

        tokio::io::copy(&mut ts_input, ffmpeg_stdin)
            .await
            .expect("Failed to write data");
    });
}
