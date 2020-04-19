use std::{
    collections::{BTreeMap, HashMap},
    process::Stdio,
};

use anyhow::anyhow;
use tokio::{io, process::Command};

use crate::{filter::silence, input::teamspeak, spec};

pub struct Mixer {
    app: String,
    stream: String,
    cmd: Command,
    stdin: Option<silence::Filler<teamspeak::Input>>,
}

impl Mixer {
    pub fn new(app: &str, stream: &str, cfg: &spec::Mixer) -> Self {
        use slog::Drain as _;

        let mut mixer = Self {
            app: app.into(),
            stream: stream.into(),
            cmd: Command::new("ffmpeg"),
            stdin: None,
        };
        mixer
            .cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        let lgr = slog_scope::logger();
        if lgr.is_debug_enabled() {
            mixer.cmd.args(&["-loglevel", "debug"]);
        }

        let srcs = cfg.src.iter().enumerate().collect::<BTreeMap<_, _>>();
        for (_, (name, src)) in &srcs {
            match src.url.scheme() {
                "ts" => mixer.add_teamspeak_src(name, src),
                "rtmp" => mixer.add_rtmp_src(src),
                _ => unimplemented!(),
            }
        }

        let mut filters = Vec::with_capacity(srcs.len());
        let mut video_num = 0;
        for (i, (name, src)) in &srcs {
            let mut f = format!(
                "adelay@x=delays={delay}|all=1,\
                 volume={volume},\
                 azmq=bind_address=tcp\\\\\\://0.0.0.0\\\\\\:{zmq_port}",
                delay = src.delay.as_millis(),
                volume = src.volume,
                zmq_port = src.zmq.port,
            );
            if src.url.scheme() == "ts" {
                f = format!("aresample=async=1,{}", f);
            }
            if src.url.scheme() == "rtmp" {
                video_num = *i;
            }
            filters.push(format!(
                "[{num}:a]{filter}[{name}]",
                num = i,
                filter = f,
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
        mixer
            .cmd
            .args(&["-filter_complex", &filters.join(";")])
            .args(&["-map", "[out]", "-map", &format!("{}:v", video_num)])
            .args(&["-c:a", "libfdk_aac", "-c:v", "copy", "-shortest"]);

        if cfg.dest.len() > 1 {
            let mut dsts = Vec::with_capacity(cfg.dest.len());
            for (_, dst) in &cfg.dest {
                let url = dst
                    .url
                    .to_string()
                    .replace("[app]", &mixer.app)
                    .replace("[stream]", &mixer.stream);
                dsts.push(format!("[f=flv]{}", url));
            }
            mixer.cmd.args(&["-f", "tee", &dsts.join("|")]);
        } else {
            let url = cfg.dest.values().next().unwrap().url
                .to_string()
                .replace("[app]", &mixer.app)
                .replace("[stream]", &mixer.stream);
            mixer
                .cmd
                .args(&["-f", "flv", url.as_str()]);
        }

        mixer
    }

    fn add_teamspeak_src(&mut self, name: &str, cfg: &spec::Source) {
        let mut host = cfg.url.host().unwrap().to_string();
        if let Some(port) = cfg.url.port() {
            host = format!("{}:{}", host, port);
        }
        self.stdin = Some(silence::Filler::new(
            teamspeak::Input::new(host)
                .channel(&cfg.url.path()[1..])
                .name_as(format!(
                    "[Bot] SRS {}/{} -> {}",
                    self.app, self.stream, name,
                ))
                .build(),
            8000, // Hz
        ));

        self.cmd
            .args(&["-f", "f32be", "-sample_rate", "48000"])
            .args(&["-use_wallclock_as_timestamps", "true"])
            .args(&["-i", "pipe:0"]);
    }

    fn add_rtmp_src(&mut self, cfg: &spec::Source) {
        let url = cfg
            .url
            .to_string()
            .replace("[app]", &self.app)
            .replace("[stream]", &self.stream);
        self.cmd.args(&["-i", &url]);
    }

    pub async fn run(mut self) -> Result<(), anyhow::Error> {
        let mut ffmpeg = self
            .cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn FFmpeg process: {}", e))?;

        if let Some(mut ts_audio) = self.stdin {
            let ffmpeg_stdin = &mut ffmpeg.stdin.ok_or_else(|| {
                anyhow!("FFmpeg's STDIN hasn't been captured")
            })?;
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
