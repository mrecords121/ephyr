use std::{
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::anyhow;
use askama::Template;
use ephyr_log::log;
use futures::future::{self, FutureExt as _, TryFutureExt as _};
use tokio::{fs, process::Command};

use crate::{api, display_panic, state};

#[derive(Clone, Debug)]
pub struct Server {
    conf_path: PathBuf,
    process: Arc<ServerProcess>,
}

impl Server {
    #[must_use]
    pub async fn try_new<P: AsRef<Path>>(
        workdir: P,
        cfg: &Config,
    ) -> Result<Self, anyhow::Error> {
        let workdir = workdir.as_ref();
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
            .arg("-c")
            .arg(&conf_path);

        let (spawner, abort_handle) = future::abortable(async move {
            loop {
                let cmd = &mut cmd;
                let _ = AssertUnwindSafe(async move {
                    let process = cmd.spawn().map_err(|e| {
                        log::crit!("Cannot start SRS server: {}", e)
                    })?;
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
                .unwrap_or_else(|_: ()| ())
                .catch_unwind()
                .await
                .map_err(|p| {
                    log::crit!(
                        "Panicked while spawning/observing SRS server: {}",
                        display_panic(&p),
                    );
                });
            }
        });

        let srv = Self {
            conf_path,
            process: Arc::new(ServerProcess { abort_handle }),
        };

        // Pre-create SRS conf file.
        srv.refresh(cfg).await?;

        // Start SRS server as a child process.
        let _ = tokio::spawn(spawner);

        Ok(srv)
    }

    pub async fn refresh(&self, cfg: &Config) -> anyhow::Result<()> {
        // SRS server reloads automatically on its conf file changes.
        fs::write(
            &self.conf_path,
            cfg.render().map_err(|e| {
                anyhow!("Failed to render SRS config from template: {}", e)
            })?,
        )
        .await
        .map_err(|e| anyhow!("Failed to write SRS config file: {}", e))
    }

    pub async fn kickoff_unnecessary_publishers(
        restreams: Vec<state::Restream>,
    ) {
        let _ = future::join_all(restreams.iter().filter_map(|r| {
            if r.enabled || r.srs_publisher_id.is_none() {
                return None;
            }
            let client_id = r.srs_publisher_id.unwrap();
            Some(api::srs::Client::kickoff_client(client_id).map_err(
                move |e| {
                    log::warn!(
                        "Failed to kickoff client {} from SRS: {}",
                        client_id,
                        e,
                    )
                },
            ))
        }))
        .await;
    }
}

#[derive(Clone, Debug)]
struct ServerProcess {
    abort_handle: future::AbortHandle,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

#[derive(Clone, Debug, Template)]
#[template(path = "restreamer.srs.conf.j2", escape = "none")]
pub struct Config {
    pub callback_port: u16,
    pub ffmpeg_path: String,
    pub restreams: Vec<state::Restream>,
}
