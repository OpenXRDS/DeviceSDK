# Getting Started

Three ways to build an XR app with DeviceSDK. Pick one, then follow its links —
this page only gets you to the right starting point.

## Build

Install system dependencies (Ubuntu/Debian):

```shell
sudo apt install clang libssl-dev libasound2-dev libudev-dev \
    libavcodec-dev libavformat-dev libavutil-dev libavdevice-dev \
    libavfilter-dev libswresample-dev libswscale-dev libpostproc-dev \
    libxcb-glx0-dev cmake perl \
    libgtk-3-dev libwebkit2gtk-4.1-dev
```

`libudev-dev` is required by Bevy's input backend, `cmake`/`perl` by `xrds-net`'s vendored
BoringSSL, and `libgtk-3-dev`/`libwebkit2gtk-4.1-dev` by the editor — `apps/xrds-editor` is a
workspace member, so a plain `cargo build` compiles its webview and file dialogs too.

The `libav*`/`libsw*` packages are FFmpeg, used by `xrds-media` for transcoding and for
video playback (`ffmpeg-next` links these libraries directly — nothing shells out to the
`ffmpeg` command).

On **Windows** the same libraries come from [vcpkg](https://vcpkg.io):

```powershell
vcpkg install ffmpeg:x64-windows
$env:VCPKG_ROOT = "C:\path\to\vcpkg"   # ffmpeg-next locates FFmpeg through this
```

```shell
cargo build
```

## Which path do I use?

| Path | You write | Bevy knowledge | Choose it when |
| --- | --- | --- | --- |
| [GUI Editor](#1-gui-editor) | nothing | none | You are authoring a scene and don't need custom app logic |
| [SDK](#2-sdk-default-layer) | Rust | not required | Your app needs logic — spawning, input handling, per-frame updates |
| [Expert](#3-expert-bevy) | Rust + Bevy | required | You need engine-level control XRDS doesn't expose |

Most projects start with the editor or the SDK. The expert path is an escape hatch, not a
progression — you can drop into it for one feature and stay on the SDK for everything else.

### 1. GUI Editor

Author a scene visually and save it as a scene document (`XrdsSceneDocument`, JSON on disk).
No Rust involved. `apps/xrds-app` then loads that document and runs it, on desktop or on a
headset.

```shell
cargo run -p xrds-editor
```

The editor needs Node.js for its frontend, so it has a one-time setup step. See
[apps/xrds-editor/README.md](../apps/xrds-editor/README.md) for setup, hot-reload, and
keyboard shortcuts, and [android/quest/README.md](../android/quest/README.md) for packaging
the result onto a Quest.

### 2. SDK (default layer)

Implement the `XrdsApp` trait. `XrdsAPI` builds the scene at startup and
`XrdsUpdateContext` drives it per frame — neither requires knowing Bevy. Abridged from
[`examples/xrds_first/simple_api.rs`](../examples/xrds_first/simple_api.rs):

```rust
impl XrdsApp for SimpleAPIApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let camera = api.spawn(&{
            XrdsCamera::new()
                .with_name("SimpleAPICamera")
                .looking_at([0.0, 0.0, 0.0])
        });
        api.set_translation(&camera, [0.0, 2.0, 6.0]);

        let cube = api.spawn(&{
            let mut cube = XrdsCube::new().with_name("SimpleAPICube");
            cube.transform.translation = [0.0, 0.5, 0.0];
            cube
        });
        api.set_material_base_color(&cube, XrdsColor::srgb(1.0, 1.0, 1.0));
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(SimpleAPIApp::default())
        .expect("failed to run simple_api");
}
```

```shell
cargo run --example simple_api
```

**Runtime types vs document types.** Descriptors come in pairs: `XrdsCube` is a live object
you spawn through `XrdsAPI`, `XrdsSceneCube` is authored data that survives save/load. The
`XrdsScene` prefix marks the whole document-layer family, so the rule holds for every pair —
`XrdsPointLight`/`XrdsScenePointLight`, `XrdsWorldButton`/`XrdsSceneWorldButton`, and so on.
Reach for the document types only when you need durable ids and round-trip persistence; see
[crates/xrds-scene-graph/README.md](../crates/xrds-scene-graph/README.md).

More examples: [examples/README.md](../examples/README.md).

### 3. Expert (Bevy)

Use `RuntimeHandler` and write Bevy systems directly. The `xrds` crate deliberately does not
re-export Bevy — expert code imports `bevy` explicitly, so dropping a layer is always visible
in your imports.

Start from [examples/expert/](../examples/expert/) and read
[ARCHITECTURE.md](../ARCHITECTURE.md) for how the expert layer sits under the SDK.

## Where to go next

- [ARCHITECTURE.md](../ARCHITECTURE.md) — layering model, crate roles, data flow
- [examples/README.md](../examples/README.md) — the full example catalog
- [docs/manual/api-reference-outline.md](manual/api-reference-outline.md) — API reference
