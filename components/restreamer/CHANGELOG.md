Ephyr re-streamer changelog
===========================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.2.0-alpha.3] · 2021-??-?? · To-be-done
[0.2.0-alpha.3]: /../../tree/restreamer-v0.2.0-alpha.3

[Diff](/../../compare/restreamer-v0.1.2...restreamer-v0.2.0-alpha.3)

### BC Breaks

- Web UI:
    - Input:
        - Remove distinguishing between pull or push endpoint in add/edit modal window ([rev2]).
- GraphQL API:
    - Types:
        - Rename root types to `Query`, `Mutation` and `Subscription` ([rev2]);
        - Rework fields of `Restream` and `Input` objects ([rev2]);
        - Remove `PushInput` and `PullInput` objects ([rev2]).
    - Mutations:
        - Replace `addPullInput` and `addPushInput` with `setRestream` ([rev2]);
        - Rename `removeInput` to `removeRestream` an change its argument's type ([rev2]);
        - Add `restreamId` argument to `enableInput` and `disableInput` ([rev2]);
        - Replace `inputId` argument with `restreamId` in `addOutput`, `removeOutput`, `enableOutput`, `disableOutput`, `enableAllOutputs` and `disableAllOutputs` ([rev2]);
        - Rename `outputId` argument to `id` in `removeOutput`, `enableOutput` and `disableOutput` ([rev2]);
        - Use `OutputDstUrl` and `MixinSrcUrl` scalars instead of `Url` in `addOutput` ([rev2]);
        - Use `Label` scalar instead of `String` in `addOutput` ([rev2]).
    - Queries:
        - Rename `restreams` to `allRestreams` ([rev2]).
    - Subscriptions:
        - Rename `restreams` to `allRestreams` ([rev2]).

### Added

- Web UI:
    - Input:
        - Optional backup endpoint (push or pull) ([a3236808], [rev2]);
        - Ability to export/import as JSON spec ([rev2]).
    - Output:
        - Specifying [TeamSpeak] URL for mixing ([77d25dd7], [#23]);
        - Tuning volume rate of tracks ([77d25dd7], [#23]);
        - Tuning delay of a mixed-in [TeamSpeak] track ([77d25dd7], [#23]); 
        - [Icecast] URL as supported destination ([5dabcfdc]).
- GraphQL API:
    - Types:
        - `Mixin` object ([77d25dd7], [#23]);
        - `MixinId`, `Volume` and `Delay` scalars ([77d25dd7], [#23]);
        - `RestreamId` scalar ([rev2]);
        - `Label` scalar ([rev2]);
        - `InputSrcUrl`, `OutputDstUrl` and `MixinSrcUrl` scalars ([5dabcfdc], [rev2]);
        - `RestreamKey` and `InputKey` scalars ([rev2]);
        - `InputSrc` union with `RemoteInputSrc` and `FailoverInputSrc` variants ([rev2]).
    - Mutations:
        - `enableRestream` and `disableRestream` ([rev2]);
        - `tuneVolume` and `tuneDelay` ([77d25dd7], [#23]);
        - `mix` argument to `addOutput` ([77d25dd7], [#23]);
        - `import` ([rev2]).
    - Queries:
        - `Output.volume` and `Output.mixins` fields ([77d25dd7], [#23]);
        - `export` ([rev2]).
- Spec (export/import):
    - `v1` version ([rev2]).
- Deployment:
    - Provision script for [Ubuntu] 20.04:
        - Optional [firewalld] installation via `WITH_FIREWALLD` env var ([rev]).

[#23]: /../../issues/23
[5dabcfdc]: /../../commit/5dabcfdce2420fdd43a8f4c20c2eff497e884ac3
[77d25dd7]: /../../commit/77d25dd739d4f05b319769eddd83c01bd3a490a4
[a3236808]: /../../commit/a3236808c43d1c5667cac4b3037d7c83edccc48f
[rev]: /../../commit/full-rev
[rev2]: /../../commit/full-rev2




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
[GraphQL]: https://www.graphql.com
[GraphQL Playground]: https://github.com/graphql/graphql-playground
[Hetzner Cloud]: https://www.hetzner.com/cloud
[Icecast]: https://icecast.org
[Podman]: https://podman.io
[RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol
[Semantic Versioning 2.0.0]: https://semver.org
[TeamSpeak]: https://teamspeak.com 
[Ubuntu]: https://ubuntu.com
