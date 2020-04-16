Ephyr
=====

[SRS] + [FFmpeg] solution for server-side mixing of live streams powered by [Rust].




## Overview

At the moment [SRS]-based PoC is implemented.

The media scheme is:

```
           +--------+             +---------+
 --RTMP--> | Origin | ---RTMP---> | Youtube |
           +--------+             +---------+
                |
                |                +------+
                +------RTMP----> | Edge |
                                 +------+
```

1. `Origin` [SRS] waits audio/video RTMP stream being published on `/stream/some` endpoint.

2. Once it's published it spawns `ffmpeg` process, which:
    1. inputs the published RTMP stream; 
    2. inputs additional MP3 stream from [public Icecast2 server](http://radio.casse-tete.solutions) (background music);
    3. tunes volumes of each audio stream and mixes them together;
    4. transcodes audio stream into aac;
    5. leaves video stream "as is" (to offload transcoding on the edge servers);
    6. pushes resultign RTMP stream to `Youtube` and `Edge` edge servers simultaneously;
    7. listens ZeroMQ connection to configure background audio volume.
    
3. `Edge` [SRS] server accepts RTMP stream from `Origin` and transcodes it with `x264` codec (in general, makes all the quality transcoding required for downstreams).


### Quickstart

To spin-up the PoC environment:
```bash
make up
```

To publish some RTMP stream to `Origin` use any possible RTMP client, or just:
```bash
make publish
```

To play some RTMP stream from `Edge` or `Youtube` use any possible RTMP player, or just:
```bash
make play from=youtube
make play from=edge
```

To tune background music volume with ZeroMQ:
```bash
make audio volume=1.5
``` 





[FFmpeg]: https://ffmpeg.org
[Rust]: https://www.rust-lang.org
[SRS]: https://github.com/ossrs/srs
