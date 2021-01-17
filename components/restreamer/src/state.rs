//! Application state.

use std::{future::Future, panic::AssertUnwindSafe, path::Path};

use anyhow::anyhow;
use derive_more::{Display, From};
use ephyr_log::log;
use futures::{
    future::TryFutureExt as _,
    sink,
    stream::{StreamExt as _, TryStreamExt as _},
};
use futures_signals::signal::{Mutable, SignalExt as _};
use juniper::{GraphQLEnum, GraphQLObject, GraphQLScalarValue, GraphQLUnion};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use tokio::{fs, io::AsyncReadExt as _};
use url::Url;
use uuid::Uuid;
use xxhash::xxh3::xxh3_64;

use crate::{display_panic, srs};

/// Reactive application state.
///
/// Any changes to it automatically propagate to appropriate subscribers.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct State {
    /// [`argon2`] hash of password which protects access to this application's
    /// public APIs.
    pub password_hash: Mutable<Option<String>>,

    /// All [`Restream`]s performed by this application.
    pub restreams: Mutable<Vec<Restream>>,
}

impl State {
    /// Instantiates a new [`State`] reading it from a file (if any) and
    /// performing all the required inner subscriptions.
    ///
    /// # Errors
    ///
    /// If [`State`] file exists, but fails to be parsed.
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

        let state = if contents.is_empty() {
            State::default()
        } else {
            serde_json::from_slice(&contents).map_err(|e| {
                anyhow!(
                    "Failed to deserialize state from '{}' file: {}",
                    file.display(),
                    e,
                )
            })?
        };

        let (file, persisted_state) = (file.to_owned(), state.clone());
        let persist_state1 = move || {
            fs::write(
                file.clone(),
                serde_json::to_vec(&persisted_state)
                    .expect("Failed to serialize server state"),
            )
            .map_err(|e| log::error!("Failed to persist server state: {}", e))
        };
        let persist_state2 = persist_state1.clone();
        Self::on_change("persist_restreams", &state.restreams, move |_| {
            persist_state1()
        });
        Self::on_change(
            "persist_password_hash",
            &state.password_hash,
            move |_| persist_state2(),
        );

        Ok(state)
    }

    /// Subscribes the specified `hook` to changes of the [`Mutable`] `val`ue.
    ///
    /// `name` is just a convenience for describing the `hook` in logs.
    pub fn on_change<F, Fut, T>(name: &'static str, val: &Mutable<T>, hook: F)
    where
        F: FnMut(T) -> Fut + Send + 'static,
        Fut: Future + Send + 'static,
        T: Clone + PartialEq + Send + Sync + 'static,
    {
        let _ = tokio::spawn(
            AssertUnwindSafe(
                val.signal_cloned().dedupe_cloned().to_stream().then(hook),
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

    /// Adds new [`Restream`] with [`PullInput`] to this [`State`].
    ///
    /// If `update_id` is [`Some`] then updates an existing [`Restream`], if
    /// any. So, returns [`None`] if no [`Restream`] with `update_id` exists.
    #[must_use]
    pub fn add_pull_input(
        &self,
        src: Url,
        label: Option<String>,
        update_id: Option<InputId>,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();

        for r in &*restreams {
            if let Input::Pull(i) = &r.input {
                if &src == &i.src && update_id != Some(r.id) {
                    return Some(false);
                }
            }
        }

        Self::add_input_to(
            &mut *restreams,
            Input::Pull(PullInput {
                src,
                status: Status::Offline,
            }),
            label,
            update_id,
        )
    }

    /// Adds new [`Restream`] with [`PushInput`] to this [`State`].
    ///
    /// If `update_id` is [`Some`] then updates an existing [`Restream`], if
    /// any. So, returns [`None`] if no [`Restream`] with `update_id` exists.
    #[must_use]
    pub fn add_push_input(
        &self,
        name: String,
        label: Option<String>,
        update_id: Option<InputId>,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();

        for r in &*restreams {
            if let Input::Push(i) = &r.input {
                if &name == &i.name && update_id != Some(r.id) {
                    return Some(false);
                }
            }
        }

        Self::add_input_to(
            &mut *restreams,
            Input::Push(PushInput {
                name,
                status: Status::Offline,
            }),
            label,
            update_id,
        )
    }

    /// Adds new [`Restream`] with the given [`Input`] to this [`State`].
    ///
    /// If `update_id` is [`Some`] then updates an existing [`Restream`], if
    /// any. So, returns [`None`] if no [`Restream`] with `update_id` exists.
    fn add_input_to(
        restreams: &mut Vec<Restream>,
        input: Input,
        label: Option<String>,
        update_id: Option<InputId>,
    ) -> Option<bool> {
        if let Some(id) = update_id {
            let r = restreams.iter_mut().find(|r| r.id == id)?;
            if !r.input.is(&input) {
                r.input = input;
                r.srs_publisher_id = None;
                for o in &mut r.outputs {
                    o.status = Status::Offline;
                }
            }
            r.label = label;
        } else {
            restreams.push(Restream {
                id: InputId::new(),
                label,
                input,
                outputs: vec![],
                enabled: true,
                srs_publisher_id: None,
            });
        }
        Some(true)
    }

    /// Removes [`Restream`] with the given `id` from this [`State`].
    ///
    /// Returns `true` if it has been removed, or `false` if doesn't exist.
    #[must_use]
    pub fn remove_input(&self, id: InputId) -> bool {
        let mut restreams = self.restreams.lock_mut();
        let prev_len = restreams.len();
        restreams.retain(|r| r.id != id);
        restreams.len() != prev_len
    }

    /// Enables [`Restream`] with the given `id` in this [`State`].
    ///
    /// Returns `true` if it has been enabled, or `false` if it already has been
    /// enabled.
    #[must_use]
    pub fn enable_input(&self, id: InputId) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let input = restreams.iter_mut().find(|r| r.id == id)?;

        if input.enabled {
            return Some(false);
        }

        input.enabled = true;
        Some(true)
    }

    /// Disables [`Restream`] with the given `id` in this [`State`].
    ///
    /// Returns `true` if it has been disabled, or `false` if it already has
    /// been disabled.
    #[must_use]
    pub fn disable_input(&self, id: InputId) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let input = restreams.iter_mut().find(|r| r.id == id)?;

        if !input.enabled {
            return Some(false);
        }

        input.enabled = false;
        input.srs_publisher_id = None;
        Some(true)
    }

    /// Adds new [`Output`] to the specified [`Restream`] of this [`State`].
    ///
    /// Returns [`None`] if no [`Restream`] with `input_id` exists.
    #[must_use]
    pub fn add_new_output(
        &self,
        input_id: InputId,
        output_dst: Url,
        label: Option<String>,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let outputs =
            &mut restreams.iter_mut().find(|r| r.id == input_id)?.outputs;

        if outputs.iter_mut().find(|o| &o.dst == &output_dst).is_some() {
            return Some(false);
        }

        outputs.push(Output {
            id: OutputId::new(),
            dst: output_dst,
            label,
            enabled: false,
            status: Status::Offline,
        });
        Some(true)
    }

    /// Removes [`Output`] from the specified [`Restream`] of this [`State`].
    ///
    /// Returns `true` if it has been removed, or `false` if doesn't exist.
    /// Returns [`None`] if no [`Restream`] with `input_id` exists.
    #[must_use]
    pub fn remove_output(
        &self,
        input_id: InputId,
        output_id: OutputId,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let outputs =
            &mut restreams.iter_mut().find(|r| r.id == input_id)?.outputs;

        let prev_len = outputs.len();
        outputs.retain(|o| o.id != output_id);
        Some(outputs.len() != prev_len)
    }

    /// Enables [`Output`] in the specified [`Restream`] of this [`State`].
    ///
    /// Returns `true` if it has been enabled, or `false` if it already has been
    /// enabled.
    /// Returns [`None`] if no [`Restream`] with `input_id` exists.
    #[must_use]
    pub fn enable_output(
        &self,
        input_id: InputId,
        output_id: OutputId,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let output = &mut restreams
            .iter_mut()
            .find(|r| r.id == input_id)?
            .outputs
            .iter_mut()
            .find(|o| o.id == output_id)?;

        if output.enabled {
            return Some(false);
        }

        output.enabled = true;
        Some(true)
    }

    /// Disables [`Output`] in the specified [`Restream`] of this [`State`].
    ///
    /// Returns `true` if it has been disabled, or `false` if it already has
    /// been disabled.
    /// Returns [`None`] if no [`Restream`] with `input_id` exists.
    #[must_use]
    pub fn disable_output(
        &self,
        input_id: InputId,
        output_id: OutputId,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let output = &mut restreams
            .iter_mut()
            .find(|r| r.id == input_id)?
            .outputs
            .iter_mut()
            .find(|o| o.id == output_id)?;

        if !output.enabled {
            return Some(false);
        }

        output.enabled = false;
        Some(true)
    }

    /// Enables all [`Output`]s in the specified [`Restream`] of this [`State`].
    ///
    /// Returns `true` if at least one has been enabled, or `false` if all of
    /// them already have been enabled.
    /// Returns [`None`] if no [`Restream`] with `input_id` exists.
    #[must_use]
    pub fn enable_all_outputs(&self, input_id: InputId) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        Some(
            restreams
                .iter_mut()
                .find(|r| r.id == input_id)?
                .outputs
                .iter_mut()
                .filter(|o| !o.enabled)
                .fold(false, |_, o| {
                    o.enabled = true;
                    true
                }),
        )
    }

    /// Disables all [`Output`]s in the specified [`Restream`] of this
    /// [`State`].
    ///
    /// Returns `true` if at least one has been disabled, or `false` if all of
    /// them already have been disabled.
    /// Returns [`None`] if no [`Restream`] with `input_id` exists.
    #[must_use]
    pub fn disable_all_outputs(&self, input_id: InputId) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        Some(
            restreams
                .iter_mut()
                .find(|r| r.id == input_id)?
                .outputs
                .iter_mut()
                .filter(|o| o.enabled)
                .fold(false, |_, o| {
                    o.enabled = false;
                    true
                }),
        )
    }
}

/// Restream of a live RTMP stream from one `Input` to many `Output`s.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct Restream {
    /// Unique ID of this `Restream`.
    pub id: InputId,

    /// Optional label of this `Restream`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// `Input` that live RTMP stream is received from.
    pub input: Input,

    /// `Output`s that live RTMP stream is restreamed to.
    pub outputs: Vec<Output>,

    /// Indicator whether this `Restream` is enabled, so allows to receive a
    /// live RTMP stream from `Input`.
    pub enabled: bool,

    /// ID of [SRS] client who publishes the ongoing live RTMP stream to
    /// [`Input`].
    ///
    /// [SRS]: https://github.com/ossrs/srs
    #[graphql(skip)]
    #[serde(skip)]
    pub srs_publisher_id: Option<srs::ClientId>,
}

/// Source of a live RTMP stream for `Restream`.
#[derive(
    Clone, Debug, Deserialize, Eq, From, GraphQLUnion, PartialEq, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Input {
    /// Receiving a live RTMP stream from an external client.
    Push(PushInput),

    /// Pulling a live RTMP stream from a remote server.
    Pull(PullInput),
}

impl Input {
    /// Indicates whether this [`Input`] is a [`PullInput`].
    #[inline]
    #[must_use]
    pub fn is_pull(&self) -> bool {
        matches!(self, Input::Pull(_))
    }

    /// Returns [`Status`] of this [`Input`].
    #[inline]
    #[must_use]
    pub fn status(&self) -> Status {
        match self {
            Self::Pull(i) => i.status,
            Self::Push(i) => i.status,
        }
    }

    /// Sets a `new` [`Status`] of this [`Input`].
    #[inline]
    pub fn set_status(&mut self, new: Status) {
        match self {
            Self::Pull(i) => i.status = new,
            Self::Push(i) => i.status = new,
        }
    }

    /// Checks whether this [`Input`] is the same as `other` one.
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pull(a), Self::Pull(b)) => a.is(b),
            (Self::Push(a), Self::Push(b)) => a.is(b),
            _ => false,
        }
    }

    /// Calculates a unique hash of this [`Input`].
    #[inline]
    #[must_use]
    pub fn hash(&self) -> u64 {
        match self {
            Self::Pull(i) => xxh3_64(i.src.as_ref().as_bytes()),
            Self::Push(i) => xxh3_64(i.name.as_bytes()),
        }
    }

    /// Returns an URL of the remote server that this [`Input`] receives a live
    /// RTMP stream from, if any.
    #[inline]
    #[must_use]
    pub fn upstream_url(&self) -> Option<&Url> {
        if let Self::Pull(i) = self {
            Some(&i.src)
        } else {
            None
        }
    }

    /// Returns hash of an URL of the remote server that this [`Input`] receives
    /// a live RTMP stream from, if any.
    #[inline]
    #[must_use]
    pub fn upstream_url_hash(&self) -> Option<u64> {
        self.upstream_url().map(|u| xxh3_64(u.as_ref().as_bytes()))
    }

    /// Returns an URL of the local [SRS] server that a received live RTMP
    /// stream by this [`Input`] may be pulled from.
    ///
    /// [SRS]: https://github.com/ossrs/srs
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

    /// Returns hash of an URL of the local [SRS] server that a received live
    /// RTMP stream by this [`Input`] may be pulled from.
    ///
    /// [SRS]: https://github.com/ossrs/srs
    #[inline]
    #[must_use]
    pub fn srs_url_hash(&self) -> u64 {
        xxh3_64(self.srs_url().as_ref().as_bytes())
    }

    /// Checks whether the given `app` parameter of a [SRS] media stream is
    /// related to this [`Input`].
    ///
    /// [SRS]: https://github.com/ossrs/srs
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

/// `Input` pulling a live RTMP stream from a remote server.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct PullInput {
    /// URL of a live RTMP stream to be pulled.
    pub src: Url,

    /// `Status` of this `PullInput` indicating whether it performs pulling.
    #[serde(skip)]
    pub status: Status,
}

impl PullInput {
    /// Checks whether this [`PullInput`] is the same as `other` on.
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.src == &other.src
    }
}

/// `Input` receiving a live RTMP stream from a remote client.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct PushInput {
    /// Name of a live RTMP stream to expose it with for receiving and
    /// restreaming media traffic.
    pub name: String,

    /// `Status` of this `PushInput` indicating whether it receives media
    /// traffic.
    #[serde(skip)]
    pub status: Status,
}

impl PushInput {
    /// Checks whether this [`PushInput`] is the same as `other` on.
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.name == &other.name
    }
}

/// Destination that `Restream` should restream a live RTMP stream to.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct Output {
    /// Unique ID of this `Output`.
    pub id: OutputId,

    /// URL to push a live RTMP stream to.
    pub dst: Url,

    /// Optional label of this `Output`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Indicator whether this `Output` is enabled, so performs a live RTMP
    /// stream restream to destination.
    pub enabled: bool,

    /// `Status` of this `Output` indicating whether it pushes media traffic to
    /// destination.
    #[serde(skip)]
    pub status: Status,
}

impl Output {
    /// Checks whether this [`Output`] is the same as `other` on.
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.dst == &other.dst
    }

    /// Calculates a unique hash of this [`Output`].
    #[inline]
    #[must_use]
    pub fn hash(&self) -> u64 {
        xxh3_64(self.dst.as_ref().as_bytes())
    }
}

/// Status indicating what's going on in `Input` or `Output`.
#[derive(Clone, Copy, Debug, Eq, GraphQLEnum, PartialEq, SmartDefault)]
pub enum Status {
    /// Inactive, no operations are performed and no media traffic is allowed.
    #[default]
    Offline,

    /// Initializing, media traffic is allowed, but not yet flows as expected.
    Initializing,

    /// Active, all operations are performing successfully and media traffic
    /// flows as expected.
    Online,
}

/// ID of an `Input`.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Display,
    Eq,
    GraphQLScalarValue,
    PartialEq,
    Serialize,
)]
pub struct InputId(Uuid);

impl InputId {
    /// Generates new random [`InputId`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// ID of an `Output`.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Display,
    Eq,
    GraphQLScalarValue,
    PartialEq,
    Serialize,
)]
pub struct OutputId(Uuid);

impl OutputId {
    /// Generates new random [`OutputId`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
