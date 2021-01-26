Ephyr re-streamer changelog
===========================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.1.0] · 2021-01-26
[0.1.0]: /../../tree/restreamer-v0.1.0

[Diff](/../../compare/v0.3.6..restreamer-v0.1.0)

### Implemented

- Web UI:
    - Input:
        - Push type to accept [RTMP] stream;
        - Pull type to automatically pull [RTMP] stream from remote server;
        - Optional label;
        - Status indication (offline, connecting, online);
        - Ability to enable/disable a single input;
        - Ability to enable/disable all outputs for a single input;
        - Editing an existing input;
        - Displaying a total count of outputs by their statuses along with the filtering. 
    - Output:
        - Optional label;
        - Adding multiple endpoints via CSV list;
        - Status indication (offline, connecting, online);
        - Ability to enable/disable a single output.
    - Optional password protection via [Basic HTTP auth].
- GraphQL API:
    - Types:
        - `Info` object;
        - `Restream` object;
        - `Input` union, `PushInput` and `PullInput` objects;
        - `Output` object;
        - `InputId`, `OutputId` scalars;
        - `Status` enum.
    - Mutations:
        - `addPullInput`, `addPushInput`, `removeInput`;
        - `enableInput`, `disableInput`;
        - `addOutput`, `removeOutput`;
        - `enableOutput`, `disableOutput`;
        - `enableAllOutputs`, `disableAllOutputs`;
        - `setPassword`.
    - Queries:
        - `info`;
        - `restreams`.
    - Subscriptions:
        - `info`;
        - `restreams`.
- Deployment:
    - [Docker] image;
    - Provision script for [Ubuntu] 20.04.
- Documentation:
    - Deployment instructions for [DigitalOcean] and [Hetzner Cloud] on English and Russian languages.





[Basic HTTP auth]: https://en.wikipedia.org/wiki/Basic_access_authentication
[DigitalOcean]: https://www.digitalocean.com
[Docker]: https://www.docker.com
[GraphQL]: https://www.graphql.com
[Hetzner Cloud]: https://www.hetzner.com/cloud
[RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
[Semantic Versioning 2.0.0]: https://semver.org
[Ubuntu]: https://ubuntu.com
