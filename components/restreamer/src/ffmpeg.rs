use std::{
    collections::HashMap,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    process::Stdio,
};

use ephyr_log::log;
use futures::future::{self, FutureExt as _, TryFutureExt as _};
use tokio::process::Command;
use url::Url;

use crate::{
    display_panic, register_async_drop,
    state::{Restream, State, Status},
};

#[derive(Clone, Debug)]
pub struct RestreamersPool {
    ffmpeg_path: PathBuf,
    pool: HashMap<(u64, u64), RestreamerProcess>,
    state: State,
}

impl RestreamersPool {
    #[inline]
    #[must_use]
    pub fn new<P: Into<PathBuf>>(ffmpeg_path: P, state: State) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            pool: HashMap::new(),
            state,
        }
    }

    pub fn apply(&mut self, restreams: Vec<Restream>) {
        if restreams.is_empty() {
            return;
        }

        let mut new = HashMap::with_capacity(self.pool.len());

        for r in &restreams {
            if !r.enabled || r.input.status() != Status::Online {
                continue;
            }
            for o in &r.outputs {
                if !o.enabled {
                    continue;
                }
                let key = (r.input.hash(), o.hash());
                let _ = new.insert(
                    key,
                    self.pool.remove(&key).unwrap_or_else(|| {
                        Restreamer::new(&self.ffmpeg_path)
                            .src_url(&r.input.srs_url())
                            .dst_url(&o.dst)
                            .run(key, self.state.clone())
                    }),
                );
            }
        }

        self.pool = new
    }
}

#[derive(Debug)]
pub struct Restreamer {
    cmd: Command,
}

impl Restreamer {
    #[must_use]
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let mut cmd = Command::new(path.as_ref());
        let _ = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        Self { cmd }
    }

    #[inline]
    #[must_use]
    pub fn src_url(mut self, url: &Url) -> Self {
        let _ = self.cmd.args(&["-i", url.as_str()]);
        self
    }

    #[inline]
    #[must_use]
    pub fn dst_url(mut self, url: &Url) -> Self {
        let _ = self.cmd.args(&["-c", "copy", "-f", "flv", url.as_str()]);
        self
    }

    #[must_use]
    pub fn run(self, key: (u64, u64), state: State) -> RestreamerProcess {
        let (mut cmd, state_for_abort) = (self.cmd, state.clone());
        let (spawner, abort_handle) = future::abortable(async move {
            loop {
                let (cmd, state) = (&mut cmd, &state);
                let _ = AssertUnwindSafe(async move {
                    Self::set_status(Status::Initializing, key, state);
                    let process = cmd.spawn().map_err(|e| {
                        log::crit!("Cannot start FFmpeg re-streamer: {}", e)
                    })?;
                    Self::set_status(Status::Online, key, state);
                    let out =
                        process.wait_with_output().await.map_err(|e| {
                            log::crit!(
                                "Failed to observe FFmpeg re-streamer: {}",
                                e,
                            )
                        })?;
                    log::error!(
                        "FFmpeg re-streamer stopped with exit code: {}\n{}",
                        out.status,
                        String::from_utf8_lossy(&out.stderr),
                    );
                    Ok(())
                })
                .unwrap_or_else(|_: ()| {
                    Self::set_status(Status::Offline, key, state)
                })
                .catch_unwind()
                .await
                .map_err(|p| {
                    log::crit!(
                        "Panicked while spawning/observing FFmpeg \
                         re-streamer: {}",
                        display_panic(&p),
                    );
                });
            }
        });

        // Start FFmpeg re-streamer as a child process.
        register_async_drop(tokio::spawn(spawner.map(move |_| {
            Self::set_status(Status::Offline, key, &state_for_abort)
        })));

        RestreamerProcess { abort_handle }
    }

    fn set_status(status: Status, key: (u64, u64), state: &State) {
        if let Some(r) = state
            .lock_mut()
            .iter_mut()
            .find(|r| r.input.hash() == key.0)
        {
            if let Some(o) = r.outputs.iter_mut().find(|o| o.hash() == key.1) {
                o.status = status;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct RestreamerProcess {
    abort_handle: future::AbortHandle,
}

impl Drop for RestreamerProcess {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}
