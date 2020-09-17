Ephyr
=====

[![GitHub Release](https://img.shields.io/github/v/release/ALLATRA-IT/ephyr)](https://github.com/ALLATRA-IT/ephyr/releases) [![Docker Pulls](https://img.shields.io/docker/pulls/allatra/ephyr.svg)](https://hub.docker.com/r/allatra/ephyr)

[Changelog](https://github.com/ALLATRA-IT/ephyr/blob/master/CHANGELOG.md)

Server-side kit for streaming solutions, powered by [Rust].




## Overview


### `ephyr mix` command

Represents a wrapper over [FFmpeg] binary, which performs mixing according to specified schema (see [example][1]). At the moment, it's intended to be called as [SRS] `exec.publish` directive, so performs mixing on-demand (when [RTMP] stream is pushed to [SRS]).

Ephyr is able to capture audio from [TeamSpeak] server, and feed it to [FFmpeg] for mixing with [RTMP] stream.

See `ephyr mix --help` for details.


### `ephyr serve vod-meta` command

Represents a simple HTTP server, which provides a meta information for [`kaltura/nginx-vod-module`] to play a scheduled [VOD] playlists. Each playlists schedules on weekly basis (see [example][2]).

New schedule may be specified via `PUT` HTTP request.

Also, supports background downloading of remote video files into a local files cache.

See `ephyr serve vod-meta --help` for details.

<details><summary>REST API overview</summary>

<details><summary>GET /{format}/{playlist}/{file}</summary>

Generates meta information for [`kaltura/nginx-vod-module`] to play a scheduled [VOD] playlist starting from now.

</details>

<details><summary>GET /</summary>

Returns the whole current persisted state of this [VOD] meta server.

</details>

<details><summary>GET /{playlist}</summary>

Returns the current state of a single playlist persisted in this [VOD] meta server.

</details>

<details><summary>PUT /[?force=true] (authorized)</summary>

Renews the whole state of this [VOD] meta server.

If renewing breaks the current playback of any playlist, then an error will be returned. Providing a query parameter `force=true` will force-apply the new state regardless of a broken playback:
```bash
curl -X PUT -H "Authorization: Bearer qwerty" -H "Content-Type: application/json" \
     -d @vod.state.json http://127.0.0.1/vod-meta/?force=true
```

<details><summary>Example of state request</summary>

Requirements:
- All weekdays in a single playlist should have at least one clip.
- Duration of any clip should divide on its playlist's `segment_duration` without remainder.
- All clips in a single weekday should have total duration less than 24 hours and represent a fraction of it.

Parameters:
- `segment_duration` is optional (defaults to `10s`);
- `resolutions` is optional (defaults to empty);
- anything other is mandatory.

```json
{
  "divan-tv": {
    "title": "Divan TV",
    "lang": "eng",
    "tz": "+02:00",
    "segment_duration": "10s",
    "resolutions": [360, 720],
    "clips": {
      "mon": [
        {"from": "00:00:00", "to": "00:02:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"},
        {"from": "00:00:00", "to": "00:01:00", "title": "Truth of Life", "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI"}
      ],
      "sun": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
      "tue": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
      "wed": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
      "thu": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
      "fri": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
      "sat": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}]
    }
  }
}
```

</details>

</details>

<details><summary>PUT /{playlist}[?force=true] (authorized)</summary>

Renews a single playlist in the current state of this [VOD] meta server.

If renewing breaks the current playlist's playback, then an error will be returned. Providing a query parameter `force=true` will force-apply the new playlist's state regardless of a broken playback:
```bash
curl -X PUT -H "Authorization: Bearer qwerty" -H "Content-Type: application/json" \
     -d @playlist.json http://127.0.0.1/vod-meta/divan-tv?force=true
```

<details><summary>Example of playlist request</summary>

Requirements:
- All weekdays should have at least one clip.
- Duration of any clip should divide on `segment_duration` without remainder.
- All clips in a single weekday should have total duration less than 24 hours and represent a fraction of it.

Parameters:
- `segment_duration` is optional (defaults to `10s`);
- `resolutions` is optional (defaults to empty);
- anything other is mandatory.

```json
{
  "title": "Divan TV",
  "lang": "eng",
  "tz": "+02:00",
  "segment_duration": "10s",
  "resolutions": [360, 720],
  "clips": {
    "mon": [
      {"from": "00:00:00", "to": "00:02:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"},
      {"from": "00:00:00", "to": "00:01:00", "title": "Truth of Life", "url": "https://www.youtube.com/watch?v=Q69gFVmrCiI"}
    ],
    "sun": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
    "tue": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
    "wed": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
    "thu": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
    "fri": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}],
    "sat": [{"from": "00:00:00", "to": "00:01:00", "title": "Life circle", "url": "https://www.youtube.com/watch?v=0wAtNWA93hM"}]
  }
}
```

</details>

</details>

<details><summary>DELETE /{playlist} (authorized)</summary>

Removes a single playlist from the current state of this [VOD] meta server.

</details>

</details>




## Try it out


### [SRS] + [FFmpeg] server-side mixing

To boot up a simple example, run:
```bash
make up app=mix rebuild=yes
```

Now, publish an RTMP stream to `rtmp://127.0.0.1:1935/input/mic` endpoint either with [OBS], or any other RTMP publisher. You may also use `FFmpeg` for that:
```bash
make publish
```

Finally, play the resulting mixed RTMP stream:
```bash
make play
```

Also, you may tune volume on-fly:
```bash
make tune volume=1 track=music
make tune volume=0.4 track=original
```


### [VOD] meta info HTTP server

To boot up a simple example, run:
```bash
make up app=vod rebuild=yes
```

Now, open an [HLS] or [DASH] stream in a [VLC] or any other media player supporting HTTP streaming:
```bash
http://localhost/hls/cnn-live/master.m3u8    # to play HLS stream
http://localhost/dash/cnn-live/manifest.mpd  # to play DASH stream
```




## License

Ephyr is subject to the terms of the [Blue Oak Model License 1.0.0](https://github.com/ALLATRA-IT/ephyr/blob/master/LICENSE.md). If a copy of the [BlueOak-1.0.0](https://spdx.org/licenses/BlueOak-1.0.0.html) license was not distributed with this file, You can obtain one at <https://blueoakcouncil.org/license/1.0.0>.

[SRS] is licensed under the [MIT license](https://github.com/ossrs/srs/blob/3.0release/LICENSE).

[FFmpeg] is generally licensed under the [GNU Lesser General Public License (LGPL) version 2.1](http://www.gnu.org/licenses/old-licenses/lgpl-2.1.html). To consider exceptions read the [FFmpeg License and Legal Considerations](https://www.ffmpeg.org/legal.html).

As with all Docker images, these likely also contain other software which may be under other licenses (such as Bash, etc from the base distribution, along with any direct or indirect dependencies of the primary software being contained), including libraries used by [FFmpeg].

As for any pre-built image usage, it is the image user's responsibility to ensure that any use of this image complies with any relevant licenses for all software contained within.





[DASH]: https://en.wikipedia.org/wiki/Dynamic_Adaptive_Streaming_over_HTTP
[FFmpeg]: https://ffmpeg.org
[HLS]: https://en.wikipedia.org/wiki/HTTP_Live_Streaming
[OBS]: https://obsproject.com
[RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
[Rust]: https://www.rust-lang.org
[SRS]: https://github.com/ossrs/srs
[TeamSpeak]: https://teamspeak.com
[VLC]: http://www.videolan.org/vlc
[VOD]: https://en.wikipedia.org/wiki/Video_on_demand

[`kaltura/nginx-vod-module`]: https://github.com/kaltura/nginx-vod-module

[1]: https://github.com/ALLATRA-IT/ephyr/blob/master/example.mix.spec.json
[2]: https://github.com/ALLATRA-IT/ephyr/blob/master/example.vod.meta.json
