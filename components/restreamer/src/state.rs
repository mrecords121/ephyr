//! Application state.

use std::{
    convert::TryInto, future::Future, panic::AssertUnwindSafe, path::Path,
    time::Duration,
};

use anyhow::anyhow;
use derive_more::{Display, From, Into};
use ephyr_log::log;
use futures::{
    future::TryFutureExt as _,
    sink,
    stream::{StreamExt as _, TryStreamExt as _},
};
use futures_signals::signal::{Mutable, SignalExt as _};
use juniper::{
    graphql_scalar, GraphQLEnum, GraphQLObject, GraphQLScalarValue,
    GraphQLUnion, ParseScalarResult, ParseScalarValue, ScalarValue, Value,
};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use tokio::{fs, io::AsyncReadExt as _};
use url::Url;
use uuid::Uuid;

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
        drop(tokio::spawn(
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
        ));
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
                if src == i.src && update_id != Some(r.id) {
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
                if name == i.name && update_id != Some(r.id) {
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
                id: InputId::random(),
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
        mix_with: Option<Url>,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let outputs =
            &mut restreams.iter_mut().find(|r| r.id == input_id)?.outputs;

        if outputs.iter_mut().any(|o| o.dst == output_dst) {
            return Some(false);
        }

        outputs.push(Output {
            id: OutputId::random(),
            dst: output_dst,
            label,
            volume: Volume::ORIGIN,
            mixins: mix_with
                .map(|url| {
                    vec![Mixin {
                        id: MixinId::random(),
                        src: url,
                        volume: Volume::ORIGIN,
                        delay: Delay::default(),
                    }]
                })
                .unwrap_or_default(),
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
        let output = restreams
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
        let output = restreams
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

    /// Tunes a [`Volume`] rate of the specified [`Output`] or its [`Mixin`] in
    /// this [`State`].
    ///
    /// Returns `true` if a [`Volume`] rate has been changed, or `false` if it
    /// has the same value already.
    /// Returns [`None`] if no [`Restream`] with `input_id` exists, or no
    /// [`Output`] with `output_id` exist, or no [`Mixin`] with `mixin_id`
    /// exists.
    #[must_use]
    pub fn tune_volume(
        &self,
        input_id: InputId,
        output_id: OutputId,
        mixin_id: Option<MixinId>,
        volume: Volume,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let output = restreams
            .iter_mut()
            .find(|r| r.id == input_id)?
            .outputs
            .iter_mut()
            .find(|o| o.id == output_id)?;

        let curr_volume = if let Some(id) = mixin_id {
            &mut output.mixins.iter_mut().find(|m| m.id == id)?.volume
        } else {
            &mut output.volume
        };

        if *curr_volume == volume {
            return Some(false);
        }

        *curr_volume = volume;
        Some(true)
    }

    /// Tunes a [`Delay`] of the specified [`Mixin`] in this [`State`].
    ///
    /// Returns `true` if a [`Delay`] has been changed, or `false` if it has the
    /// same value already.
    /// Returns [`None`] if no [`Restream`] with `input_id` exists, or no
    /// [`Output`] with `output_id` exist, or no [`Mixin`] with `mixin_id`
    /// exists.
    #[must_use]
    pub fn tune_delay(
        &self,
        input_id: InputId,
        output_id: OutputId,
        mixin_id: MixinId,
        delay: Delay,
    ) -> Option<bool> {
        let mut restreams = self.restreams.lock_mut();
        let mixin = restreams
            .iter_mut()
            .find(|r| r.id == input_id)?
            .outputs
            .iter_mut()
            .find(|o| o.id == output_id)?
            .mixins
            .iter_mut()
            .find(|m| m.id == mixin_id)?;

        if mixin.delay == delay {
            return Some(false);
        }

        mixin.delay = delay;
        Some(true)
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

impl Restream {
    /// Returns an URL of the remote server that this [`Restream`] receives a
    /// live stream from, if any.
    #[inline]
    #[must_use]
    pub fn upstream_url(&self) -> Option<&Url> {
        if let Input::Pull(i) = &self.input {
            Some(&i.src)
        } else {
            None
        }
    }

    /// Returns an URL of the local [SRS] server that the received live stream
    /// by this [`Restream`] may be pulled from.
    ///
    /// [SRS]: https://github.com/ossrs/srs
    #[must_use]
    pub fn srs_url(&self) -> Url {
        Url::parse(&match &self.input {
            Input::Pull(_) => {
                format!("rtmp://127.0.0.1:1935/pull_{}/in", self.id)
            }
            Input::Push(i) => format!("rtmp://127.0.0.1:1935/{}/in", i.name),
        })
        .unwrap()
    }

    /// Checks whether the given `app` parameter of a [SRS] media stream is
    /// related to this [`Restream::input`].
    ///
    /// [SRS]: https://github.com/ossrs/srs
    #[inline]
    #[must_use]
    pub fn uses_srs_app(&self, app: &str) -> bool {
        match &self.input {
            Input::Pull(_) => {
                app.starts_with("pull_") && app[5..].parse() == Ok(self.id.0)
            }
            Input::Push(i) => app == i.name,
        }
    }
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
}

/// `Input` pulling a live RTMP stream from a remote server.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct PullInput {
    /// URL of a live stream to be pulled from.
    ///
    /// At the moment only [RTMP] is supported.
    ///
    /// [RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
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
        self.src == other.src
    }
}

/// `Input` receiving a live RTMP stream from a remote client.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct PushInput {
    /// Name of a live RTMP stream to expose it with for receiving and
    /// re-streaming media traffic.
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
        self.name == other.name
    }
}

/// Destination that `Restream` should restream a live RTMP stream to.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct Output {
    /// Unique ID of this `Output`.
    pub id: OutputId,

    /// URL to push a live stream on to.
    ///
    /// At the moment only [RTMP] and [Icecast] are supported.
    ///
    /// [Icecast]: https://icecast.org
    /// [RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
    pub dst: Url,

    /// Optional label of this `Output`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Volume rate of this `Output`'s audio tracks when mixed with
    /// `Output.mixins`.
    ///
    /// Has no effect when there is no `Output.mixins`.
    #[serde(default, skip_serializing_if = "Volume::is_origin")]
    pub volume: Volume,

    /// `Mixin`s to mix this `Output` with before restream to the destination.
    ///
    /// If empty, then no mixing is performed and restreaming is as cheap as
    /// possible (just copying bytes "as is").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mixins: Vec<Mixin>,

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
        self.dst == other.dst
    }
}

/// Additional source for an `Output` to be mixed with before re-streamed to the
/// destination.
#[derive(
    Clone, Debug, Deserialize, Eq, GraphQLObject, PartialEq, Serialize,
)]
pub struct Mixin {
    /// Unique ID of this `Mixin`.
    pub id: MixinId,

    /// URL of the source to be mixed in.
    ///
    /// At the moment, only [TeamSpeak] is supported.
    ///
    /// [TeamSpeak]: https://teamspeak.com
    pub src: Url,

    /// Volume rate of this `Mixin`'s audio tracks to mix them with.
    #[serde(default, skip_serializing_if = "Volume::is_origin")]
    pub volume: Volume,

    /// Delay that this `Mixin` should wait before mixed with an `Output`.
    ///
    /// Very useful to fix de-synchronization issues and correct timings between
    /// `Mixin` and its `Output`.
    #[serde(default, skip_serializing_if = "Delay::is_zero")]
    pub delay: Delay,
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
    From,
    GraphQLScalarValue,
    Into,
    PartialEq,
    Serialize,
)]
pub struct InputId(Uuid);

impl InputId {
    /// Generates a new random [`InputId`].
    #[inline]
    #[must_use]
    pub fn random() -> Self {
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
    From,
    GraphQLScalarValue,
    Into,
    PartialEq,
    Serialize,
)]
pub struct OutputId(Uuid);

impl OutputId {
    /// Generates a new random [`OutputId`].
    #[inline]
    #[must_use]
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }
}

/// ID of a `Mixin`.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Display,
    Eq,
    From,
    GraphQLScalarValue,
    Into,
    PartialEq,
    Serialize,
)]
pub struct MixinId(Uuid);

impl MixinId {
    /// Generates a new random [`MixinId`].
    #[inline]
    #[must_use]
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Volume rate of an audio track in percents.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
    SmartDefault,
)]
pub struct Volume(#[default(Self::ORIGIN.0)] u8);

impl Volume {
    /// Maximum possible value of a [`Volume`] rate.
    pub const MAX: Volume = Volume(200);

    /// Value of a [`Volume`] rate corresponding to the original one of an audio
    /// track.
    pub const ORIGIN: Volume = Volume(100);

    /// Minimum possible value of a [`Volume`] rate. Actually, disables audio.
    pub const OFF: Volume = Volume(0);

    /// Creates a new [`Volume`] rate value if it satisfies the required
    /// invariants:
    /// - within [`Volume::OFF`] and [`Volume::MAX`] values.
    #[must_use]
    pub fn new<N: TryInto<u8>>(num: N) -> Option<Self> {
        let num = num.try_into().ok()?;
        if (Self::OFF.0..=Self::MAX.0).contains(&num) {
            Some(Self(num))
        } else {
            None
        }
    }

    /// Displays this [`Volume`] as a fraction of `1`, i.e. `100%` as `1`, `50%`
    /// as `0.50`, and so on.
    #[must_use]
    pub fn display_as_fraction(self) -> String {
        format!("{}.{:02}", self.0 / 100, self.0 % 100)
    }

    /// Indicates whether this [`Volume`] rate value corresponds is the
    /// [`Volume::ORIGIN`]al one.
    #[allow(clippy::trivially_copy_pass_by_ref)] // required for `serde`
    #[inline]
    #[must_use]
    pub fn is_origin(&self) -> bool {
        *self == Self::ORIGIN
    }
}

/// Type a volume rate of audio track in percents.
///
/// It's values are always within range of `0` and `200` (inclusively).
///
/// `0` means disabled audio.
#[graphql_scalar]
impl<S> GraphQLScalar for Volume
where
    S: ScalarValue,
{
    fn resolve(&self) -> Value {
        Value::scalar(i32::from(self.0))
    }

    fn from_input_value(v: &InputValue) -> Option<Volume> {
        v.as_scalar()
            .and_then(ScalarValue::as_int)
            .and_then(Volume::new)
    }

    fn from_str(value: ScalarToken<'_>) -> ParseScalarResult<'_, S> {
        <String as ParseScalarValue<S>>::from_str(value)
    }
}

/// Delay of a [`Mixin`] being mixed with an [`Output`].
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Default,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub struct Delay(Duration);

impl Delay {
    /// Creates a new [`Delay`] out of the given milliseconds.
    #[inline]
    #[must_use]
    pub fn from_millis<N: TryInto<u64>>(millis: N) -> Option<Self> {
        millis
            .try_into()
            .ok()
            .map(|m| Self(Duration::from_millis(m)))
    }

    /// Returns milliseconds of this [`Delay`].
    #[inline]
    #[must_use]
    pub fn as_millis(&self) -> i32 {
        self.0.as_millis().try_into().unwrap()
    }

    /// Indicates whether this [`Delay`] introduces no actual delay.
    #[inline]
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0 == Duration::default()
    }
}

/// Type of a `Mixin` delay in milliseconds.
///
/// Negative values are not allowed.
#[graphql_scalar]
impl<S> GraphQLScalar for Delay
where
    S: ScalarValue,
{
    fn resolve(&self) -> Value {
        Value::scalar(self.as_millis())
    }

    fn from_input_value(v: &InputValue) -> Option<Delay> {
        v.as_scalar()
            .and_then(ScalarValue::as_int)
            .and_then(Delay::from_millis)
    }

    fn from_str(value: ScalarToken<'_>) -> ParseScalarResult<'_, S> {
        <String as ParseScalarValue<S>>::from_str(value)
    }
}

#[cfg(test)]
mod volume_spec {
    use super::Volume;

    #[test]
    fn displays_as_fraction() {
        for (input, expected) in &[
            (1, "0.01"),
            (10, "0.10"),
            (200, "2.00"),
            (107, "1.07"),
            (170, "1.70"),
        ] {
            let actual = Volume::new(*input).unwrap().display_as_fraction();
            assert_eq!(&actual, *expected);
        }
    }
}
