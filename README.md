# DeviceSDK

![GitHub License](https://img.shields.io/github/license/OpenXRDS/DeviceSDK?color=white)
![GitHub Issues or Pull Requests](https://img.shields.io/github/issues/OpenXRDS/DeviceSDK)
![GitHub Issues or Pull Requests](https://img.shields.io/github/issues-closed/OpenXRDS/DeviceSDK?color=red)

To ensure compatibility with the majority of XR devices for XR applications.

## Design principles

This project targets non-experts first.

- The default path should provide the simplest way to build XR applications without requiring Bevy knowledge.
- XRDS concepts should be the primary public surface for common tasks such as spawning scene objects, updating transforms, handling input, and loading assets.
- The expert path must remain available for developers who need lower-level control, but it should not shape the default mental model of the SDK.
- New features should first be evaluated against a simple question: can a non-expert use this through XRDS concepts alone?
- Bevy should remain an implementation engine and an escape hatch for advanced users, not the required interface for routine application development.

In practice, this means the repo follows a two-layer model:

- `XrdsApp`, `XrdsAPI`, and `XrdsUpdateContext` are the default application-facing layer.
- `RuntimeHandler` and direct Bevy systems are the expert layer for engine-level control.

For strict layering, the `xrds` crate does not re-export Bevy. If you intentionally drop to the expert layer, import `bevy` explicitly instead of reaching it through XRDS.

Editor-focused planning notes are tracked in [docs/editor-readiness-checklist.md](docs/editor-readiness-checklist.md).

## Which Type Do I Use?

Use this rule first:

- If you are building or editing live runtime content through `XrdsAPI`, use runtime-facing XRDS types such as `XrdsCamera`, `XrdsCube`, and `XrdsPointLight`.
- If you are building, saving, loading, importing, or exporting an authored scene document, use scene-document types such as `XrdsSceneDocument` and `XrdsSceneNode`.

In short:

- `XrdsCamera` means "live runtime object I spawn or edit through XRDS".
- `XrdsSceneNode` means "authored scene data that should survive save/load and import/export".

For scene environment control, use the same rule:

- If environment policy is part of saved scene meaning, author it in `XrdsSceneDocument` and import/export it through the document layer.
- If environment policy is owned by live app logic, use `XrdsAPI::merge_scene_assets(...)`, `XrdsAPI::set_scene_environment(...)`, and `XrdsAPI::clear_scene_environment(...)` at runtime.

Typical SDK app code should usually start from the runtime-facing layer and keep the typed handles returned by `XrdsAPI::spawn(...)`.

Use `xrds-scene-graph` only when you need a durable document model with stable ids, hierarchy, editor metadata, and round-trip persistence. See [crates/xrds-scene-graph/README.md](crates/xrds-scene-graph/README.md) for the document-layer boundary.

For authored material texture UVs, the document layer treats `rotation_deg` as center-based by default. When you intentionally need exact low-level origin-based behavior, use the document/runtime UV transform mode escape hatch and select `Raw` explicitly.

Scene-level environment policy now supports both of the intended control paths:

- document-first: save/load a scene document, then import it into runtime
- runtime-first: merge durable scene asset ids into the runtime catalog, then set or clear scene environment policy directly through `XrdsAPI`

Today that scene-wide policy surface includes IBL, skybox, manual exposure, and linear fog.

For a quick visual verification pass on environment-map lighting, run [examples/environment_map_visual_check.rs](examples/environment_map_visual_check.rs).

## Usage

## How to build

Install system dependencies (Ubuntu/Debian):

```shell
sudo apt install clang libssl-dev libasound2-dev \
    libavcodec-dev libavformat-dev libavutil-dev libavdevice-dev \
    libavfilter-dev libswresample-dev libswscale-dev libpostproc-dev \
    libxcb-glx0-dev
```

Then build:

```shell
cargo build
```

## Project structure

![project_structure](res/module_deps.svg?raw=true)

## Editor on Linux

The editor is supported on Linux (X11, Vulkan).  A few platform-specific notes:

### Running from VS Code

`.cargo/config.toml` includes a custom runner that strips VS Code Snap's GTK module paths (they point to glibc 2.31 loaders that fail on Ubuntu 22.04+) and sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` to prevent a webkit2gtk/NVIDIA GLX conflict.  `cargo run -p xrds-editor` picks this up automatically — no manual setup needed.

For running the compiled binary directly (e.g. a release build), use:

```shell
./run-editor.sh
```

### Window resize stutter

Dragging the window border causes the Vulkan swap chain to report "surface changed" on each resize event — this is a Vulkan/X11 limitation and Bevy recovers by dropping the affected frames.  It does not affect normal operation.  Avoid slow edge-drags during screen recording or demos; use the window manager's keyboard shortcut (e.g. `Super+←/→`) to snap the window to a new size instead.
