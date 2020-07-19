//! Inner state of a [VOD] server.
//!
//! It holds a collection of [`Playlist`]s, each of which has a week-based
//! schedule of [`Clip`]s (each weekday has its own collection of [`Clip`]s).
//!
//! The total duration of all [`Clip`]s in the one weekday hasn't to be exactly
//! 24 hours, but cannot be more than that. Also, 24 hours should divide on that
//! duration without any fractions. This is this dictated by the necessity to
//! correctly loop the weekday's playlist to fill the whole 24 hours.
//!
//! [VOD]: https://en.wikipedia.org/wiki/Video_on_demand
//! [`Clip`]: crate::vod::state::Clip
//! [`Playlist`]: crate::vod::state::Playlist

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use chrono::{FixedOffset as TimeZone, Weekday};
use isolang::Language;
use mime::Mime;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use url::Url;

use crate::util::serde::{timelike, timezone};

/// State of a [VOD] server, representing a set of [`Playlist`]s for different
/// audiences.
///
/// [VOD]: https://en.wikipedia.org/wiki/Video_on_demand
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct State(pub HashMap<PlaylistSlug, Playlist>);

/// Playlist of [`Clip`]s to be played for some audience.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Playlist {
    /// [URL slug][1] of this [`Playlist`] to display in URLs referring to it.
    ///
    /// [1]: https://en.wikipedia.org/wiki/Clean_URL#Slug
    pub slug: PlaylistSlug,

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

    /// [`Clips`] which form this [`Playlist`], distributed by [`Weekday`]s.
    ///
    /// The total duration of all [`Clip`]s in the one [`Weekday`] hasn't to be
    /// exactly 24 hours, but cannot be more than that. Also, 24 hours should
    /// divide on that duration without any fractions. This is this dictated by
    /// the necessity to correctly loop the weekday's playlist to fill the whole
    /// 24 hours.
    ///
    /// All the [`Clip`]s provided for a single [`Weekday`] will be scheduled
    /// one after another sequentially, in the order they were provided, and
    /// without any gaps between them.
    pub clips: HashMap<Weekday, Vec<Clip>>,
}

impl Playlist {
    /// Hydrates the intersection of video resolutions provided by all
    /// [`Playlist`]'s [`Clip`]s returning a set of mutual resolutions (such
    /// ones that all [`Clip`]s have them).
    #[must_use]
    pub fn mutual_src_sizes(&self) -> HashSet<SrcSize> {
        let mut mutual: Option<HashSet<SrcSize>> = None;
        for clips in self.clips.values() {
            for clip in clips {
                if let Some(m) = &mut mutual {
                    m.retain(|size| clip.sources.contains_key(size))
                } else {
                    mutual = Some(clip.sources.keys().copied().collect())
                }
            }
        }
        mutual.unwrap_or_default()
    }
}

/// [URL slug][1] of a [`Playlist`].
///
/// [1]: https://en.wikipedia.org/wiki/Clean_URL#Slug
pub type PlaylistSlug = String; // TODO: newtype

/// Clip in a [`Playlist`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Clip {
    /// ID of this [`Clip`] on [YouTube].
    ///
    /// [YouTube]: https://youtube.com
    pub youtube_id: YoutubeId,

    /// Human-readable title of this [`Clip`].
    pub title: String,

    /// Time window of this [`Clip`] in its source file to be played.
    pub view: ClipView,

    /// Source files of this [`Clip`] distributed by their
    /// [video resolution][1].
    ///
    /// [1]: https://en.wikipedia.org/wiki/Display_resolution
    pub sources: HashMap<SrcSize, Src>,
}

/// ID of a [`Clip`] on [YouTube].
///
/// [YouTube]: https://youtube.com
pub type YoutubeId = String; // TODO: newtype

/// Time window in a source file to play in a [`Clip`]. Also, defines duration
/// of a [`Clip`].
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ClipView {
    /// Starting timing position (inclusive) in a source file to play from in a
    /// [`Clip`].
    #[serde(with = "timelike")]
    pub from: Duration,

    /// Finish timing position (exclusive) in a source file to play until in a
    /// [`Clip`].
    ///
    /// Obviously, should be always greater than [`ClipView::from`].
    #[serde(with = "timelike")]
    pub to: Duration,
}

/// Source file of a [`Clip`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Src {
    /// URL of this source file, where it can be read from.
    pub url: SrcUrl,

    /// [MIME type][1] of this source file.
    ///
    /// [1]: https://en.wikipedia.org/wiki/Media_type
    #[serde(rename = "type", with = "mime_serde_shim")]
    pub mime_type: Mime,

    /// Resolution of the video contained in this source file.
    pub size: SrcSize,
}

/// [URL] of a [`Clip`]'s source file.
///
/// [URL]: https://en.wikipedia.org/wiki/URL
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SrcUrl {
    /// Remote URL of the original source file on upstream server.
    ///
    /// Supports `http://` and `https://` schemes only.
    pub upstream: Url,

    /// Local URL of the locally cached version of the source file.
    ///
    /// Supports `file://` scheme only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<Url>,
}

/// Supported [video resolutions][1] of a [`Clip`].
///
/// These are basically the ones [supported by YouTube][2] (not all, though).
///
/// [1]: https://en.wikipedia.org/wiki/Display_resolution
/// [2]: https://support.google.com/youtube/answer/6375112
#[derive(
    Clone, Copy, Debug, Deserialize_repr, Eq, Hash, PartialEq, Serialize_repr,
)]
#[repr(u16)]
pub enum SrcSize {
    /// 240p [LDTV] (low-definition television) resolution.
    ///
    /// [LDTV]: https://en.wikipedia.org/wiki/Low-definition_television
    P240 = 240,

    /// 360p [LDTV] (low-definition television) resolution.
    ///
    /// [LDTV]: https://en.wikipedia.org/wiki/Low-definition_television
    P360 = 360,

    /// [480p] [EDTV] (enhanced-definition television) resolution.
    ///
    /// [480p]: https://en.wikipedia.org/wiki/480p
    /// [EDTV]: https://en.wikipedia.org/wiki/Enhanced-definition_television
    P480 = 480,

    /// [720p] (standard HD) [HDTV] (high-definition television) resolution.
    ///
    /// [720p]: https://en.wikipedia.org/wiki/720p
    /// [HDTV]: https://en.wikipedia.org/wiki/High-definition_television
    P720 = 720,

    /// [1080p] (full HD) [HDTV] (high-definition television) resolution.
    ///
    /// [1080p]: https://en.wikipedia.org/wiki/1080p
    /// [HDTV]: https://en.wikipedia.org/wiki/High-definition_television
    P1080 = 1080,
}

#[cfg(test)]
mod spec {
    use std::fs;

    use super::*;

    #[test]
    fn deserializes_example() {
        let serialized =
            fs::read("example.vod.meta.json").expect("No example file found");
        let state = serde_json::from_slice::<State>(&serialized);

        assert!(state.is_ok(), "deserialization fails");
    }
}
