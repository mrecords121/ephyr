use std::{
    convert::TryInto as _,
    panic::AssertUnwindSafe,
    path::Path,
    process::Stdio,
    sync::{Arc, Mutex},
};

use ephyr_log::log;
use futures::future::{self, FutureExt as _, TryFutureExt as _};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use tokio::{process::Command, task::JoinHandle};

use crate::{display_panic, register_async_drop};

#[derive(Clone, Debug)]
pub struct Server {
    process: Arc<ServerProcess>,
    id: Arc<Mutex<Option<u32>>>,
}

#[derive(Debug)]
struct ServerProcess {
    abort_handle: future::AbortHandle,
    spawn_handle: Option<JoinHandle<Result<(), future::Aborted>>>,
}

impl Server {
    #[must_use]
    pub fn new(workdir: &Path) -> Self {
        let mut bin_path = workdir.to_path_buf();
        bin_path.push("objs/srs");
        let mut conf_path = workdir.to_path_buf();
        conf_path.push("conf/srs.conf");
        let mut cmd = Command::new(bin_path);
        let _ = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .current_dir(workdir)
            .arg("-s")
            .arg(conf_path);

        let id = Arc::new(Mutex::new(None));

        let process_id = id.clone();
        let (process, abort_handle) = future::abortable(async move {
            loop {
                let cmd = &mut cmd;
                let process_id = &process_id;
                let _ = AssertUnwindSafe(async move {
                    let process = cmd.spawn().map_err(|e| {
                        log::crit!("Cannot start SRS server: {}", e)
                    })?;
                    {
                        *process_id.lock().unwrap() = Some(process.id());
                    }
                    let out =
                        process.wait_with_output().await.map_err(|e| {
                            log::crit!("Failed to observe SRS server: {}", e)
                        })?;
                    log::crit!(
                        "SRS server stopped with exit code: {}",
                        out.status,
                    );
                    Ok(())
                })
                .unwrap_or_else(|_: ()| {
                    *process_id.lock().unwrap() = None;
                })
                .catch_unwind()
                .await
                .map_err(|p| {
                    log::error!(
                        "Panicked while spawning/observing SRS server: {}",
                        display_panic(&p),
                    );
                });
            }
        });

        Self {
            process: Arc::new(ServerProcess {
                abort_handle,
                spawn_handle: Some(tokio::spawn(process)),
            }),
            id,
        }
    }

    pub fn reload(&self) -> nix::Result<()> {
        if let Some(process_id) = { *self.id.lock().unwrap() } {
            signal::kill(
                Pid::from_raw(process_id.try_into().unwrap()),
                Signal::SIGHUP,
            )
        } else {
            Ok(())
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        register_async_drop(self.spawn_handle.take().unwrap());
        self.abort_handle.abort();
    }
}
