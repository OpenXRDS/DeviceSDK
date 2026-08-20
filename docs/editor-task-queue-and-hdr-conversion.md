# Environment authoring: sky, conversion, and a task queue

**Status:** planned, not started. Written 2026-08-20, reordered the same day.

## Read this first: the order changed

This began as "convert a downloaded HDR panorama into something usable", with a
task queue underneath it so the editor would not freeze. Both are still worth
building. Neither should be built first.

Surveying what Bevy already offers turned up two things the editor does not expose,
and one of them makes the conversion pipeline optional for a large class of scenes:

1. **`bevy_pbr::atmosphere` — a procedural sky.** Hillaire 2020 atmospheric
   scattering: add `Atmosphere` to a 3D camera and get a physically-based sky with
   dynamic time of day, driven by the scene's own directional lights, plus
   scattering in front of distant geometry. LUT-based and "fairly cheap" per its
   own documentation. `bevy_pbr` is already in our feature set.

   For an outdoor scene this is *better* than a downloaded panorama, not merely
   cheaper: the sun matches the light that casts the shadows, which a static image
   can never do. It also pairs naturally with terrain, since it handles aerial
   perspective over distance.

2. **`Skybox::rotation`** — a `Quat` on the component, unexposed. This is not
   polish. A converted panorama arrives in whatever orientation it was shot, and
   aiming the sun *is* rotation. Shipping conversion without it delivers a sky the
   author cannot point.

So the original framing — "an author cannot use a downloaded sky" — was answering
the wrong question first. For many scenes the better answer is "you do not need
one". Conversion still matters for authors who bring a specific environment, and
for IBL, but it is no longer the only path to a sky.

**Revised order:**

| Step | Why it comes first |
| --- | --- |
| 0a. Expose `Skybox::rotation` | Trivial; conversion is incomplete without it, and it improves the two shipped maps today |
| 0b. Atmosphere spike, verified on Quest | May remove the need for conversion in most outdoor scenes. Cheap to try, and **unverified on Adreno** — the `bevy_hanabi` lesson says check before committing |
| A. Task queue | Independently useful regardless of what follows |
| B. Equirect → cubemap | Only for authors bringing their own environment |
| C. IBL prefilter | Real graphics work; own pass |

Two caveats on the atmosphere before leaning on it: upstream states it is untested
with volumetric fog, and nobody has run it on an Adreno GPU. That is a device check
before commitment, not after.

Also cheap and adjacent: **fog is linear-only** here
(`XrdsSceneFogEnvironment` is colour/start/end), while Bevy's `FogFalloff` offers
exponential, squared-exponential and atmospheric. The last pairs with step 0b.

---

The original plan follows, still accurate for steps A–C.

## Why

An author who downloads an environment — the ordinary way to get one, e.g. a
stitched HDR panorama from ambientCG — cannot use it. The SDK ships two pre-baked
cubemaps in `assets/environment_maps/` and offers no way to make more.

The obstacle is a format one, and it is absolute rather than a matter of tagging:
Bevy's `Skybox` and `EnvironmentMapLight` both require a **cube** texture, while a
downloaded panorama is a single **equirectangular** 2-D image. Radiance `.hdr` has
no cubemap concept at all, so no import-time classification can make one work. The
missing piece is a conversion, and it does not exist anywhere in the tree.

This surfaced while adding the Skybox section to the editor
(`docs/small-phases-plan.md`, S6-adjacent work): asset-kind detection was fixed to
read the KTX2 header — `faceCount == 6` means cubemap — which correctly identifies
the SDK's own maps, and equally correctly refuses to pretend a panorama is one.

## Part 1 — a general task queue

Conversion is slow enough to be visible (seconds), and it must not freeze the
editor. Rather than a third bespoke background job, this generalises what already
exists.

### What is there now

`ExportJob` and `ApkExportJob` in `editor_state.rs`: a `std::thread::spawn` plus
`Arc<Mutex<Option<Result<String, String>>>>`, polled each frame with `try_lock`,
each with its own snapshot boolean (`is_exporting`, `is_exporting_apk`) and its own
UI. The pattern works; it just does not scale to a third caller, and neither job
can report progress — only "running" or "done".

### Model

```rust
struct EditorTask {
    id: u64,
    label: String,          // "Converting sky.hdr"
    state: TaskState,       // Queued | Running | Done | Failed | Cancelled
    progress: Option<f32>,  // None = indeterminate, else 0.0..=1.0
    detail: Option<String>, // current step, or the error when Failed
    cancel: Arc<AtomicBool>,// checked by the worker at its own granularity
}
```

- **A queue, not just a set.** Conversions are CPU- and memory-heavy; running six
  at once because someone dropped a folder in would be worse than running them in
  order. A small concurrency cap, default 1 for conversions.
- **Progress is `Option`.** An APK build cannot honestly report a fraction, and a
  fake progress bar that jumps to 90% and sits there is worse than a spinner.
  Conversion *can* report real progress (faces, then mip levels), so the model must
  express both rather than forcing one.
- **Finished tasks linger** until dismissed, so a failure that lands while the
  author is looking elsewhere is still there when they look back. This is the
  common failure of status-bar-only reporting.

### Scope: author-initiated jobs only

**This queue is for visible, author-initiated work.** Slow, few, and something the
author is waiting on: conversion, export, baking. Anything high-frequency or
system-initiated must not report here.

Written down because the boundary is not obvious and the wrong side of it is
ruinous. Terrain is the case that makes it concrete — it is on the Gap Analysis,
and it brings two kinds of work that look alike and are not:

| | Author-initiated | System-initiated |
| --- | --- | --- |
| Terrain | import a heightmap, bake chunks, generate collision | stream chunks as the player moves |
| LOD | bake the LOD chain | select a level per frame |
| Environment | convert a panorama | — |

The right-hand column would break this design in three specific ways: "finished
tasks linger until dismissed" buries a real failure under thousands of successes;
FIFO is wrong when the chunk ahead of you matters more than the one behind; and
there is no cancellation, which is streaming's most common event.

A shared thread pool underneath is fine. A shared queue *with a UI* is not. If
streaming ever needs scheduling, it needs its own — priority-ordered, cancellable,
and silent.

### Cancellation

In the model from the start, not added later. Immediately useful — an author who
starts converting a 16K panorama and realises it is the wrong file should be able
to stop it — and it is the single thing a future scheduling mechanism would most
need if any of this is ever shared. Retrofitting cancellation through a thread
that only reports a final `Result` means rewriting every job.

### UI

A task strip in the status bar showing the active task and a count, expanding to a
list. Failures stay until dismissed and carry their error text.

**Existing jobs migrate onto it**, so APK export and app export report through the
same surface. That is the test of whether the abstraction is right: if they do not
fit, it is the wrong shape.

## Part 2 — the conversion

### Phase order matters here

Equirect → cubemap and IBL prefiltering are different jobs of very different size,
and the plan should not pretend otherwise.

**Phase B — skybox cubemap.** Project the panorama onto six faces. One render pass
per face, or a straightforward CPU loop; no filtering. Sufficient for `Skybox`,
which is what an author sees first and asks for by name.

**Phase C — IBL prefiltering.** `EnvironmentMapLight` needs a diffuse irradiance
map and a specular chain prefiltered by roughness (the shipped `specular.ktx2` has
11 mip levels, `diffuse.ktx2` has 1). This is GGX importance sampling — real
graphics work, and worth its own pass rather than being smuggled into Phase B.

Until Phase C, a converted panorama lights nothing; it is a backdrop. That must be
said in the UI, or it becomes another silent half-feature.

### The input contract — decided 2026-08-20

What the editor accepts as an environment source. Settled by looking at a real
download rather than reasoning about formats: `DayEnvironmentHDRI043_4K.zip` from
ambientCG.

| Accepted | Notes |
| --- | --- |
| Equirectangular **`.exr`**, 2:1 | OpenEXR. **The required format** — what vendors ship |
| Equirectangular **`.hdr`**, 2:1 | Radiance RGBE. Already enabled, so accepted for free |
| **`.ktx2`** with `faceCount == 6` | Already a cubemap; passes through unconverted |

Everything else is refused, including `.zip` — an author extracts first.

**What the vendor package actually contains**, which is what settled this:

```text
 30.89 MB  DayEnvironmentHDRI043_4K_HDR.exr        <- the payload
  5.70 MB  DayEnvironmentHDRI043_4K_TONEMAPPED.jpg
  1.14 MB  DayEnvironmentHDRI043_4K.blend
  0.32 MB  DayEnvironmentHDRI043.png
  0.00 MB  DayEnvironmentHDRI043_4K.usdc
  0.00 MB  DayEnvironmentHDRI043_4K.tres           <- Godot stub
```

One interchange image plus per-engine wrappers, and *no cubemap* — confirming from
the supply side that every engine builds its own. The `.exr` is 4096×2048, RGBA
half-float, PIZ-compressed.

**`.exr` could not be read at all.** Bevy's `exr` feature was off and the `exr`
crate was absent from the lockfile entirely, so the single most likely file an
author downloads was refused at import. Now enabled — but as an `image` dependency
of `xrds-editor` rather than a Bevy feature on the workspace, so an OpenEXR decoder
does not ship to a headset that only ever loads converted KTX2. Verified against
the real file: PIZ decodes in 0.48 s, and the header-only dimension read the
classifier uses takes 5.3 ms.

#### Why LDR is refused rather than merely discouraged

Measured on the two halves of that same download — same capture, same resolution,
solid-angle weighted so polar rows do not distort the average:

| | max | mean luminance | light above 1.0 |
| --- | --- | --- | --- |
| `_HDR.exr` | 17312.0 | 2.5645 | **84.4%** |
| `_TONEMAPPED.jpg` | 1.0 | 0.4492 | 0% |

**84.4% of this environment's light lives above 1.0** and is absent from the JPEG.
As a backdrop it merely looks flat — the sun clamps to exactly the brightness of a
cloud, and since the JPEG is already tonemapped, the renderer tonemaps it a second
time and it reads milky. As *lighting* it is broken: no dominant direction, no
sun, no directional shadow, no sharp specular. Raising skybox brightness cannot
recover it, because scaling clipped values brightens sun and cloud together.

Every major engine *does* accept LDR as a backdrop while documenting HDR for
lighting, and all of them split the two concepts — `scene.background` vs
`scene.environment` in three.js, `PanoramaSkyMaterial` vs sky radiance in Godot.
XRDS has the same split already. So allowing LDR as a skybox-only backdrop stays
open as a later option; it is the same conversion pipeline with a different
decoder. It is simply not Phase B, and shipping it *without* the distinction would
invite exactly the `_TONEMAPPED.jpg` mistake this package makes easy.

Hence `EnvironmentSourceError::NotHighDynamicRange` names the fix rather than the
fault: the file an author wants is usually sitting in the folder they just browsed.

### How to convert — decided 2026-08-20, after a survey and a working probe

The options were: (1) in-process with Bevy's renderer, (2) the Khronos `ktx` CLI,
(3) a Rust crate. The survey turned (3) into a split, and that split wins.

**No single Rust crate does this.** [`ktx2`](https://crates.io/crates/ktx2) is a
read-only parser. [`gltf-ibl-sampler-egui`](https://github.com/pcwalton/gltf-ibl-sampler-egui)
— by a Bevy renderer contributor, targeting Bevy's KTX2 exactly, and very plausibly
what produced the maps we ship — does the whole job but is a **GUI binary with no
library API**, bundling the Khronos C++ sampler and needing a Vulkan GPU.

**[`ctt`](https://lib.rs/crates/ctt) 0.5.0** (MIT/Apache/Zlib) is the piece worth
taking: a Rust library that writes KTX2 with cubemap faces, mip levels and optional
zstd supercompression, and encodes BC6H/ASTC later if we want them. It does *not*
do equirect projection or IBL prefiltering.

| Part | Who | Why |
| --- | --- | --- |
| Decode `.exr` / `.hdr` | `image` | Already in the editor for the import contract |
| Equirect → 6 faces | **us**, CPU | Direction math; small, parallel, testable |
| GGX prefilter (Phase C) | **us**, CPU | Well-documented; the task queue exists for its runtime |
| Write KTX2 | **`ctt`** | Container, DFD and face/mip layout are fiddly and easy to get subtly wrong |
| BC6H / ASTC | `ctt` encoders, later | For the VRAM problem below; costs a C++ toolchain |

**Option (1) is rejected, and not on performance.** A CPU converter runs headless,
so it is unit-testable — and *every* environment-map bug in this project so far was
found by a person running the editor, never by a test. In-process rendering would
also mean render-graph plumbing, async readback, and contending with the viewport
for the GPU, to speed up something already measured at half a second.

Use `gltf-ibl-sampler-egui` anyway, as a **cross-check oracle**: same input through
both, compare outputs. Better than eyeballing a sphere.

#### What the probe established

Against the real 4096×2048 ambientCG `.exr`, headless:

```text
decoded 4096x2048        270 ms
projected 6x512² faces    13 ms
encoded KTX2 (zstd-9)    205 ms   -> 10.83 MB
```

Output header versus the map we ship:

| | Format | Size | Faces | Mips | Supercompression | File |
| --- | --- | --- | --- | --- | --- | --- |
| probe output | RGBA16F | 512² | 6 | 10 | Zstandard | **10.8 MB** |
| shipped `specular.ktx2` | RGBA16F | 1024² | 6 | 11 | none | 67.1 MB |

And an orientation check, which a valid header cannot give you — a flipped axis or
wrong face order still produces a structurally perfect cubemap:

```text
+Y(up)    mean luminance 9.2260   <- open sky, brightest
-Y(down)  mean luminance 0.2187   <- ground, darkest
```

A 42× ratio the right way round, so the face order and axis signs are right.

**`ctt` API notes for whoever implements this.** `Image { surfaces, kind }` takes
raw in-memory buffers with `surfaces[face][mip]`. `ConvertSettings::mipmap = true`
*downsamples* to complete the chain — correct for a skybox, **wrong for the
specular chain**, whose mips are roughness levels that must be computed. Phase C
supplies its own levels and sets `mipmap = false`.

**Caveats.** `default-features = false` drops all five encoder bindings, but `ctt`
still pulls `zstd-sys`, which compiles C — this workspace otherwise avoids that by
using Bevy's pure-Rust `zstd_rust`. Editor-only, and it built without complaint.
Unverified: that Bevy loads a *zstd-supercompressed* KTX2 (we enable `zstd_rust`,
so it should — but "should" is exactly what 0b punished), and visual correctness,
since the probe point-sampled. Production needs real filtering: with a sun at
17312, aliasing will be violent.

#### Sizing: do not reproduce what we ship

The two shipped maps total **117 MB, uncompressed RGBA16F**, which is VRAM as well
as disk. Two things to fix rather than copy:

- `diffuse.ktx2` is **1024² with a single mip**. Diffuse irradiance is an extremely
  low-frequency signal and is conventionally 32² or 64² — roughly 50 KB, not 50 MB.
  `gltf-ibl-sampler-egui` defaults to "max 4K per side", which likely explains it.
- Nothing is supercompressed, though `zstd_rust` is already enabled. The probe got
  6× smaller partly this way.

#### Runtime format: verified, and not what was assumed

Both open risks were checked against Bevy's real loader (`ktx2_buffer_to_image`)
rather than reasoned about.

**Zstd supercompression loads.** Our converted file comes back as `Rgba16Float`,
512×512×6, `view_dimension = Cube` — the same shape as the shipped map. But note
what it costs: **10.8 MB on disk decompresses to 16.78 MB in memory.**
Supercompression is a file-size and APK-size win only; it does nothing for VRAM.

**"ASTC HDR on Adreno" was wrong, and would have wasted Phase C.** Bevy's
`CompressedImageFormats` has exactly three flags — `ASTC_LDR`, `BC`, `ETC2` — and
its own source says why:

```text
// NOTE: Rgba16Float should be transcoded to BC6H/ASTC_HDR. Neither are supported by
// basis-universal, nor is ASTC_HDR supported by wgpu
```

So there is **no compressed HDR path on Adreno at all**, whatever the device
reports. BC6H is mapped and real, but BC is desktop-only.

**`Rgb9e5Ufloat` is the answer for a headset**, and a better one than compression
would have been: shared-exponent RGB at 4 bytes/px against RGBA16F's 8. Verified
end to end — `ctt` writes `E5B9G9R9_UFLOAT_PACK32`, Bevy's loader maps it, and it
arrives as `Rgb9e5Ufloat` 512×512×6 cube at **8.39 MB against 16.78 MB, exactly
half**. Crucially it is *not* a compressed format, so it loaded with
`CompressedImageFormats::NONE` and needs no device feature flag.

Its range tops out near 65408 with a 9-bit mantissa; the sun in the sample panorama
peaks at 17312, so it fits with room to spare. It drops alpha, which an environment
map does not use.

| Target | Format | Bytes/px | 512² cube in VRAM |
| --- | --- | --- | --- |
| Desktop | BC6H | 1 | ~2.1 MB |
| Headset | `Rgb9e5Ufloat` | 4 | 8.39 MB |
| Today | `Rgba16Float` | 8 | 16.78 MB |

**Verified on a Quest 3, 2026-08-20.** A scene carrying a 512² `Rgb9e5Ufloat`
skybox converted from the ambientCG panorama, plus a lit cube as a liveness control
— so a black sky could not be confused with a dead app. Both rendered. No panics,
no wgpu validation errors, scene loaded with 3 entities.

So the runtime format for a headset is settled: **`Rgb9e5Ufloat`, half the VRAM of
what we ship, no device feature flag required.** Unlike 0b, this one survived
contact with the hardware.

Frame time during the check ran ~4.8 ms, comfortably inside the 13.9 ms budget —
but treat that as "no alarm raised" rather than a measurement. It was sampled over
a different window than the 0b figures and the counter ran faster than display
refresh, so it is not comparable to them.

**Resolution remains the bigger lever anyway.** Dropping the diffuse map from 1024²
to 32² is a ~1000× saving; no format choice comes close.

## Phasing

| Phase | Scope | Notes |
| --- | --- | --- |
| 0a | Expose `Skybox::rotation` | Trivial. Do first — see the reordering at the top |
| 0b | ~~Procedural atmosphere, verified on Quest~~ | **Done, and measured — see below** |
| A | ~~Task queue + UI, existing export jobs migrated~~ | **Done — see below** |
| B | ~~Equirect → cubemap for skybox~~ | **Done** — verified in the editor |
| C | ~~IBL prefilter (diffuse + specular chain)~~ | **Done** — see below |

Phase A is worth doing regardless of whether B and C ever happen: it removes two
bespoke job implementations and gives long operations somewhere honest to report.

**B and C may never be needed.** If the atmosphere covers outdoor scenes and the
two shipped cubemaps cover indoor ones, conversion serves only the author who wants
one *specific* environment. That is a real use, and a much weaker reason to build a
GPU cubemap pipeline than "you cannot have a sky at all" was. Re-judge after 0b
rather than treating this plan as committed.

## 0b result: works on Adreno, costs more than a whole frame budget

Measured on a Quest 3, 2026-08-20. Two APKs from the same generated scene — a
200×200 ground plane, one shadow-casting directional light, one cube — differing
only in whether `atmosphere` was set. Frame time taken from the
`[XR-DIAG] update_view_proj#N` counter, which logs every 90 frames, so consecutive
timestamps give the period of 90 frames directly.

| | Frame time | Rate |
| --- | --- | --- |
| Without atmosphere | ~13.0 ms | ~77 fps (at display cap) |
| With atmosphere | ~31.3 ms | ~32 fps |
| **Cost** | **+18.3 ms/frame** | |

The 72 Hz budget is 13.9 ms. **Atmosphere alone costs about 1.3× the entire frame
budget**, while everything else in the scene fits comfortably inside it. That is not
a tuning gap; it is a different order of magnitude.

**It renders correctly** — sky, horizon, aerial haze and the sun disc all confirmed
by eye on device, with no validation errors and no panics. This is not the
`bevy_hanabi` outcome, where the feature simply produced nothing on Adreno. It
works; it is unaffordable at default settings.

Two things were needed to get it working at all, both worth keeping in mind if this
is revisited:

- `Atmosphere` requires Bevy's `Hdr`, adding a float intermediate render target —
  paid once per eye.
- The `render_sky` pass **samples the depth texture**, and `Camera3d::default`
  creates depth as `RENDER_ATTACHMENT` only. Bevy adds `TEXTURE_BINDING` itself for
  occlusion-culling views and *not* for atmosphere, so the bind group fails
  validation without an explicit fix on our side.

### Decision: ship it as an advanced, opt-in feature

Not removed — it works, and desktop and exported desktop apps have the budget for
it. Not defaulted on — an XR scene that enables it drops to under half the target
frame rate, and a default that is pleasant on desktop and unusable on a headset is
the wrong default for this SDK. Unreal defaults its sky atmosphere on because it
targets hardware where the intermediate is nearly free; we do not have that.

The editor states the measured cost rather than a vague warning, so the choice is
informed at the point it is made.

**Not attempted, and the obvious next question:** `AtmosphereSettings` exposes LUT
sizes and per-ray sample counts, and the defaults are desktop-tuned. Whether 18 ms
can be brought under ~3 ms that way is unknown and would be its own measurement.
Recorded as a lead, not a plan.

### Consequence for B and C

The premise that "the atmosphere may make conversion unnecessary for outdoor
scenes" **does not hold for XR**. On a headset an author still needs a cubemap, so
conversion keeps whatever priority it had. On desktop the atmosphere is a genuine
alternative.

## Phase A result: the migration paid for itself before conversion exists

`apps/xrds-editor/src-tauri/src/task_queue.rs`, plus a `TaskStrip` in the
frontend. Both export jobs are now tasks; `ExportJob` and `ApkExportJob` are gone,
along with the two `Arc<Mutex<Option<Result<..>>>>` pairs and — importantly — the
*two separate polling sites* they had grown (`bevy_scene.rs` and
`bevy_bridge.rs` were both watching the desktop export).

The abstraction test in this plan was "if the existing jobs do not fit, it is the
wrong shape". They fit, and both got things they did not have:

- **Cancellation.** `TaskContext::run_child` polls `try_wait` rather than
  blocking on `wait`, so a cancel actually kills the compiler. An APK build runs
  for minutes and previously could not be abandoned — the dialog's "Cancel" was
  greyed out while building, so realising you had picked the wrong folder meant
  waiting it out.
- **Streamed output for the desktop export**, which previously wrote to the
  editor's own stdout, i.e. nowhere an author could read it.

Three things worth recording:

**Lanes, not a global cap.** `Build` is capped at 1 because cargo locks the target
directory: a desktop export and an APK build started together did not run in
parallel before, they ran one at a time while both claimed to be running. Making
the second one visibly `Queued` describes what was already happening.

**`Cancelling` is a state.** A cargo build does not stop when asked, and reporting
"Cancelled" while it still holds the target-dir lock would make the next queued
build look stuck for no visible reason. Relatedly, a killed build exits non-zero;
the queue reports that as cancelled rather than failed, because telling an author
their own stop request "failed" is a lie about their own action.

**It fixed the reported dialog bug as a side effect.** "After clicking Export APK
the popup disappears" was `phase === "building" && !snapshot.is_exporting_apk`:
`phase` was set synchronously, but the next snapshot still had the flag `false`
because the backend had not drained the command yet. *A boolean going low cannot
distinguish "not started" from "finished".* The dialog now watches its own task by
monotonic id, which answers the question that was actually being asked.

Also removed: `EditorState::is_selected`, which evaluated `contains(id)`, discarded
it, and returned `false`. Dead — every caller used `selection.contains` directly —
but a trap with a correct-looking name.

### What the first real run caught: cancellation wedged the lane

Verified in the editor — export and cancel both work for the desktop app and the
APK. Getting there took three fixes, and the first is the one worth remembering.

**`kill()` does not kill a process tree.** Killing cargo leaves its rustc children
alive, and they inherited the stdout/stderr pipe handles. `run_child` joined its
reader threads unconditionally, so the readers blocked on a pipe a surviving
grandchild still held; `run_child` never returned, the task sat in `Cancelling`
forever, and — because a task in `Cancelling` still occupies its lane — every later
build was refused with "Busy". The symptom was an APK export stuck at "Starting…"
*after an application export had been cancelled*, which points at the wrong feature
entirely.

The drain is now bounded (2s), after which the readers are left detached; they hold
an `Arc`, so late lines still land in the log. The test that guards this asserts the
invariant rather than the mechanism: **a cancelled build must release its lane.**
Killing the whole process tree would be better still, and is platform-specific work
nobody needs yet — a wedged lane was the actual damage.

**A refusal behind a modal is a hang.** `ExportApk` has five guards that reject
before any task exists, each setting a status toast — which renders *behind* the
modal dialog. The dialog set `phase = "building"` optimistically and waited forever
for a task that was never created. It now shows the refusal and returns to "ready",
with a frame-count backstop for any future guard that forgets to set a message.
Generalisable: **optimistic UI needs a path for "the backend said no".**

**A guard mirrored in the UI drifts from the one that enforces it.** The dialog's
`canExport` checked `!is_dirty` but not `has_save_path`, while the backend checks
both, so a never-saved scene left the button enabled and the export silently
refused.

## Phase C result: IBL, and the sizing lesson applied

`ibl.rs`. Split-sum (Karis 2013): a cosine-weighted diffuse irradiance map and a
GGX-prefiltered specular chain whose mips are roughness levels. The BRDF term is
not generated — Bevy already has that LUT built in.

**Two conventions were read out of `bevy_pbr`'s shader rather than assumed**, since
guessing either produces lighting that looks plausible and is wrong:

- `radiance_level = perceptual_roughness * (textureNumLevels - 1)`, so mip *m* is
  `perceptual_roughness = m / (levels - 1)`. Note *perceptual* — Bevy squares it to
  get the GGX alpha, so the generator must square it too.
- The diffuse map is sampled at **mip 0 only**, so it needs exactly one level and
  no reason to be large.

**Handedness needed no adjustment.** `environment_map.wgsl` negates the sample
direction's z ("cube maps are left-handed") and `skybox.wgsl` does exactly the
same, so the face convention already verified for the skybox is correct here. Worth
having checked: "the reflection turns opposite" has bitten this project before.

**The source mip pyramid is what stops the sun becoming fireflies.** Importance
sampling takes a few hundred directions per texel; where the source holds a feature
far brighter and smaller than the sample spacing — 17312 against a mean of 2.6 —
whether any sample lands on it is luck, and neighbouring texels get wildly
different answers. Sampling from a mip level chosen by the sample's solid angle
averages the feature in before the luck applies.

### Measured against the maps we ship

Real ambientCG panorama, release build, ~1 second total:

| | Generated | Shipped | |
| --- | --- | --- | --- |
| diffuse | 32²×6, 1 mip, **24 KB** | 1024²×6, 1 mip, 50.3 MB | **2048×** smaller |
| specular | 512²×6, 10 mips, 8.39 MB | 1024²×6, 11 mips, 67.1 MB | 8× smaller |
| **total VRAM** | **8.41 MB** | 117.4 MB | **14×** |

Both verified through Bevy's own `ktx2_buffer_to_image` as `Rgb9e5Ufloat` cubes
with the expected level counts.

The diffuse figure is the sizing note in this document acted on rather than merely
recorded: irradiance is a cosine-weighted integral over a whole hemisphere and has
no detail left to carry, so 32² is indistinguishable from 1024² and 2048× cheaper.

### One import, three maps

Importing a `.exr` produces the skybox cubemap *and* both IBL maps, and registers
all three. Splitting them would leave an author with a sky that lights nothing and
no indication a second step existed — which is precisely the "backdrop only"
half-feature this phase was written to avoid. The skybox is generated first, so it
appears while the prefilter (most of the wall clock) is still running.

## Done when

An author drops a downloaded panorama into the editor, watches a progress entry
finish without the editor freezing, and picks the result in the Skybox section —
and, after Phase C, in IBL too.
