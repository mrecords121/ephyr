//! [FFmpeg]-based definitions and implementations.
//!
//! [FFmpeg]: https://ffmpeg.org

use std::{
    collections::HashMap,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use ephyr_log::log;
use futures::future::{self, FutureExt as _, TryFutureExt as _};
use tokio::{process::Command, time};
use url::Url;

use crate::{
    display_panic,
    state::{Restream, State, Status},
};

/// Pool of [FFmpeg] processes performing restreaming of media traffic.
///
/// [FFmpeg]: https://ffmpeg.org
#[derive(Clone, Debug)]
pub struct RestreamersPool {
    /// Path to [FFmpeg] binary used for spawning processes.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    ffmpeg_path: PathBuf,

    /// Pool of currently running [FFmpeg] processes identified by it source
    /// and destination and represented by the processes' abort handles.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    pool: HashMap<(u64, u64), DroppableAbortHandle>,

    /// Application [`State`] dictating which [FFmpeg] processes should run.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    state: State,
}

impl RestreamersPool {
    /// Creates new [`RestreamersPool`] out of the given parameters.
    #[inline]
    #[must_use]
    pub fn new<P: Into<PathBuf>>(ffmpeg_path: P, state: State) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            pool: HashMap::new(),
            state,
        }
    }

    /// Adjusts this [`RestreamersPool`] to run [FFmpeg] restreaming processes
    /// according to the given [`Restream`]s.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    pub fn apply(&mut self, restreams: Vec<Restream>) {
        if restreams.is_empty() {
            return;
        }

        let mut new = HashMap::with_capacity(self.pool.len() + 1);

        for r in &restreams {
            if !r.enabled {
                continue;
            }

            if r.input.is_pull() {
                let key = (
                    r.input.upstream_url_hash().unwrap(),
                    r.input.srs_url_hash(),
                );
                let val = self
                    .pool
                    .remove(&key)
                    .or_else(|| new.remove(&key))
                    .unwrap_or_else(|| {
                        Restreamer::new(&self.ffmpeg_path)
                            .src_url(r.input.upstream_url().unwrap())
                            .dst_url(&r.input.srs_url())
                            .run(key, self.state.clone())
                    });
                let _ = new.insert(key, val);
            }

            if r.input.status() != Status::Online {
                continue;
            }

            for o in &r.outputs {
                if !o.enabled {
                    continue;
                }

                let key = (r.input.srs_url_hash(), o.hash());
                let val = self
                    .pool
                    .remove(&key)
                    .or_else(|| new.remove(&key))
                    .unwrap_or_else(|| {
                        Restreamer::new(&self.ffmpeg_path)
                            .src_url(&r.input.srs_url())
                            .dst_url(&o.dst)
                            .run(key, self.state.clone())
                    });
                let _ = new.insert(key, val);
            }
        }

        self.pool = new
    }
}

/// [FFmpeg] process performing restreaming.
///
/// [FFmpeg]: https://ffmpeg.org
#[derive(Debug)]
pub struct Restreamer {
    /// Exact [`Command`] representing this [`Restreamer`] process.
    cmd: Command,
}

impl Restreamer {
    /// Creates new [`Restreamer`] accepting `path` to [FFmpeg] binary.
    ///
    /// Doesn't run or spawn any processes.
    ///
    /// [FFmpeg]: https://ffmpeg.org
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

    /// Sets [`Url`] to pull RTMP media stream from by this [`Restreamer`].
    #[inline]
    #[must_use]
    pub fn src_url(mut self, url: &Url) -> Self {
        let _ = self.cmd.args(&["-i", url.as_str()]);
        self
    }

    /// Sets [`Url`] to push RTMP media stream to by this [`Restreamer`].
    #[inline]
    #[must_use]
    pub fn dst_url(mut self, url: &Url) -> Self {
        let _ = self.cmd.args(&["-c", "copy", "-f", "flv", url.as_str()]);
        self
    }

    /// Runs this [`Restreamer`] spawning the actual [FFmpeg] process and
    /// returning the handle to abort it.
    ///
    /// [FFmpeg]: https://ffmpeg.org
    #[must_use]
    pub fn run(self, key: (u64, u64), state: State) -> DroppableAbortHandle {
        let (mut cmd, state_for_abort) = (self.cmd, state.clone());
        let (spawner, abort_handle) = future::abortable(async move {
            loop {
                let (cmd, state) = (&mut cmd, &state);
                let _ = AssertUnwindSafe(async move {
                    Self::set_status(Status::Initializing, key, state);

                    let state = state.clone();
                    let (to_online, abort) = future::abortable(async move {
                        time::delay_for(Duration::from_secs(5)).await;
                        Self::set_status(Status::Online, key, &state);
                    });
                    let _abort = DroppableAbortHandle(abort);

                    let process = cmd.spawn().map_err(|e| {
                        log::crit!("Cannot start FFmpeg re-streamer: {}", e)
                    })?;

                    let _ = tokio::spawn(to_online);

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

                time::delay_for(Duration::from_secs(2)).await;
            }
        });

        // Start FFmpeg re-streamer as a child process.
        let _ = tokio::spawn(spawner.map(move |_| {
            Self::set_status(Status::Offline, key, &state_for_abort)
        }));

        DroppableAbortHandle(abort_handle)
    }

    /// Sets the given [`Status`] to [`Input`]s and [`Output`]s related to the
    /// given `key` [`Restreamer`] identified in the given [`State`].
    fn set_status(status: Status, key: (u64, u64), state: &State) {
        for r in state.restreams.lock_mut().iter_mut() {
            if status != Status::Online
                && r.input.is_pull()
                && r.input.upstream_url_hash().unwrap() == key.0
                && r.input.srs_url_hash() == key.1
            {
                r.input.set_status(status);
            }

            if r.input.srs_url_hash() != key.0 {
                continue;
            }
            for o in &mut r.outputs {
                if o.hash() == key.1 {
                    o.status = status;
                }
            }
        }
    }
}

/// Abort handle of a spawned [FFmpeg] [`Restreamer`] process.
///
/// [FFmpeg]: https://ffmpeg.org
#[derive(Clone, Debug)]
pub struct DroppableAbortHandle(future::AbortHandle);

impl Drop for DroppableAbortHandle {
    #[inline]
    fn drop(&mut self) {
        self.0.abort();
    }
}
