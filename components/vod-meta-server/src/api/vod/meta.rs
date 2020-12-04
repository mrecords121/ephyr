//! Definitions of API provided by this application's [VOD] meta server.
//!
//! [VOD]: https://en.wikipedia.org/wiki/Video_on_demand

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use chrono::{FixedOffset as TimeZone, Weekday};
use ephyr_serde::{timelike, timezone};
use isolang::Language;
use serde::{Deserialize, Serialize};
use url::Url;

pub use crate::vod::meta::state::{PlaylistSlug, Resolution, SegmentDuration};

/// Set of [`Playlist`]s to be provided th the server.
pub type Request = HashMap<PlaylistSlug, Playlist>;

/// Playlist of [`Clip`]s to be played for some audience.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Playlist {
    /// Human-readable title of this [`Playlist`].
    pub title: String,

    /// Language of the audience this [`Playlist`] is intended for.
    pub lang: Language,

    /// Timezone of the audience this [`Playlist`] is intended for.
    ///
    /// [`Playlist::clips`] are scheduled in this timezone according to the
    /// provided [`Weekday`]s.
    #[serde(with = "timezone")]
    pub tz: TimeZone,

    /// Optional duration of segments to serve [`Playlist`]'s [`Clip`]s with.
    ///
    /// All [`Clip`]s in [`Playlist`] are mandatory cut to this duration
    /// segments when are served. That's why [`Clip`]'s duration should divide
    /// on [`SegmentDuration`] without any fractions.
    ///
    /// If not specified then default value of [`SegmentDuration`] will be used
    /// for this [`Playlist`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_duration: Option<SegmentDuration>,

    /// Set of [`Clip`]'s [`Resolution`]s that should be provided by this
    /// [`Playlist`].
    ///
    /// If not specified or empty then all available [`Clip`]'s [`Resolution`]s
    /// will be used.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub resolutions: HashSet<Resolution>,

    /// [`Clip`]s which form this [`Playlist`], distributed by [`Weekday`]s.
    ///
    /// The total duration of all [`Clip`]s in the one [`Weekday`] hasn't to be
    /// exactly 24 hours, but cannot be more than that, and hast to be a
    /// fraction of 24 hours. This is this dictated by the necessity to
    /// correctly loop the [`Weekday`]'s schedule to fill the whole 24 hours.
    ///
    /// All the [`Clip`]s provided for a single [`Weekday`] will be scheduled
    /// one after another sequentially, in the order they were provided, and
    /// without any gaps between them.
    pub clips: HashMap<Weekday, Vec<Clip>>,
}

/// Clip in a [`Playlist`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Clip {
    /// [YouTube]'s full URL of this [`Clip`] (not shortened).
    ///
    /// [YouTube]: https://youtube.com
    pub url: Url,

    /// Human-readable title of this [`Clip`].
    pub title: String,

    /// Starting timing position to play this [`Clip`] from.
    #[serde(with = "timelike")]
    pub from: Duration,

    /// Finish timing position to play this [`Clip`] until.
    ///
    /// Obviously, should be always greater than [`Clip::from`] for at least
    /// 1 second.
    #[serde(with = "timelike")]
    pub to: Duration,
}

#[cfg(test)]
mod spec {
    use super::*;

    mod playlist {
        use super::*;

        #[test]
        fn deserializes_valid() {
            const RAW_JSON: &str = r#"{
              "title": "Передачи с Игорем Михайловичем",
              "lang": "rus",
              "tz": "+03:00",
              "segment_duration": "6s",
              "resolutions": [720, 360],
              "clips": {
                "mon": [{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "1:51:26"
                }, {
                  "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                  "title": "ПРАВДА ЖИЗНИ",
                  "from": "00:00:00",
                  "to": "1:00:00"
                }]
              }
            }"#;

            let res = serde_json::from_str::<Playlist>(RAW_JSON);
            assert!(res.is_ok(), "failed to deserialize: {}", res.unwrap_err());
        }

        #[test]
        fn fails_deserialize_invalid() {
            for json in &[
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "dddd",
                  "clips": {
                    "mon": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "1:51:26"
                    }]
                  }
                }"#,
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": null,
                  "tz": "+03:00",
                  "clips": {
                    "mon": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "1:51:26"
                    }]
                  }
                }"#,
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "segment_duration": "3s",
                  "clips": {
                    "mon": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "1:51:26"
                    }]
                  }
                }"#,
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "fin": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "1:51:26"
                    }]
                  }
                }"#,
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "resolutions": [34],
                  "clips": {
                    "mon": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "1:51:26"
                    }]
                  }
                }"#,
            ] {
                let res = serde_json::from_str::<Playlist>(*json);
                assert!(res.is_err(), "should not deserialize: {}", json);
            }
        }
    }

    mod clip {
        use super::*;

        #[test]
        fn deserializes_valid() {
            const RAW_JSON: &str = r#"{
              "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
              "title": "Круг Жизни",
              "from": "00:00:00",
              "to": "1:51:26"
            }"#;

            let res = serde_json::from_str::<Clip>(RAW_JSON);
            assert!(res.is_ok(), "failed to deserialize: {}", res.unwrap_err());
        }

        #[test]
        fn fails_deserialize_invalid() {
            for json in &[
                r#"{
                  "url": null,
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "1:51:26"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": 123,
                  "from": "00:00:00",
                  "to": "1:51:26"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "ababa",
                  "to": "1:51:26"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "galamaga"
                }"#,
            ] {
                let res = serde_json::from_str::<Clip>(*json);
                assert!(res.is_err(), "should not deserialize: {}", json);
            }
        }
    }
}
