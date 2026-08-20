# DeviceSDK

![GitHub License](https://img.shields.io/github/license/OpenXRDS/DeviceSDK?color=white)
![GitHub Issues or Pull Requests](https://img.shields.io/github/issues/OpenXRDS/DeviceSDK)
![GitHub Issues or Pull Requests](https://img.shields.io/github/issues-closed/OpenXRDS/DeviceSDK?color=red)

To ensure compatibility with the majority of XR devices for XR applications.

DeviceSDK targets non-experts first: you should be able to build an XR application without
knowing Bevy. XRDS concepts — spawning objects, updating transforms, handling input, loading
assets — are the primary public surface. Bevy stays underneath as the implementation engine
and remains reachable as an escape hatch for developers who need engine-level control.

## Getting started

There are three ways to build with DeviceSDK: the **GUI editor** (no code), the **SDK**
(Rust, no Bevy knowledge required), and the **expert layer** (Rust with direct Bevy access).

**→ [docs/getting-started.md](docs/getting-started.md)** covers build requirements and walks
through choosing a path.

```shell
cargo build
cargo run --example simple_api
```

## Project structure

![project_structure](res/module_deps.svg?raw=true)

The workspace is a four-tier stack. Applications sit on top, `xrds-runtime` is the facade
they target, and the foundation crates below it stay independent of each other. Two
applications consume the stack: `apps/xrds-app` is the standalone XR runtime that loads a
scene and runs on Android or desktop, and [`apps/xrds-editor`](apps/xrds-editor/README.md) is
the GUI scene editor.

Two boundaries in the graph are intentional. `xrds-net` carries no codec dependencies and
`xrds-media` carries no networking ones — capture produces already-encoded streams that
consumer code wires into `xrds-net` itself. And `xrds-audio` is excluded from the workspace
and not built; Bevy's built-in spatial audio replaced it.

See [ARCHITECTURE.md](ARCHITECTURE.md) for per-crate roles and the layering model.

## Documentation

| Document | Contents |
| --- | --- |
| [docs/getting-started.md](docs/getting-started.md) | Build, choosing a path, first app |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Layering model, crate roles, data flow, diagrams |
| [examples/README.md](examples/README.md) | Example catalog, grouped by path |
| [apps/xrds-editor/README.md](apps/xrds-editor/README.md) | Editor setup, running, shortcuts |
| [android/quest/README.md](android/quest/README.md) | Packaging and deploying to a Quest |
| [crates/xrds-scene-graph/README.md](crates/xrds-scene-graph/README.md) | Scene document layer |

## License

See [LICENSE](LICENSE).
