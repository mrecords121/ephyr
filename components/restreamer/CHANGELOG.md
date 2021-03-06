Ephyr re-streamer changelog
===========================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.2.0] · 2021-03-18
[0.2.0]: /../../tree/restreamer-v0.2.0

[Diff](/../../compare/restreamer-v0.1.2...restreamer-v0.2.0)

### BC Breaks

- Web UI:
    - Input:
        - Remove distinguishing between pull or push endpoint in add/edit modal window ([9e1ac1c7]).
- GraphQL API:
    - Types:
        - Rename root types to `Query`, `Mutation` and `Subscription` ([9e1ac1c7]);
        - Rework fields of `Restream` and `Input` objects ([9e1ac1c7]);
        - Remove `PushInput` and `PullInput` objects ([9e1ac1c7]).
    - Mutations:
        - Replace `addPullInput` and `addPushInput` with `setRestream` ([9e1ac1c7]);
        - Replace `addOutput` with `setOutput` ([740fa998], [#28]);
        - Rename `removeInput` to `removeRestream` an change its argument's type ([9e1ac1c7]);
        - Add `restreamId` argument to `enableInput` and `disableInput` ([9e1ac1c7]);
        - Replace `inputId` argument with `restreamId` in `addOutput`, `removeOutput`, `enableOutput`, `disableOutput`, `enableAllOutputs` and `disableAllOutputs` ([9e1ac1c7]);
        - Rename `outputId` argument to `id` in `removeOutput`, `enableOutput` and `disableOutput` ([9e1ac1c7]);
        - Use `OutputDstUrl` and `MixinSrcUrl` scalars instead of `Url` in `addOutput` ([9e1ac1c7]);
        - Use `Label` scalar instead of `String` in `addOutput` ([9e1ac1c7]).
    - Queries:
        - Rename `restreams` to `allRestreams` ([9e1ac1c7]).
    - Subscriptions:
        - Rename `restreams` to `allRestreams` ([9e1ac1c7]).

### Added

- Web UI:
    - Input:
        - Optional backup endpoint (push or pull) ([a3236808], [9e1ac1c7]);
        - Ability to export/import as JSON spec ([9e1ac1c7]);
        - Optional [HLS] endpoint ([65f8b86e]);
        - Ability to pull from [HLS] HTTP URL ([2e4d46ae], [#27]);
        - Confirmation window on removing ([9acf42e2]).
    - Output:
        - Specifying [TeamSpeak] URL for mixing ([77d25dd7], [#23]);
        - Specifying [MP3] HTTP URL for mixing ([e96b39f1], [#30]);
        - Tuning and toggling volume rate of tracks ([77d25dd7], [a2c5f83f], [#23]);
        - Tuning delay of a mixed-in [TeamSpeak] track ([77d25dd7], [#23]);
        - Separate page for mixing a single output ([8103cb32], [#29]);
        - [Icecast] URL as supported destination ([5dabcfdc]);
        - [SRT] URL as supported destination ([d397aaaf], [#21]);
        - [FLV] `file:///` URL as supported destination ([46c85d4d], [#26]);
        - Ability to show, download and remove recorded [FLV] files ([46c85d4d], [#26]);
        - Ability to edit an existing output ([740fa998], [#28]);
        - Confirmation window on removing ([9acf42e2]).
    - Copying URLs to clipboard by double-click ([62355b8f]).
- GraphQL API:
    - Types:
        - `Mixin` object ([77d25dd7], [#23]);
        - `MixinId`, `Volume` and `Delay` scalars ([77d25dd7], [#23]);
        - `RestreamId` scalar ([9e1ac1c7]);
        - `Label` scalar ([9e1ac1c7]);
        - `InputSrcUrl`, `OutputDstUrl` and `MixinSrcUrl` scalars ([5dabcfdc], [9e1ac1c7]);
        - `RestreamKey` and `InputKey` scalars ([9e1ac1c7]);
        - `InputSrc` union with `RemoteInputSrc` and `FailoverInputSrc` variants ([9e1ac1c7]);
        - `InputEndpoint` object, `InputEndpointKind` enum and `EndpointId` scalar ([65f8b86e]).
    - Mutations:
        - `enableRestream` and `disableRestream` ([9e1ac1c7]);
        - `tuneVolume` and `tuneDelay` ([77d25dd7], [#23]);
        - `mix` argument to `addOutput` ([77d25dd7], [#23]);
        - `import` ([9e1ac1c7]);
        - `removeDvrFile` ([46c85d4d], [#26]).
    - Queries:
        - `Output.volume` and `Output.mixins` fields ([77d25dd7], [#23]);
        - `export` ([9e1ac1c7]);
        - `dvrFiles` ([46c85d4d], [#26]).
- Spec (export/import):
    - `v1` version ([9e1ac1c7]).
- Config:
    - `--srs-http-dir` CLI option and `EPHYR_RESTREAMER_SRS_HTTP_DIR` env var ([65f8b86e]).
- Deployment:
    - Provision script for [Ubuntu] 20.04:
        - Optional [firewalld] installation via `WITH_FIREWALLD` env var ([bbccc004]);
        - Auto-detection and usage of [DigitalOcean] and [Hetzner Cloud] mounted external volumes ([46c85d4d], [#26]).
- Documentation:
    - Deployment instructions:
        - [Oracle Cloud Infrastructure] on English and Russian languages ([9c7a9c71]);
        - Mounting additional volume on [DigitalOcean] and [Hetzner Cloud] ([46c85d4d], [#26]).

[#21]: /../../issues/21
[#23]: /../../issues/23
[#26]: /../../issues/26
[#27]: /../../issues/27
[#28]: /../../issues/28
[#29]: /../../issues/29
[#30]: /../../issues/30
[2e4d46ae]: /../../commit/2e4d46ae929da87fb78f3fa312768bd4e3693e38
[46c85d4d]: /../../commit/46c85d4d67e7b8a0efb91444f94f3575f9dfa665
[5dabcfdc]: /../../commit/5dabcfdce2420fdd43a8f4c20c2eff497e884ac3
[62355b8f]: /../../commit/62355b8f8b8f10fa3c1b6f21c9cfc86eef519211
[65f8b86e]: /../../commit/65f8b86eebad0396ef37f1df27548e70952eef63
[740fa998]: /../../commit/740fa9985feae057ecea758292bcf1c2d2758988
[77d25dd7]: /../../commit/77d25dd739d4f05b319769eddd83c01bd3a490a4
[8103cb32]: /../../commit/8103cb32c1f0e71f13907fc9917c8bcf66c51696
[9acf42e2]: /../../commit/9acf42e26aa3089688378a25871cc341cd0ab04e
[9c7a9c71]: /../../commit/9c7a9c7105324ca198eb322071ced35f53413b00
[9e1ac1c7]: /../../commit/9e1ac1c7e576c22f6234777bf01d054adb9fe5db
[a2c5f83f]: /../../commit/a2c5f83ff55f078f242f3beb6d2310a24c835c98
[a3236808]: /../../commit/a3236808c43d1c5667cac4b3037d7c83edccc48f
[bbccc004]: /../../commit/bbccc0040d95d47a72c3bf7c6fc0908f32c89bd4
[d397aaaf]: /../../commit/d397aaafde43c98e34837273926b5672df2449fe
[e96b39f1]: /../../commit/e96b39f1fd3f249b1befd0db4db745e5a495b62d




## [0.1.2] · 2021-02-13
[0.1.2]: /../../tree/restreamer-v0.1.2

[Diff](/../../compare/restreamer-v0.1.1...restreamer-v0.1.2)

### Fixed

- Deployment:
    - Provision script for [Ubuntu] 20.04:
        - Incorrect default registry pick up by [Podman] ([43bb1948]).

[43bb1948]: /../../commit/43bb1948297a6864affbf098498e4e9810358e0e




## [0.1.1] · 2021-02-05
[0.1.1]: /../../tree/restreamer-v0.1.1

[Diff](/../../compare/restreamer-v0.1.0...restreamer-v0.1.1)

### Fixed

- Broken [GraphQL Playground] in debug mode ([3bcbfa07]).

[3bcbfa07]: /../../commit/3bcbfa073bdd13bb401d0f625509d4dea392f32e




## [0.1.0] · 2021-01-26
[0.1.0]: /../../tree/restreamer-v0.1.0

[Diff](/../../compare/v0.3.6...restreamer-v0.1.0)

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
[firewalld]: https://firewalld.org
[FLV]: https://en.wikipedia.org/wiki/Flash_Video
[GraphQL]: https://www.graphql.com
[GraphQL Playground]: https://github.com/graphql/graphql-playground
[Hetzner Cloud]: https://www.hetzner.com/cloud
[HLS]: https://en.wikipedia.org/wiki/HTTP_Live_Streaming
[Icecast]: https://icecast.org
[MP3]: https://en.wikipedia.org/wiki/MP3
[Oracle Cloud Infrastructure]: https://www.oracle.com/cloud
[Podman]: https://podman.io
[RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
[Semantic Versioning 2.0.0]: https://semver.org
[SRT]: https://en.wikipedia.org/wiki/Secure_Reliable_Transport
[TeamSpeak]: https://teamspeak.com 
[Ubuntu]: https://ubuntu.com
