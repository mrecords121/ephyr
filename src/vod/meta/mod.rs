//! Definitions of `vod-meta` server.

pub mod state;

use std::collections::HashMap;

use chrono::{Datelike as _, Duration as DateDuration, Utc};

use crate::api::nginx;

use self::state::Playlist;

pub use self::state::State;

/// Schedules the given [`Playlist`] to be played by [`nginx-vod-module`][1]
/// starting from now.
///
/// # Algorithm
///
/// Schedule is created for a week starting from today and now. Each weekday is
/// fully filled with clips without any gaps, if it has at least one clip. If
/// some weekday doesn't contain enough clips to fill all the 24 hours, then
/// clips sequence is looped to fill the whole day.
///
/// Weekdays are schedule in the [`Playlist`]'s timezone.
///
/// Algorithm automatically cares to fit into the
/// [`nginx::vod_module::mapping::Set::MAX_DURATIONS_LEN`] requirement.
///
/// [1]: https://github.com/kaltura/nginx-vod-module
#[must_use]
pub fn schedule_nginx_vod_module_set(
    pl: &Playlist,
) -> nginx::vod_module::mapping::Set {
    use nginx::vod_module::mapping;

    let mut set = mapping::Set {
        id: Some(pl.slug.clone().into()),
        playlist_type: mapping::PlaylistType::Live,
        discontinuity: true,
        segment_duration: Some(pl.segment_duration.as_duration().into()),
        ..mapping::Set::default()
    };

    // Because all `mapping::Set::sequences` must have the same length, we
    // should define the minimal mutual intersection of all quality sizes and
    // use only them to form a `mapping::Set`.
    let sizes = pl.mutual_src_sizes();
    if sizes.is_empty() {
        return set;
    }
    let mut sequences: HashMap<_, _> = sizes
        .iter()
        .map(|size| {
            let sequence = mapping::Sequence {
                id: Some(format!("{}p", *size as u16)),
                language: Some(pl.lang),
                label: Some(format!("{}p", *size as u16)),
                ..mapping::Sequence::default()
            };
            (*size, sequence)
        })
        .collect();

    let now = Utc::now().with_timezone(&pl.tz);
    let mut today = now.date().and_hms(0, 0, 0);
    'whole_loop: for i in 0..7 {
        let in_today = i == 0;
        let tomorrow = today + DateDuration::days(1);

        if let Some(today_clips) = pl.clips.get(&today.weekday()) {
            let mut time = today;

            // Unfortunately, nginx-vod-module loops the whole playlist
            // only, and is unable to loop a part of playlist in the given
            // time window. That's why, to loop all clips of today's day
            // without affecting tomorrow's playlist, we need to repeat the
            // playlist manually, until tomorrow comes.
            'day_loop: while time < tomorrow {
                let mut is_at_least_one_clip_considered = false;

                for clip in today_clips {
                    let clip_duration = clip.view.to - clip.view.from;
                    let next_time =
                        time + DateDuration::from_std(clip_duration).unwrap();

                    // There is no sense to return today's clips, which have
                    // been already finished. Instead, we start from the
                    // first non-finished today's clip. This way we reserve
                    // more space for future clips, considering the
                    // nginx-vod-module `mapping::Set::MAX_DURATIONS_LEN`
                    // limitation.
                    let should_skip = in_today && next_time <= now;

                    // "Considered" means that clip's duration is considered
                    // for building the sequence timestamps. However, it
                    // doesn't necessarily mean that clip is added to this
                    // sequence.
                    let mut is_clip_considered = false;

                    for (size, src) in &clip.sources {
                        if let Some(seq) = sequences.get_mut(size) {
                            if !should_skip {
                                seq.clips.push(mapping::Clip {
                                    r#type: mapping::SourceClip {
                                        path: mapping::SourceClip::get_url_path(
                                            src.url
                                                .local
                                                .as_ref()
                                                .unwrap_or(&src.url.upstream),
                                        ),
                                        from: Some(clip.view.from.into()),
                                        to: Some(clip.view.to.into()),
                                    }
                                    .into(),
                                });
                            }

                            is_clip_considered = true;
                        }
                    }

                    if !is_clip_considered {
                        continue;
                    }
                    is_at_least_one_clip_considered = true;

                    if !should_skip {
                        set.clip_times
                            .push(time.clone().with_timezone(&Utc).into());

                        set.durations.push(clip_duration.into());
                        if set.durations.len()
                            >= mapping::Set::MAX_DURATIONS_LEN
                        {
                            break 'whole_loop;
                        }
                    }

                    time = next_time;
                    if time >= tomorrow {
                        break 'day_loop;
                    }
                }

                if !is_at_least_one_clip_considered {
                    break;
                }
            }
        }

        today = tomorrow;
    }

    set.sequences = sequences.into_iter().map(|(_, seq)| seq).collect();
    set
}
