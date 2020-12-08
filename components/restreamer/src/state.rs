use std::{future::Future, panic::AssertUnwindSafe, path::Path};

use anyhow::anyhow;
use derive_more::{Deref, From};
use ephyr_log::log;
use futures::{
    future::TryFutureExt as _,
    sink,
    stream::{StreamExt as _, TryStreamExt as _},
};
use futures_signals::signal::{Mutable, SignalExt as _};
use juniper::{GraphQLEnum, GraphQLObject, GraphQLUnion};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use tokio::{fs, io::AsyncReadExt as _};
use url::Url;
use xxhash::xxh3::xxh3_64;

use crate::display_panic;

#[derive(Clone, Debug, Deref)]
pub struct State(Mutable<Vec<Restream>>);

impl State {
    pub async fn try_new<P: AsRef<Path>>(
        file: P,
    ) -> Result<Self, anyhow::Error> {
        let file = file.as_ref();

        let mut contents = vec![];
        let _ = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .read(true)
            .open(&file)
            .await
            .map_err(|e| {
                anyhow!("Failed to open '{}' file: {}", file.display(), e)
            })?
            .read_to_end(&mut contents)
            .await
            .map_err(|e| {
                anyhow!("Failed to read '{}' file: {}", file.display(), e)
            })?;

        let state = Self(Mutable::new(if contents.is_empty() {
            vec![]
        } else {
            serde_json::from_slice(&contents).map_err(|e| {
                anyhow!(
                    "Failed to deserialize state from '{}' file: {}",
                    file.display(),
                    e,
                )
            })?
        }));

        let file = file.to_owned();
        state.on_change("persist_state", move |restreams| {
            fs::write(
                file.clone(),
                serde_json::to_vec(&restreams)
                    .expect("Failed to serialize server state"),
            )
            .map_err(|e| log::error!("Failed to persist server state: {}", e))
        });

        Ok(state)
    }

    pub fn on_change<F, Fut>(&self, name: &'static str, hook: F)
    where
        F: FnMut(Vec<Restream>) -> Fut + Send + 'static,
        Fut: Future + Send + 'static,
    {
        let _ = tokio::spawn(
            AssertUnwindSafe(
                self.0
                    .signal_cloned()
                    .dedupe_cloned()
                    .to_stream()
                    .then(hook),
            )
            .catch_unwind()
            .map_err(move |p| {
                log::crit!(
                    "Panicked executing `{}` hook of state: {}",
                    name,
                    display_panic(&p),
                )
            })
            .map(|_| Ok(()))
            .forward(sink::drain()),
        );
    }

    #[must_use]
    pub fn add_new_pull_input(&self, src: Url) -> bool {
        let mut restreams = self.0.lock_mut();

        if restreams
            .iter_mut()
            .find(|r| r.input.is_pull() && r.input.has_id(src.as_str()))
            .is_some()
        {
            return false;
        }

        restreams.push(Restream {
            input: PullInput {
                src,
                status: Status::Offline,
            }
            .into(),
            outputs: vec![],
            enabled: true,
            srs_publisher_id: None,
        });
        true
    }

    #[must_use]
    pub fn add_new_push_input(&self, name: String) -> bool {
        let mut restreams = self.0.lock_mut();

        if restreams
            .iter_mut()
            .find(|r| !r.input.is_pull() && r.input.has_id(&name))
            .is_some()
        {
            return false;
        }

        restreams.push(Restream {
            input: PushInput {
                name,
                status: Status::Offline,
            }
            .into(),
            outputs: vec![],
            enabled: true,
            srs_publisher_id: None,
        });
        true
    }

    #[must_use]
    pub fn remove_input(&self, input_id: &str) -> bool {
        let mut restreams = self.0.lock_mut();
        let prev_len = restreams.len();
        restreams.retain(|r| !r.input.has_id(input_id));
        restreams.len() != prev_len
    }

    #[must_use]
    pub fn enable_input(&self, input_id: &str) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let input = restreams.iter_mut().find(|r| r.input.has_id(input_id))?;

        if input.enabled {
            return Some(false);
        }

        input.enabled = true;
        Some(true)
    }

    #[must_use]
    pub fn disable_input(&self, input_id: &str) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let input = restreams.iter_mut().find(|r| r.input.has_id(input_id))?;

        if !input.enabled {
            return Some(false);
        }

        input.enabled = false;
        Some(true)
    }

    #[must_use]
    pub fn add_new_output(
        &self,
        input_id: &str,
        output_dst: Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let outputs = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs;

        if outputs.iter_mut().find(|o| &o.dst == &output_dst).is_some() {
            return Some(false);
        }

        outputs.push(Output {
            dst: output_dst,
            enabled: false,
            status: Status::Offline,
        });
        Some(true)
    }

    #[must_use]
    pub fn remove_output(
        &self,
        input_id: &str,
        output_dst: &Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let outputs = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs;

        let prev_len = outputs.len();
        outputs.retain(|o| &o.dst != output_dst);
        Some(outputs.len() != prev_len)
    }

    #[must_use]
    pub fn enable_output(
        &self,
        input_id: &str,
        output_dst: &Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let output = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs
            .iter_mut()
            .find(|o| &o.dst == output_dst)?;

        if output.enabled {
            return Some(false);
        }

        output.enabled = true;
        Some(true)
    }

    #[must_use]
    pub fn disable_output(
        &self,
        input_id: &str,
        output_dst: &Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let output = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs
            .iter_mut()
            .find(|o| &o.dst == output_dst)?;

        if !output.enabled {
            return Some(false);
        }

        output.enabled = false;
        Some(true)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize)]
pub struct Restream {
    pub input: Input,
    pub outputs: Vec<Output>,
    pub enabled: bool,
    #[graphql(skip)]
    #[serde(skip)]
    pub srs_publisher_id: Option<u32>,
}

#[derive(
    Clone, Debug, Deserialize, Eq, From, GraphQLUnion, PartialEq, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Input {
    Push(PushInput),
    Pull(PullInput),
}

impl Input {
    #[inline]
    #[must_use]
    pub fn is_pull(&self) -> bool {
        matches!(self, Input::Pull(_))
    }

    #[inline]
    #[must_use]
    pub fn status(&self) -> Status {
        match self {
            Self::Pull(i) => i.status,
            Self::Push(i) => i.status,
        }
    }

    #[inline]
    pub fn set_status(&mut self, new: Status) {
        match self {
            Self::Pull(i) => i.status = new,
            Self::Push(i) => i.status = new,
        }
    }

    #[inline]
    #[must_use]
    pub fn has_id(&self, id: &str) -> bool {
        match self {
            Self::Pull(i) => i.src.as_str() == id,
            Self::Push(i) => i.name.as_str() == id,
        }
    }

    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pull(a), Self::Pull(b)) => a.is(b),
            (Self::Push(a), Self::Push(b)) => a.is(b),
            _ => false,
        }
    }

    #[inline]
    #[must_use]
    pub fn hash(&self) -> u64 {
        match self {
            Self::Pull(i) => xxh3_64(i.src.as_ref().as_bytes()),
            Self::Push(i) => xxh3_64(i.name.as_bytes()),
        }
    }

    #[inline]
    #[must_use]
    pub fn upstream_url(&self) -> Option<&Url> {
        if let Self::Pull(i) = self {
            Some(&i.src)
        } else {
            None
        }
    }

    #[must_use]
    pub fn srs_url(&self) -> Url {
        Url::parse(&match self {
            Self::Pull(_) => {
                format!("rtmp://127.0.0.1:1935/pull_{}/in", self.hash())
            }
            Self::Push(i) => format!("rtmp://127.0.0.1:1935/{}/in", i.name),
        })
        .unwrap()
    }

    #[inline]
    #[must_use]
    pub fn uses_srs_app(&self, app: &str) -> bool {
        match self {
            Self::Pull(_) => {
                app.starts_with("pull_") && app[5..].parse() == Ok(self.hash())
            }
            Self::Push(i) => app == &i.name,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize)]
pub struct PullInput {
    pub src: Url,
    #[serde(skip)]
    pub status: Status,
}

impl PullInput {
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.src == &other.src
    }
}

#[derive(Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize)]
pub struct PushInput {
    pub name: String,
    #[serde(skip)]
    pub status: Status,
}

impl PushInput {
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.name == &other.name
    }
}

#[derive(Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize)]
pub struct Output {
    pub dst: Url,
    pub enabled: bool,
    #[serde(skip)]
    pub status: Status,
}

impl Output {
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.dst == &other.dst
    }

    #[inline]
    #[must_use]
    pub fn hash(&self) -> u64 {
        xxh3_64(self.dst.as_ref().as_bytes())
    }
}

#[derive(Clone, Copy, Debug, Eq, GraphQLEnum, PartialEq, SmartDefault)]
pub enum Status {
    #[default]
    Offline,
    Initializing,
    Online,
}
