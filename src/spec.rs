//! Mixing specification schema.

use std::{collections::HashMap, convert::TryInto as _, time::Duration};

use config::{Config, ConfigError, Environment, FileFormat};
use decimal::Decimal;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use url::Url;
use validator::{Validate, ValidationError, ValidationErrors};
use validator_derive::Validate;

/// Specification of how the mixing should be performed.
///
/// [`Spec`] consists of applications, and each application consists of
/// [`Mixer`]s. Therefore, once program is invoked for some application it will
/// run all its [`Mixer`]s.
///
/// If application contains no [`Mixer`]s the program will still run as no-op
/// until is killed.
///
/// # Example
///
/// ```
/// # use ephyr::Spec;
/// # use validator::Validate as _;
/// let spec = r#"{"spec": {
///   "input": {
///     "en": {
///       "src": {
///         "org": {
///           "url": "rtmp://127.0.0.1/[app]/[stream]",
///           "volume": 1,
///           "zmq": {"port": 60010}
///         },
///         "trn": {
///           "url": "ts://127.0.0.1:9067/translation/en",
///           "delay": "300ms",
///           "volume": 1.5,
///           "zmq": {"port": 60011}
///         }
///       },
///       "dest": {
///         "output": {"url": "rtmp://127.0.0.1/output/en_[stream]"},
///         "youtube": {"url": "rtmp://rtmp.youtube.com/stream-key"}
///       }
///     },
///     "zh": {
///       "src": {
///         "org": {
///           "url": "rtmp://127.0.0.1/[app]/[stream]",
///           "volume": 1,
///           "zmq": {"port": 60020}
///         },
///         "trn": {
///           "url": "ts://127.0.0.1:9067/translation/zh",
///           "delay": "500ms",
///           "volume": 1.5,
///           "zmq": {"port": 60021}
///         }
///       },
///       "dest": {
///         "output": {"url": "rtmp://127.0.0.1/output/zh_[stream]"},
///         "facecast": {"url": "rtmp://rtmp.facecast.io/stream-key"}
///       }
///     }
///   },
///   "output": {}
/// }}"#;
///
/// let spec: Spec =
///     serde_json::from_str(spec).expect("Deserialization failed");
/// assert!(spec.validate().is_ok());
/// ```
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Spec {
    /// Applications with their [`Mixer`]s.
    ///
    /// Defaults to empty.
    pub spec: HashMap<String, HashMap<String, Mixer>>,
}

impl Validate for Spec {
    /// Performs nested validation of [`Spec`].
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut out = Ok(());
        for (_, app) in &self.spec {
            for (_, mixer) in app {
                out =
                    ValidationErrors::merge(out, "app/mixer", mixer.validate());
            }
        }
        out
    }
}

impl Spec {
    /// Parses [`Spec`] from all possible sources and evaluates its values.
    ///
    /// # Errors
    ///
    /// - If [`Spec`] fails to be parsed from file or environment variables.
    /// - If [`Spec`] fails its validation.
    pub fn parse() -> Result<Self, ConfigError> {
        let mut spec = Config::new();
        spec.merge(
            config::File::with_name("spec.json")
                .format(FileFormat::Json)
                .required(false),
        )?
        .merge(Environment::with_prefix("").separator("."))?;

        let spec: Self = spec.try_into()?;

        spec.validate()
            .map_err(|e| ConfigError::Foreign(Box::new(e)))?;

        Ok(spec)
    }
}

/// Spec of a mixer that mixes [`Source`]s and feeds the result to
/// [`Destination`]s.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Mixer {
    /// [`Source`]s to be mixed.
    ///
    /// Defaults to empty.
    pub src: HashMap<String, Source>,

    /// [`Destination`]s to be fed with the mixing result.
    ///
    /// Defaults to empty.
    pub dest: HashMap<String, Destination>,
}

impl Validate for Mixer {
    /// Validates whether the given [`Mixer`] has at least one [`Source`] for
    /// mixing and at least one [`Destination`] to feed the result into.
    /// And performs nested validation.
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut out = Ok(());
        if self.src.is_empty() {
            let mut errs = ValidationErrors::new();
            errs.add(
                "src",
                ValidationError::new(
                    "At least one source for mixing should be specified",
                ),
            );
            out = ValidationErrors::merge(out, "src", Err(errs));
        }
        for (_, s) in &self.src {
            out = ValidationErrors::merge(out, "src", s.validate());
        }
        if self.dest.is_empty() {
            let mut errs = ValidationErrors::new();
            errs.add(
                "src",
                ValidationError::new(
                    "At least one destination for mixing result should be \
                 specified",
                ),
            );
            out = ValidationErrors::merge(out, "dest", Err(errs));
        }
        for (_, d) in &self.dest {
            out = ValidationErrors::merge(out, "dest", d.validate());
        }
        out
    }
}

/// Spec of a source to get audio/video channels from, along with their mixing
/// options.
#[derive(
    Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault, Validate,
)]
#[serde(default)]
pub struct Source {
    /// URL of this [`Source`] to capture audio/video channels from.
    ///
    /// `[app]` and `[stream]` placeholders may be used to specify dynamic
    /// runtime values instead of static hardcoded ones.
    ///
    /// Defaults to `rtmp://127.0.0.1/[app]/[stream]` (ingest RTMP stream).
    #[default(Url::parse("rtmp://127.0.0.1/[app]/[stream]").unwrap())]
    #[validate(custom = "is_rtmp_or_ts")]
    pub url: Url,

    /// [`adelay` filter][1] duration to apply to the audio channels of this
    /// [`Source`].
    ///
    /// Defaults to `0` (no delay).
    ///
    /// [1]: https://ffmpeg.org/ffmpeg-filters.html#adelay
    #[serde(with = "serde_humantime")]
    #[validate(custom = "has_milliseconds_precision")]
    pub delay: Duration,

    /// [`volume` filter][1] ratio to apply to the audio channels of this
    /// [`Source`].
    ///
    /// Defaults to `1` (100% original volume).
    ///
    /// [1]: https://ffmpeg.org/ffmpeg-filters.html#volume
    #[default(1.into())]
    pub volume: Decimal,

    /// [`azmq` filter][1] to apply to the audio channels of this [`Source`].
    ///
    /// [1]: https://ffmpeg.org/ffmpeg-filters.html#zmq_002c-azmq
    pub zmq: AzmqFilter,
}

/// Validates whether the given [`Url`] has `rtmp` or `ts` scheme.
fn is_rtmp_or_ts(url: &Url) -> Result<(), ValidationError> {
    if !matches!(url.scheme(), "rtmp" | "ts") {
        return Err(ValidationError::new(
            "Only RTMP and TeamSpeak sources are supported",
        ));
    }
    Ok(())
}

/// Validates whether the given [`Duration`] has at most milliseconds precision.
fn has_milliseconds_precision(dur: &Duration) -> Result<(), ValidationError> {
    if dur.subsec_nanos() % 1_000_000 != 0 {
        return Err(ValidationError::new(
            "At most milliseconds precision is allowed",
        ));
    }
    Ok(())
}

/// Spec of [`azmq` filter][1] options.
///
/// [1]: https://ffmpeg.org/ffmpeg-filters.html#zmq_002c-azmq
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault,
)]
#[serde(default)]
pub struct AzmqFilter {
    /// Port of [`azmq` filter][1] to listen ZeroMQ commands on.
    ///
    /// Defaults to `5555`.
    ///
    /// [1]: https://ffmpeg.org/ffmpeg-filters.html#zmq_002c-azmq
    #[default = 5555]
    pub port: u16,
}

/// Spec of a destination to push the mixed [`Source`]s to.
#[derive(
    Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault, Validate,
)]
#[serde(default)]
pub struct Destination {
    /// URL of this [`Destination`] to push data to.
    ///
    /// `[app]` and `[stream]` placeholders may be used to specify dynamic
    /// runtime values instead of static hardcoded ones.
    ///
    /// Defaults to `rtmp://127.0.0.1/[app]_out/[stream]` (out RTMP stream).
    #[default(Url::parse("rtmp://127.0.0.1/[app]_out/[stream]").unwrap())]
    #[validate(custom = "is_rtmp_only")]
    pub url: Url,
}

/// Validates whether the given [`Url`] has `rtmp` scheme.
fn is_rtmp_only(url: &Url) -> Result<(), ValidationError> {
    if url.scheme() != "rtmp" {
        return Err(ValidationError::new(
            "Only RTMP destinations are supported",
        ));
    }
    Ok(())
}
