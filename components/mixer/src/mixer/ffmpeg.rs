//! [FFmpeg] based mixer.
//!
//! [FFmpeg]: https://ffmpeg.org

use std::{collections::BTreeMap, path::Path, process::Stdio};

use anyhow::anyhow;
use tokio::{io, process::Command};

use crate::input::teamspeak;

use super::spec;

/// Mixer that performs mixing via [FFmpeg] invoked as a child process.
///
/// [FFmpeg]: https://ffmpeg.org
#[derive(Debug)]
pub struct Mixer {
    /// RTMP application of live stream being mixed.
    app: String,

    /// RTMP key of live stream being mixed.
    stream: String,

    /// Unique name of this [`Mixer`] for `app`.
    name: String,

    /// [FFmpeg] command to run and perform mixing with.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    cmd: Command,

    /// Audio data to be fed into [FFmpeg]'s STDIN.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    stdin: Option<teamspeak::Input>,
}

impl Mixer {
    /// Creates new `name`d [`Mixer`] for the given `app` and `stream` according
    /// to the provided [`spec::Mixer`].
    #[must_use]
    pub fn new(
        bin: &Path,
        app: &str,
        stream: &str,
        name: &str,
        cfg: &spec::Mixer,
    ) -> Self {
        use ephyr_log::Drain as _;

        let mut mixer = Self {
            app: app.into(),
            stream: stream.into(),
            name: name.into(),
            cmd: Command::new(bin),
            stdin: None,
        };
        let _ = mixer
            .cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        let lgr = ephyr_log::logger();
        if lgr.is_debug_enabled() {
            let _ = mixer.cmd.args(&["-loglevel", "debug"]);
        }

        let srcs = cfg.src.iter().enumerate().collect::<BTreeMap<_, _>>();
        for (name, src) in srcs.values() {
            match src.url.scheme() {
                "ts" => mixer.add_teamspeak_src(name, src),
                "rtmp" => mixer.add_rtmp_src(src),
                _ => unimplemented!(),
            }
        }

        let mut video_num = 0;
        let mut filters = Vec::with_capacity(srcs.len());
        for (i, (name, src)) in &srcs {
            if src.url.scheme() == "rtmp" {
                video_num = *i;
            }

            // WARNING: The filters order matters here!
            let mut extra_filters = String::new();
            if src.url.scheme() == "ts" {
                extra_filters.push_str("aresample=async=1,");
            };
            if src.url.scheme() == "rtmp" {
                extra_filters.push_str("aresample=48000,");
            };
            let delay = src.delay.as_millis();
            if delay > 0 {
                extra_filters
                    .push_str(&format!("adelay=delays={}:all=1,", delay));
            }
            let filter_complex = format!(
                "volume@{name}={volume},\
                 {extra_filters}\
                 azmq=bind_address=tcp\\\\\\://0.0.0.0\\\\\\:{zmq_port}",
                volume = src.volume,
                extra_filters = extra_filters,
                zmq_port = src.zmq.port,
                name = name,
            );

            filters.push(format!(
                "[{num}:a]{filter}[{name}]",
                num = i,
                filter = filter_complex,
                name = name,
            ));
        }
        filters.push(format!(
            "[{filter_names}]amix=inputs={n}:duration=longest[out]",
            filter_names = cfg
                .src
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("]["),
            n = srcs.len(),
        ));
        let _ = mixer
            .cmd
            .args(&["-filter_complex", &filters.join(";")])
            .args(&["-map", "[out]", "-map", &format!("{}:v", video_num)])
            .args(&["-max_muxing_queue_size", "50000000"])
            .args(&["-c:a", "libfdk_aac", "-c:v", "copy", "-shortest"]);

        if cfg.dest.len() > 1 {
            let mut dsts = Vec::with_capacity(cfg.dest.len());
            for dst in cfg.dest.values() {
                let url = dst
                    .url
                    .to_string()
                    .replace("[app]", &mixer.app)
                    .replace("[stream]", &mixer.stream);
                dsts.push(format!("[f=flv]{}", url));
            }
            let _ = mixer.cmd.args(&["-f", "tee", &dsts.join("|")]);
        } else {
            let url = cfg
                .dest
                .values()
                .next()
                .unwrap()
                .url
                .to_string()
                .replace("[app]", &mixer.app)
                .replace("[stream]", &mixer.stream);
            let _ = mixer.cmd.args(&["-f", "flv", url.as_str()]);
        }

        mixer
    }

    /// Adds [`teamspeak::Input`] to inputs for mixing.
    #[allow(clippy::non_ascii_literal)]
    fn add_teamspeak_src(&mut self, name: &str, cfg: &spec::Source) {
        let mut host = cfg.url.host().unwrap().to_string();
        if let Some(port) = cfg.url.port() {
            host = format!("{}:{}", host, port);
        }
        let channel = cfg.url.path()[1..].to_string();
        let name = format!(
            "🤖 {}/{} <- {}/{}",
            self.app, self.stream, self.name, name,
        );
        self.stdin = Some(teamspeak::Input::new(
            teamspeak::Config::new(host.as_str())
                .channel(channel)
                .name(name),
        ));

        let _ = self
            .cmd
            .args(&["-thread_queue_size", "512"])
            .args(&["-f", "f32be", "-sample_rate", "48000", "-channels", "2"])
            .args(&["-use_wallclock_as_timestamps", "true"])
            .args(&["-i", "pipe:0"]);
    }

    /// Adds remote [RTMP] endpoint as input for mixing.
    ///
    /// [RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
    fn add_rtmp_src(&mut self, cfg: &spec::Source) {
        let url = cfg
            .url
            .to_string()
            .replace("[app]", &self.app)
            .replace("[stream]", &self.stream);
        let _ = self.cmd.args(&["-thread_queue_size", "512", "-i", &url]);
    }

    /// Runs this [`Mixer`] until it completes or fails.
    ///
    /// # Errors
    ///
    /// Errors if running and attaching to [FFmpeg] process fails.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    pub async fn run(mut self) -> Result<(), anyhow::Error> {
        let ffmpeg = self
            .cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn FFmpeg process: {}", e))?;

        if let Some(mut ts_audio) = self.stdin {
            let ffmpeg_stdin = &mut ffmpeg.stdin.ok_or_else(|| {
                anyhow!("FFmpeg's STDIN hasn't been captured")
            })?;
            let _ =
                io::copy(&mut ts_audio, ffmpeg_stdin).await.map_err(|e| {
                    anyhow!("Failed to write into FFmpeg's STDIN: {}", e)
                })?;
        } else {
            let _ = ffmpeg
                .wait_with_output()
                .await
                .map_err(|e| anyhow!("FFmpeg process stopped: {}", e))?;
        }
        Ok(())
    }
}
