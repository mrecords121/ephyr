use std::{collections::HashMap, rc::Rc, time::Duration};

use decimal::Decimal;
use derive_more::{Display, From, Into};
use futures_signals::{signal::Mutable, signal_vec::MutableVec};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct State {
    pub active_stream: Mutable<usize>,
    pub streams: Rc<MutableVec<Stream>>,
}

impl State {
    pub fn from_seed() -> Self {
        serde_json::from_str(
            r#"{
          "active_stream": 0,
          "streams": [{
            "name": "love",
            "active_preset": 2,
            "presets": [{
              "name": "Original",
              "volume": []
            }, {
              "name": "English",
              "volume": []
            }, {
              "name": "Spanish",
              "volume": []
            }],
            "mixers": [{
              "name": "en",
              "sources": [],
              "destinations": [],
              "is_online": true
            }, {
              "name": "es",
              "sources": [],
              "destinations": [],
              "is_online": false
            }, {
              "name": "itttt",
              "sources": [],
              "destinations": [],
              "is_online": false
            }, {
              "name": "fr",
              "sources": [],
              "destinations": [],
              "is_online": true
            }, {
              "name": "de",
              "sources": [],
              "destinations": [],
              "is_online": false
            }]
          }, {
            "name": "life_is_beautiful",
            "active_preset": 1,
            "presets": [{
              "name": "Always",
              "volume": []
            }, {
              "name": "And now",
              "volume": []
            }],
            "mixers": []
          }, {
            "name": "trance_radio",
            "active_preset": 2,
            "presets": [{
              "name": "Trance",
              "volume": []
            }, {
              "name": "Silence",
              "volume": []
            }, {
              "name": "Mood",
              "volume": []
            }],
            "mixers": []
          }]
        }"#,
        )
        .expect("Failed to deserialize State from seed")
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Stream {
    pub name: Mutable<StreamName>,
    pub active_preset: Mutable<usize>,
    pub presets: Rc<MutableVec<Preset>>,
    pub mixers: Rc<MutableVec<Mixer>>,
}

#[derive(
    Clone, Debug, Deserialize, Display, Eq, From, Hash, Into, PartialEq,
)]
pub struct StreamName(String);

#[derive(Clone, Debug, Deserialize)]
pub struct Preset {
    pub name: Mutable<PresetName>,
    pub volume: Rc<MutableVec<(MixerName, HashMap<SourceName, Volume>)>>,
}

#[derive(
    Clone, Debug, Deserialize, Display, Eq, From, Hash, Into, PartialEq,
)]
pub struct PresetName(String);

#[derive(Clone, Debug, Deserialize)]
pub struct Mixer {
    pub name: Mutable<MixerName>,
    pub sources: Rc<MutableVec<Source>>,
    pub destinations: Rc<MutableVec<Destination>>,
    pub is_online: Mutable<bool>,
}

#[derive(
    Clone, Debug, Deserialize, Display, Eq, From, Hash, Into, PartialEq,
)]
pub struct MixerName(String);

#[derive(Clone, Debug, Deserialize)]
pub struct Source {
    pub name: SourceName,
    pub url: String,
    pub delay: Duration,
    pub volume: Volume,
    pub zmq_port: u16,
}

#[derive(
    Clone, Debug, Deserialize, Display, Eq, From, Hash, Into, PartialEq,
)]
pub struct SourceName(String);

#[derive(Clone, Debug, Deserialize)]
pub struct Destination {
    pub name: DestinationName,
    pub url: String,
}

#[derive(
    Clone, Debug, Deserialize, Display, Eq, From, Hash, Into, PartialEq,
)]
pub struct DestinationName(String);

// TODO: deserialize with validation
#[derive(Clone, Copy, Deserialize, Debug, Display, Eq, PartialEq)]
pub struct Volume(Decimal);

impl Volume {
    pub fn new<D: Into<Decimal>>(num: D) -> Option<Self> {
        let num = num.into();
        if Self::validate(num) {
            Some(Self(num))
        } else {
            None
        }
    }

    #[inline]
    pub fn validate(num: Decimal) -> bool {
        num >= 0.into() && num <= 2.into()
    }
}
