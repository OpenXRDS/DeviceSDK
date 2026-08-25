# Video on a headset — spike plan

**Status** (2026-08-25): **answered — a video plays on a material on a Quest 3.**
Phase 0 complete on desktop; route A measured on device and eliminated; route B
built and verified end to end. One piece of cleanup outstanding: the conversion pass
still stalls on a fence every frame, which is most of its 3.53 ms cost.
Written 2026-08-24.

One question, answered on device before any of the three phases in
`OVERALL_PROGRESS.md`'s Medium tier are committed to:

> **Can a hardware-decoded video frame reach a Bevy material on a Quest 3?**

Everything either side of that is already proven. Only the join is unknown, and it
is unknown in a specific way.

## Why this is worth doing rather than deferring

The blueprint has deferred Video repeatedly on the grounds that "the playback half
has no precedent in the workspace — no video decode path exists". That was true of
this workspace and false of the team's work:

**`F:\workspace\rust\HMDViewer` already does the hard half on a Quest 3.**

```text
H.264 → AMediaCodec hardware decoder → AImageReader surface
     → AHardwareBuffer → VkImage (YCbCr sampler conversion)   [zero-copy]
     → inside-facing equirect sphere, Vulkan multiview
```

4096×2048 at 30 fps, from a WebSocket stream or a bundled MP4. Not a prototype.

**And DeviceSDK already owns the bridge from Vulkan into wgpu.**
`crates/xrds-openxr/src/backends/vulkan.rs` wraps OpenXR swapchain images as wgpu
textures every frame:

```rust
let hal_texture = hal_device.texture_from_raw(...);
device.create_texture_from_hal::<wgpu_hal::api::Vulkan>(...)
```

So this is not "build a video decoder". It is "connect two things that both work".

## The one thing that does not join, and why

HMDViewer's `AImageReader` uses `AIMAGE_FORMAT_PRIVATE` — the buffer's layout is
vendor-defined. The Vulkan import consequently uses an **external format**
(`vk_renderer.rs:783`):

```rust
vk::SamplerYcbcrConversionCreateInfo::default()
    .format(vk::Format::UNDEFINED)          // not a known VkFormat
    .push_next(&mut ext_format)             // VkExternalFormatANDROID
```

Sampling that image requires a `VkSamplerYcbcrConversion` built from the external
format id, **and an immutable sampler baked into the descriptor set layout**.

**wgpu cannot express any of it.** `create_texture_from_hal` takes a
`wgpu::TextureFormat`, and there is no external/undefined variant;
`BindGroupLayoutEntry` has no immutable-sampler concept; and the conversion is a
property of the descriptor layout rather than something a shader can do.

wgpu 26 *does* have `TextureFormat::NV12` behind `Features::TEXTURE_FORMAT_NV12`,
with plane aspects (`Plane0` → `R8Unorm`, `Plane1` → `Rg8Unorm`) — but that is a
known format, which an `AIMAGE_FORMAT_PRIVATE` buffer is not. The NV12 route only
becomes available if the decoder can be made to emit a non-opaque layout.

That is the whole spike.

## Three candidate joins, cheapest first

**A. Ask the decoder for a known format.** Create the `AImageReader` with
`AIMAGE_FORMAT_YUV_420_888` instead of `PRIVATE`. If MediaCodec will still render
into it with `GPU_SAMPLED_IMAGE` usage, the planes are addressable: import as two
wgpu textures and convert YUV→RGB in the Bevy material's shader. No Ycbcr
conversion, no immutable sampler, no external format.

*Risk:* many devices only support zero-copy into `PRIVATE`, and asking for
`YUV_420_888` can silently force a CPU-side copy — which would show up as frame
time, not as an error. Measure, do not assume.

**B. Convert on the GPU into a wgpu-owned texture.** Import the `AHardwareBuffer`
as a `VkImage` with the Ycbcr conversion in raw `ash` exactly as HMDViewer does,
then run one small Vulkan pass writing RGBA into a *second*, normal-format image,
and hand that to wgpu through the existing `texture_from_hal` bridge.

*Cost:* one full-frame GPU write per frame. At 4096×2048 RGBA8 that is ~33 MB
written and read per frame, ~2 GB/s at 30 fps — against an Adreno 740's roughly
60 GB/s, so on the order of 3%. Plausible, and it must be measured rather than
argued, given how this project's last three cost estimates went.

*Benefit:* works regardless of what format the decoder will emit, because it never
asks wgpu to understand the opaque buffer.

**C. Render the video outside Bevy**, as HMDViewer does. Rejected unless A and B
both fail: it means a second render path with its own pass ordering and its own
XR swapchain interaction, which fights the architecture this SDK exists to provide.

### Route A is unavailable on Quest 3 — measured, 2026-08-25

Probed on device (Quest 3, Adreno 740) by instrumenting HMDViewer, whose pipeline
already exists, rather than porting 640 lines into this workspace to find out
afterwards. Branch `probe/yuv420-reader` in that repo; `master` untouched.

The request succeeds. **The buffer is still opaque.**

```text
[probe] AImageReader created with YUV_420_888 (4096x2048)
[probe] AHardwareBuffer: 4096x2048 layers=1 stride=4096
        format=0x7fa30c06 usage=0x10000030410300
[probe] 720 frames, 30.0 fps sustained
[probe] VkFormat = 0 (VK_FORMAT_UNDEFINED), external = 0x1fa,
        ycbcr model = YCBCR_709, range = ITU_NARROW
```

Three findings, in the order they matter:

1. **`AIMAGE_FORMAT_YUV_420_888` is accepted.** The reader is created; the decoder
   renders into it. The spike's stated risk — outright rejection — did not happen.
2. **Zero-copy survives it.** 30.0–30.2 fps sustained over 720 frames, the clip's
   full rate. The *other* stated risk — a silently inserted CPU copy — also did not
   happen. This is worth knowing independently of route A.
3. **But Vulkan still reports `VK_FORMAT_UNDEFINED`.** The buffer's real format is
   `0x7fa30c06`, in Qualcomm's vendor range — a UBWC/Venus tiled layout, not linear
   NV12. So sampling it *still* requires an external format plus a
   `VkSamplerYcbcrConversion` in an immutable sampler, which is precisely the set of
   things wgpu cannot express.

**Asking for a known format is not the same as getting one.** The API accepts the
request and the driver honours it at full speed while handing back a vendor layout
anyway. Nothing errors. Route A dies not on the format request but one layer down, at
`vkGetAndroidHardwareBufferPropertiesANDROID`.

**Route B is the path.** Two figures come free from this probe and are needed for it:
the conversion is **BT.709, narrow range**, and the source is 4096×2048 at 30 fps.

Cost of the probe: two constants and two log lines, in a project that already ran on
the target device. Cost of learning the same thing by building route A first: the
whole import path, discarded.

## Phases

| Phase | Scope | Answers |
| --- | --- | --- |
| 0 ✅ | Desktop: a decoded frame on a quad, any decoder | Does the Bevy material/asset shape work at all? — **yes**, see below |
| 1 ❌ | Quest: route A — `YUV_420_888` + two-plane import + shader convert | Is the cheap join available? — **no**, see below |
| 2 | Quest: route B — external-format import + conversion pass | Is the fallback affordable? |

Route B is split into three stages, because only the middle one is uncertain:

| | Stage | State |
| --- | --- | --- |
| B1 | Android decode → `AHardwareBuffer` (`xrds-media/src/video/android.rs`) | ✅ verified on device |
| B2 | `ash` import + Ycbcr conversion pass → RGBA → wgpu (`xrds-openxr/src/video/`) | ✅ verified on device |
| B3 | Wire into the video texture registry (`xrds-runtime/src/xrds_api/video_android.rs`) | ✅ **video playing on a Quest 3** |

### Route B works — 2026-08-25

A video file plays on a material in a 3-D scene on a Quest 3. The full chain:

```text
MediaCodec (hardware) → AImageReader → AHardwareBuffer
  → VkImage, external format + VkSamplerYcbcrConversion
  → fullscreen pass through an immutable sampler
  → R8G8B8A8_SRGB VkImage → wgpu::Texture
  → RenderAssets<GpuImage> → XRDS material texture slot
```

From an author's point of view none of that is visible:

```rust
api.play_hardware_video("clip", "/sdcard/.../clip.mp4");
api.set_material_texture_slot(&screen, BaseColor, Some(XrdsMaterialTextureRef {
    texture_asset_id: "clip".into(),
    ..Default::default()
}));
```

Which is the same two lines the desktop path takes, and the reason phase 0 was worth
doing first: `XrdsMaterialTextureRef` already meant "a texture named by id", so
nothing about materials, meshes, shaders or the renderer had to learn about video.

**Costs measured, and one is not yet acceptable.** The conversion pass ran at
**3.53 ms/frame at 1920×800** when measured from the main world — far above the ~3%
of frame budget the bandwidth estimate predicted. The gap is not the conversion: it
is a full CPU/GPU stall, because `convert()` blocks on a fence after submitting.

Blocking was deliberate — it is what makes the write safe without wgpu knowing the
pass happened — but it is the dominant cost and it must go. Two related problems,
one fix:

1. **The per-frame fence stall.** Still present.
2. **`vkQueueSubmit` racing wgpu's own submissions.** Addressed by B3: the pass now
   runs in the render schedule (`RenderSystems::PrepareResources`), on the render
   thread, at a point wgpu is not submitting.

The remaining work is to drop the stall — signal a semaphore wgpu waits on instead of
blocking the CPU — and to re-measure. Until then the figure above is the honest cost,
and it is a quarter of a 72 Hz budget at a quarter of 4K.

**A note on what B3 had to work around.** The material bind group problem from phase 0
appears here too, in a nastier form: the render world installs the real texture on
some frame, and a material whose bind group was built before that keeps sampling the
placeholder forever. `rebind_hardware_video_materials` marks the materials modified
every frame rather than reasoning about the ordering — the same Bevy gap
([bevy#3674](https://github.com/bevyengine/bevy/issues/3674)), reached by a different
road.

### B1 result — hardware decode on a Quest 3

```text
hardware decode: video/hevc 1920x800
[b1] frame 1: HardwareBuffer(0xb400007917f5bf00)
[b1] 240 frames in 10.0s = 24.0 fps, from 2341 polls
```

HEVC decoded in hardware into GPU-resident buffers, paced exactly to the clip's
24 fps. The 2341 polls against 240 frames is the part worth noting: the
presentation clock is driving output, not the poll loop.

Run it by pushing a clip and launching; it logs and returns without starting the
app, because the subject is the decoder and an XR session would only add noise:

```powershell
adb push clip.mp4 /sdcard/Android/data/org.openxrds.devicesdk/files/probe_video.mp4
```

**Delete that file to get the normal app back** — its presence is the only trigger.

Two things B1 settled that were not on anyone's list:

- **`xrds-media`'s capture half cannot exist on Android.** `nokhwa` and `cpal`
  (via `oboe`) fail at *link* time, not compile time, so nothing catches it until
  an Android binary actually depends on the crate. Both are now target-gated off.
- **`cargo ndk check -p xrds-app` is not the build.** Without
  `--no-default-features` it pulls `xrds-net`'s FTP server and with it
  `libunftp → rustls → aws-lc-sys`, which does not cross-compile. The project's
  own build script passes that flag; a bare `check` does not, and looks like a
  broken tree.

Phase 0 is desktop and can use anything (`bevy_av1` 0.3.0 targets Bevy 0.17 and
decodes AV1/IVF via dav1d; `xrds-media` already has ffmpeg decoders behind its
`transcoding` feature). Its purpose is *not* the decoder — it is to settle the
asset and material shape without Android in the way.

### Phase 0 result

**Answered: yes, and no new render path is needed.** A video screen is an ordinary
textured quad; the only thing that makes it a video is that its texture changes.

What had to be built, in the runtime rather than the example:

- `crates/xrds-runtime/src/xrds_api/video.rs` — a registry of runtime-owned
  `Handle<Image>` keyed by id, with `create_video_texture` / `write_video_frame` /
  `remove_video_texture` on both `XrdsAPI` and `XrdsUpdateContext`.
- One branch in `resolved_texture_handle_for_material_slot`, consulted *before* the
  asset catalog.

That branch is the whole architectural change, and it closes a real gap: before it,
**no runtime-generated texture could reach an XRDS material by any public route.**
Every texture was file-backed — a slot named an asset id, which resolved to a URI
and went through `AssetServer::load` — and `XrdsRuntimeMaterial` is `pub(super)`, so
there was no expert-layer escape either. A decoded frame exists only in memory.
Nothing downstream of the branch knows the difference.

Decoding lives in `xrds-media` behind a new `playback` feature (`video/decode.rs`),
never in the runtime: a runtime that depended on ffmpeg could not ship to a headset.

Measured, 1920×800 at 24 fps, desktop (`cargo run --example video_texture_check`):

| | |
| --- | --- |
| Upload (`write_video_frame`) | **0.28 ms/frame, 0.14 GB/s** |
| Software decode ceiling, unpaced | ~270 fps |
| Effective playback | 23.5 fps against a 24 fps source |

Upload is the only figure that survives to a headset — decode becomes MediaCodec —
and 0.28 ms is comfortable. Scaled to 4096×2048 it is ~11× the pixels, ~3 ms/frame,
which at 72 Hz is already a fifth of the frame budget. **That is the argument for
zero-copy, now with a number attached:** phases 1 and 2 are not optimisation.

### The real bug: writing the image is only half of an update

**Bevy does not rebind a material when the image behind it changes.** This is the one
finding from phase 0 worth carrying forward, and it is not documented anywhere in
Bevy:

- `GpuImage::prepare_asset` rebuilds a modified image into an **entirely new**
  `wgpu::Texture` — it calls `create_texture_with_data` rather than writing into the
  existing one (`bevy_render-0.17.2/src/texture/gpu_image.rs:51`).
- `bevy_pbr` contains **no `AssetEvent<Image>` handling at all**. A material's bind
  group is allocated once, capturing the `TextureView` as it stood at that moment.

So a surface goes on sampling the *first* texture forever. The symptom is a picture
frozen on frame one while every upstream signal reports success: the write lands,
`AssetEvent::Modified` fires, the render asset re-prepares, the upload is measurable
and correctly sized. Nothing in the main world is wrong. The only wrong thing is
which texture a bind group in the render world points at.

`write_video_frame` therefore also re-prepares every material sampling that texture
(`video.rs::rebind_materials_sampling`), which is what re-allocates the bind group
against the current `RenderAssets<GpuImage>`. A regression test asserts it, and fails
without it — the assertion is on `AssetEvent<XrdsRuntimeMaterial>`, not on pixels,
because pixels are not observable from a test.

**A video screen should also be `unlit`.** Otherwise scene lighting modulates the
picture — a bright green card renders as dark olive — which looks wrong and makes a
working screen easy to mistake for a broken one.

This is a **known Bevy gap**, not something particular to this SDK, and it is not
close to being fixed. It has been reported since 0.6, and the workaround everyone
converges on is the one used here:

- [bevy#3674](https://github.com/bevyengine/bevy/issues/3674) — 0.6, 2D materials
- [bevy#17350](https://github.com/bevyengine/bevy/issues/17350) — 0.15 regression
- [bevy#20575](https://github.com/bevyengine/bevy/pull/20575) — an attempt at the
  real fix. **Closed**, blocked on cost (it scanned every asset) and on granularity
  (not every asset should rebuild when a dependency changes, and there was no
  opt-in). The design moved toward many-to-many asset relationships
  ([bevy#11266](https://github.com/bevyengine/bevy/issues/11266)) — architectural
  work with no timeline.

Two consequences worth holding onto:

1. **`rebind_materials_sampling` is long-lived**, not a stopgap awaiting a release.
   Its cost should be treated as permanent and budgeted accordingly.
2. **The reviewers' objection is the same tradeoff we made.** Ours is far narrower —
   one material type, only on frames actually written — but it is the same shape, and
   a scene with many materials would want the dependents cached rather than scanned.

Worth noting who filed that PR: `rectalogic`, the author of `bevy_av1`. Anyone
building video on Bevy hits this wall, and nobody has got past it upstream yet.

### What phase 0 cost, and the method that finally worked

Several rounds of debugging went into this, most of them spent downstream of a
problem that was not there. Two harness faults hid the real bug for most of it:

1. **A test clip too dark to judge.** It averaged 38/255, so "playing correctly" and
   "not updating at all" rendered identically. The example now defaults to a bundled
   clip that opens on a bright green MPAA card.
2. **An unpaced decoder.** It ran at 270 fps for a 24 fps clip — eleven times speed —
   and got 900 frames in before the window even opened, all dropped. Short clips ran
   off the end within seconds, leaving the final frame frozen on the quad, which
   looks exactly like the bind-group bug and masked it. The decode thread now waits
   for the scene to exist, paces against PTS, and loops at end of stream.

Every wrong conclusion in this spike came from reasoning about a stage instead of
observing it, and every correct one came from making something observable:

| Question | What settled it |
| --- | --- |
| Is the decoder correct? | Dumped frames to PNG and looked at them |
| Does a write reach the asset and announce itself? | Headless test on `AssetEvent<Image>` |
| Does the material follow the image? | Headless test on `AssetEvent<Material>` |
| **Does the picture actually change on screen?** | **The example screenshots itself** |

That last row is the one that had been missing, and it is why
`XRDS_VIDEO_SCREENSHOT=<dir>` now exists: it captures the window at three spaced
moments and writes PNGs. "Does the picture change" was the only question the example
could not answer from inside itself, and delegating it to a person looking at a
screen is how a texture frozen since frame one survived three rounds of debugging.
Mean pixel values were what misled throughout; images were unambiguous every time.

Phases 1 and 2 are the real spike. Both are measured on device, against a baseline
without video, using the `handle_events tick frame=N` counter and the method in
`docs/quest-device-test-recipe.md` — with more warm-up windows dropped than feels
necessary, which is the lesson the LOD measurement paid for.

## What can be lifted from HMDViewer

Roughly 640 lines, largely as-is:

- `src/video/decoder.rs` (251 lines) — `AMediaExtractor` + `AMediaCodec`, PTS-paced
  against a wall clock, loops on EOS.
- `src/video/stream_decoder.rs` (158 lines) — the NAL feeder, for a WebSocket
  source rather than a file. Relevant because streaming video is a more compelling
  XR feature than local playback, and `xrds-net` already has WebSocket.
- `src/video/mod.rs` (231 lines) — the `AImageReader` pool. Note
  `IMAGE_KEEPALIVE = 3` and `MAX_IMAGES = 5`: acquired images are kept alive behind
  the current one so the GPU never samples a buffer the reader has recycled. That
  is the kind of detail that is invisible until it corrupts a frame.

`vk_renderer.rs` is *not* liftable — it is a complete standalone Vulkan renderer.
Only its import sequence (lines ~737–800) is of interest, and only for route B.

## What must not be built before this answers

**Not the asset-kind half on its own.** `XrdsSceneAssetKind::Video` alongside
`Audio` is genuinely easy — the Audio variant touches eight files and the pattern is
mechanical. It is also exactly the wrong thing to ship first: an asset an author can
import, that validates, that round-trips, and that plays nothing, is the
authorable-but-inert failure this project has hit repeatedly. Build it when there is
something for it to point at.

## Prior art worth knowing

- [`bevy_av1`](https://docs.rs/bevy_av1/latest/bevy_av1/) 0.3.0 (Aug 2026) targets
  Bevy 0.17, decodes AV1/IVF via dav1d, and is extensible through
  `Decodable + Asset` — the same trait shape `bevy_audio` uses. Documented for
  x86_64-linux; no ARM/Android claim. Useful for phase 0, not for the headset.
- [`bevy_video`](https://lib.rs/crates/bevy_video) targets Bevy **0.9**, last
  released January 2023. Dead.
