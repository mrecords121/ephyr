Ephyr changelog
===============

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.1.0] · 2020-06-04
[0.1.0]: /../../tree/v0.1.0

### Implemented

- `teamspeak::Input`:
    - Capturing audio from a [TeamSpeak] channel;
    - Mixing audio streams of multiple talkers.
- `silence::Filter`:
    - Filling audio stream with a silence if it produces no data.
- `ffmpeg::Mixer`:
    - Mixing STDIN audio stream with [RTMP] stream;
    - Correcting STDIN audio stream sample rate;
    - Delaying audio stream;
    - On-fly toggling of audio stream volume via [ZeroMQ].
- Mixing `Spec`:
    - Multiple mixing schemes for a single [RTMP] stream;
    - Multiple outputs for a single mixing result.
- [Docker] image:
    - [FFmpeg] 4.3 for mixing;
    - [SRS] 3.0 media server for receiving [RTMP] stream and optionally serving the mixing result.





[Docker]: https://www.docker.com
[FFmpeg]: https://ffmpeg.org
[RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
[Semantic Versioning 2.0.0]: https://semver.org
[SRS]: https://ossrs.net
[TeamSpeak]: https://teamspeak.com
[ZeroMQ]: https://zeromq.org
