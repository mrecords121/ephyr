//! Inner state of a `vod-meta` server.
//!
//! It holds a collection of [`Playlist`]s, each of which has a week-based
//! schedule of [`Clip`]s (each weekday has its own collection of [`Clip`]s).
//!
//! The total duration of all [`Clip`]s in the one weekday hasn't to be exactly
//! 24 hours, but cannot be more than that, and has to be a fraction of 24
//! hours. This is this dictated by the necessity to correctly loop the
//! weekday's playlist to fill the whole 24 hours.
//!
//! [`Clip`]: crate::vod::meta::state::Clip
//! [`Playlist`]: crate::vod::meta::state::Playlist

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::Duration,
};

use anyhow::anyhow;
use chrono::{FixedOffset as TimeZone, Weekday};
use derive_more::{Display, Into};
use futures::{stream, StreamExt as _, TryFutureExt as _, TryStreamExt as _};
use isolang::Language;
use mime::Mime;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use crate::{
    api::{self, allatra},
    util::serde::{timelike, timezone},
};

pub use crate::api::allatra::video::{Resolution, YoutubeId};

/// State of a `vod-meta` server, representing a set of [`Playlist`]s for
/// different audiences.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct State(pub HashMap<PlaylistSlug, Playlist>);

impl State {
    /// Parses new [`State`] from the given `vod-meta` server API request.
    ///
    /// # Errors
    ///
    /// If some [`Playlist`] fails to parse.
    pub async fn parse_request(
        req: api::vod::meta::Request,
    ) -> Result<Self, anyhow::Error> {
        // We don't process each playlist concurrently to avoid performing too
        // many concurrent requests to `allatra::video::Api`.
        Ok(Self(
            stream::iter(req.into_iter())
                .then(|(pl_slug, pl)| Playlist::parse_request(pl_slug, pl))
                .map_ok(|pl| (pl.slug.clone(), pl))
                .try_collect()
                .await?,
        ))
    }
}

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
    pub fn mutual_src_sizes(&self) -> HashSet<Resolution> {
        let mut mutual: Option<HashSet<Resolution>> = None;
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

    /// Parses new [`Playlist`] from the given `vod-meta` server API request.
    ///
    /// # Errors
    ///
    /// - If [`Playlist`] has empty title.
    /// - If all [`Clip`]s in [`Playlist`] don't fit well into 24 hours.
    /// - If some [`Clip`] fails to parse.
    pub async fn parse_request(
        slug: PlaylistSlug,
        req: api::vod::meta::Playlist,
    ) -> Result<Self, anyhow::Error> {
        // We limit concurrent requests to `allatra::video::Api` to avoid
        // possible rate-limiting.
        const CONCURRENT_REQUESTS: usize = 10;
        const SECS_IN_DAY: u64 = 86400;

        if req.title.is_empty() {
            return Err(anyhow!(
                "Playlist '{}' shouldn't have empty title",
                slug,
            ));
        }

        let clips =
            stream::iter(req.clips.into_iter().flat_map(|(day, clips)| {
                clips.into_iter().map(move |c| (day, c))
            }))
            .map(|(day, req)| {
                Clip::parse_request(req).map_ok(move |c| (day, c))
            })
            .buffered(CONCURRENT_REQUESTS)
            .try_fold(
                <HashMap<_, Vec<_>>>::new(),
                |mut all, (day, clip)| async move {
                    all.entry(day).or_default().push(clip);
                    Ok(all)
                },
            )
            .await?;

        for (weekday, clips) in &clips {
            if clips.is_empty() {
                continue;
            }
            let total_duration: Duration =
                clips.iter().map(|c| c.view.to - c.view.from).sum();
            if total_duration.as_secs() > SECS_IN_DAY {
                return Err(anyhow!(
                    "Total duration of all clips in day {} of playlist '{}' \
                     is more than 24 hours",
                    weekday,
                    req.title,
                ));
            }
            if SECS_IN_DAY % total_duration.as_secs() != 0 {
                return Err(anyhow!(
                    "Total duration of all clips in day {} of playlist '{}' \
                     is not fraction of 24 hours",
                    weekday,
                    req.title,
                ));
            }
        }

        Ok(Playlist {
            slug,
            title: req.title,
            lang: req.lang,
            tz: req.tz,
            clips,
        })
    }
}

/// [URL slug][1] of a [`Playlist`].
///
/// [1]: https://en.wikipedia.org/wiki/Clean_URL#Slug
#[derive(Clone, Debug, Display, Eq, Hash, Into, PartialEq, Serialize)]
pub struct PlaylistSlug(String);

impl PlaylistSlug {
    /// Creates new [`PlaylistSlug`] from the given `slug` string if it
    /// represents a [valid slug][1].
    ///
    /// [1]: https://en.wikipedia.org/wiki/Clean_URL#Slug
    #[must_use]
    pub fn new<S: AsRef<str> + Into<String>>(slug: S) -> Option<Self> {
        if Self::validate(&slug) {
            Some(Self(slug.into()))
        } else {
            None
        }
    }

    /// Validates whether the given `slug` string represents a [valid slug][1].
    ///
    /// [1]: https://en.wikipedia.org/wiki/Clean_URL#Slug
    #[must_use]
    pub fn validate<S: AsRef<str> + ?Sized>(slug: &S) -> bool {
        static SLUG_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());

        let slug = slug.as_ref();
        !slug.is_empty() && SLUG_REGEX.is_match(slug)
    }
}

impl<'de> Deserialize<'de> for PlaylistSlug {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error as _;
        Ok(Self::new(<Cow<'_, str>>::deserialize(deserializer)?)
            .ok_or_else(|| D::Error::custom("not a valid URL slug"))?)
    }
}

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

    /// Source files of this [`Clip`] distributed by their video [`Resolution`].
    pub sources: HashMap<Resolution, Src>,
}

impl Clip {
    /// Parses new [`Clip`] from the given `vod-meta` server API request.
    ///
    /// # Errors
    ///
    /// - If [`Clip`] has empty title.
    /// - If incorrect [`Clip`]'s [YouTube] video URL is provided.
    /// - If [`Clip`] info cannot be retrieved from [`allatra::video::Api`].
    /// - If [`Clip`]'s duration is incorrect.
    ///
    /// [YouTube]: https://youtube.com
    pub async fn parse_request(
        req: api::vod::meta::Clip,
    ) -> Result<Self, anyhow::Error> {
        if req.title.is_empty() {
            return Err(anyhow!(
                "Clip with URL '{}' shouldn't have empty title",
                req.url,
            ));
        }

        let youtube_id = Self::parse_youtube_id(&req.url).map_err(|e| {
            anyhow!(
                "Incorrect video link '{}' provided for clip '{}': {}",
                req.url,
                req.title,
                e,
            )
        })?;

        let resp = allatra::video::Api::get_videos_yt(&youtube_id)
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to retrieve info about clip '{}' by the provided \
                     URL '{}': {}",
                    req.title,
                    req.url,
                    e,
                )
            })?;

        if req.from >= resp.duration {
            return Err(anyhow!(
                "Clip '{}' cannot start from {}, because video's total \
                 duration is {}",
                req.title,
                timelike::format(&req.from),
                timelike::format(&resp.duration),
            ));
        }
        if req.to > resp.duration {
            return Err(anyhow!(
                "Clip '{}' cannot finish at {}, because video's total duration \
                 is {}",
                req.title,
                timelike::format(&req.to),
                timelike::format(&resp.duration),
            ));
        }
        if req.to.checked_sub(req.from).unwrap_or_default()
            < Duration::from_secs(1)
        {
            return Err(anyhow!(
                "Clip '{}' should start before it ends at {}, but it starts \
                 from {}",
                req.title,
                timelike::format(&req.to),
                timelike::format(&req.from),
            ));
        }

        Ok(Self {
            youtube_id,
            title: req.title,
            view: ClipView {
                from: req.from,
                to: req.to,
            },
            sources: resp
                .sources
                .into_iter()
                .map(|source| {
                    let src = Src {
                        url: SrcUrl {
                            upstream: source.src,
                            local: None, // TODO: preserve
                        },
                        mime_type: source.r#type,
                        size: source.size,
                    };
                    (source.size, src)
                })
                .collect(),
        })
    }

    /// Validates whether the given [`Url`] is a correct [YouTube] video link
    /// and parses ID of the video from it.
    ///
    /// # Errors
    ///
    /// - If [`Url`]'s scheme is not `http`/`https`.
    /// - If [`Url`]'s host is not `youtube.com`.
    /// - If [`Url`]'s path is not `watch`.
    /// - If [`Url`]'s query misses `v` parameter.
    ///
    /// [YouTube]: https://youtube.com
    pub fn parse_youtube_id(url: &Url) -> Result<YoutubeId, anyhow::Error> {
        if !matches!(url.scheme().to_lowercase().as_str(), "http" | "https") {
            return Err(anyhow!("Only HTTP YouTube URLs are supported"));
        }
        if !matches!(
            url.host_str(),
            Some("youtube.com") | Some("www.youtube.com")
        ) {
            return Err(anyhow!("Only YouTube URLs are supported"));
        }
        if url.path().trim_end_matches('/') != "/watch" {
            return Err(anyhow!("Only full YouTube URLs are supported"));
        }
        url.query_pairs()
            .find_map(
                |(name, id)| if name == "v" { Some(id.into()) } else { None },
            )
            .ok_or_else(|| anyhow!("YouTube URL should contain video ID"))
    }
}

/// Time window in a source file to play in a [`Clip`]. Also, defines duration
/// of a [`Clip`].
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ClipView {
    /// Starting timing position in a source file to play from in a [`Clip`].
    #[serde(with = "timelike")]
    pub from: Duration,

    /// Finish timing position in a source file to play until in a [`Clip`].
    ///
    /// Obviously, should be always greater than [`ClipView::from`] for at least
    /// 1 second.
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
    pub size: Resolution,
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

#[cfg(test)]
mod spec {
    use std::fs;

    use super::*;

    #[test]
    fn deserializes_example() {
        let serialized =
            fs::read("example.vod.meta.json").expect("No example file found");
        let res = serde_json::from_slice::<State>(&serialized);

        assert!(res.is_ok(), "deserialization fails: {}", res.unwrap_err());
    }

    mod playlist_slug {
        use super::*;

        #[test]
        fn allows_valid_slugs() {
            for (input, desc) in &[
                ("sdf0sdf0", "single segment"),
                ("0-0-0-df", "multiple segments"),
            ] {
                let actual = PlaylistSlug::new(*input);
                assert!(actual.is_some(), "disallows {}", desc);
            }
        }

        #[test]
        fn disallows_invalid_slugs() {
            for (input, desc) in &[
                ("", "empty value"),
                ("-djksf0", "starting with hyphen"),
                ("djksf0-", "ending with hyphen"),
                ("djk-s___f0", "incorrect symbols"),
            ] {
                let actual = PlaylistSlug::new(*input);
                assert!(actual.is_none(), "disallows {}", desc);
            }
        }
    }

    mod clip {
        use super::*;

        #[tokio::test]
        async fn parses_valid_request() {
            let req = serde_json::from_str::<api::vod::meta::Clip>(
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "1:51:26"
                }"#,
            )
            .expect("Failed to deserialize request");

            let res = Clip::parse_request(req).await;
            assert!(res.is_ok(), "failed to parse: {}", res.unwrap_err());

            let clip = res.unwrap();
            assert_eq!(&clip.youtube_id, "0wAtNWA93hM");
            assert_eq!(&clip.title, "Круг Жизни");
            assert_eq!(clip.view.from, Duration::from_secs(0));
            assert_eq!(clip.view.to, Duration::from_secs(6686));
            assert_eq!(clip.sources.len(), 5);
        }

        #[tokio::test]
        async fn disallows_non_youtube_url() {
            for json in &[
                r#"{
                  "url": "https://vimeo.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "0:00:10"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/dfsdf?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "0:00:10"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch?vd=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "0:00:10"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "0:00:10"
                }"#,
            ] {
                let req = serde_json::from_str::<api::vod::meta::Clip>(&json)
                    .expect("Failed to deserialize request");

                let res = Clip::parse_request(req).await;
                assert!(res.is_err(), "allows non-YouTube URL in: {}", json);
            }
        }

        #[tokio::test]
        async fn disallows_invalid_duration() {
            for json in &[
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "0:00:00"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:01",
                  "to": "0:00:00"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "02:00:00",
                  "to": "02:00:03"
                }"#,
                r#"{
                  "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                  "title": "Круг Жизни",
                  "from": "00:00:00",
                  "to": "02:00:03"
                }"#,
            ] {
                let req = serde_json::from_str::<api::vod::meta::Clip>(&json)
                    .expect("Failed to deserialize request");

                let res = Clip::parse_request(req).await;
                assert!(res.is_err(), "allows invalid duration in: {}", json);
            }
        }
    }

    mod playlist {
        use super::*;

        #[tokio::test]
        async fn parses_valid_request() {
            let slug = PlaylistSlug::new("life").unwrap();
            let req = serde_json::from_str::<api::vod::meta::Playlist>(
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "mon": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "0:30:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "1:00:00"
                    }]
                  }
                }"#,
            )
            .expect("Failed to deserialize request");

            let res = Playlist::parse_request(slug.clone(), req).await;
            assert!(res.is_ok(), "failed to parse: {}", res.unwrap_err());

            let pl = res.unwrap();
            assert_eq!(pl.slug, slug);
            assert_eq!(&pl.title, "Передачи с Игорем Михайловичем");
            assert_eq!(pl.lang, Language::from_639_1("ru").unwrap());
            assert_eq!(pl.tz, TimeZone::east(3 * 3600));
            assert_eq!(pl.clips.len(), 1);
            assert!(pl.clips.contains_key(&Weekday::Mon), "incorrect weekday");
            assert_eq!(pl.clips.get(&Weekday::Mon).unwrap().len(), 2);
        }

        #[tokio::test]
        async fn disallows_invalid_clip() {
            let slug = PlaylistSlug::new("life").unwrap();
            for json in &[
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "mon": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "0:00:00"
                    }]
                  }
                }"#,
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "tue": [{
                      "url": "https://vimeo.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "1:00:00"
                    }]
                  }
                }"#,
            ] {
                let req =
                    serde_json::from_str::<api::vod::meta::Playlist>(&json)
                        .expect("Failed to deserialize request");

                let res = Playlist::parse_request(slug.clone(), req).await;
                assert!(res.is_err(), "allows invalid clip in value: {}", json);
            }
        }

        #[tokio::test]
        async fn disallows_non_24_hours_fractioned_weekday_clips_duration() {
            let slug = PlaylistSlug::new("life").unwrap();
            for json in &[
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "sat": [{
                      "url": "https://www.youtube.com/watch?v=0wAtNWA93hM",
                      "title": "Круг Жизни",
                      "from": "00:00:00",
                      "to": "0:32:57"
                    }]
                  }
                }"#,
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "sun": [{
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
                }"#,
            ] {
                let req =
                    serde_json::from_str::<api::vod::meta::Playlist>(&json)
                        .expect("Failed to deserialize request");

                let res = Playlist::parse_request(slug.clone(), req).await;
                assert!(
                    res.is_err(),
                    "allows non-24-hours fractioned total duration in: {}",
                    json,
                );
            }
        }

        #[tokio::test]
        async fn disallows_more_than_24_hours_weekday_clips_duration() {
            let slug = PlaylistSlug::new("life").unwrap();
            for json in &[
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "sat": [{
                      "url": "https://www.youtube.com/watch?v=R29rL-CIsbo",
                      "title": "Сознание и Личность",
                      "from": "00:00:00",
                      "to": "7:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=R29rL-CIsbo",
                      "title": "Сознание и Личность",
                      "from": "00:00:00",
                      "to": "7:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=R29rL-CIsbo",
                      "title": "Сознание и Личность",
                      "from": "00:00:00",
                      "to": "7:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=R29rL-CIsbo",
                      "title": "Сознание и Личность",
                      "from": "00:00:00",
                      "to": "7:00:00"
                    }]
                  }
                }"#,
                r#"{
                  "title": "Передачи с Игорем Михайловичем",
                  "lang": "rus",
                  "tz": "+03:00",
                  "clips": {
                    "mon": [{
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }, {
                      "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI",
                      "title": "ПРАВДА ЖИЗНИ",
                      "from": "00:00:00",
                      "to": "2:00:00"
                    }]
                  }
                }"#,
            ] {
                let req =
                    serde_json::from_str::<api::vod::meta::Playlist>(&json)
                        .expect("Failed to deserialize request");

                let res = Playlist::parse_request(slug.clone(), req).await;
                assert!(
                    res.is_err(),
                    "allows more than 24 hours total duration in: {}",
                    json,
                );
            }
        }
    }
}
