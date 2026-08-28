# Post-processing beyond bloom — plan

**Status** (2026-08-26): phase 1 (colour grading) **built**. Phase 2 answered by
reading the code rather than by a device run, and it corrected two claims this
document made when it was written — see "What phase 2 actually found".

Closes the "Post-processing stack" Gap Analysis row, minus SSAO and auto exposure,
which are out of scope by decision — both are compute-based, and this project has
already lost `bevy_hanabi` to a GPU-compute path that rendered nothing on Adreno.

## The framing that decides the scope

A post-processing list for a flat game is not the same list for a headset, and
treating them as one is how this row got sized as "DOF, SSAO, colour grading" in
the first place. Three effects on that list are actively wrong for XR:

- **Depth of field** fights the wearer's eyes. On a flat screen the author chooses
  the focal plane and the viewer accepts it. In a headset the viewer's eyes focus
  where *they* choose, and blurring what they are looking at reads as a broken
  display rather than as an effect. Most shipped VR titles avoid it.
- **Motion blur** is a comfort hazard. A headset already reprojects to hide
  latency; adding smear on top of head motion is a known nausea contributor.
- **TAA** is temporal reprojection, which the compositor is also doing. Two
  reprojections disagreeing produces exactly the artefacts each is meant to hide.

That leaves a much shorter list, and it is a better one: what a headset actually
needs is **colour control** and **edge quality**, because aliasing is far more
visible at close range on a low-PPD panel than it is on a monitor.

## What phase 2 actually found

This section originally said the risk was **multiview**: that the XR path renders
with `array_size: 2`, so a post-process pass would have to handle a two-layer array
texture and Bevy's effects are not written for that.

**That was wrong.** `session.rs:846` spawns *one ordinary camera per eye*, each with
its own `RenderTarget::TextureView` onto a swapchain layer. Bevy sees two normal
single-view cameras; the array lives in the OpenXR swapchain, below Bevy. There is
no two-layer-array problem for a pass.

The real finding is simpler and larger. An XR camera is spawned with exactly:

```rust
Camera { target: TextureView(handle), order: i }
OpenXrCameraIndex, Projection::custom, NoCpuCulling,
NoIndirectDrawing, Camera3d { depth_texture_usages }
```

No `Hdr`. No `Tonemapping`. No `Bloom` — and `Hdr` is inserted nowhere in
`xrds-openxr` or `xrds-runtime`. `XrdsBloom` and `XrdsTonemapping` are applied in
`spawn.rs`, which runs only for cameras spawned through `XrdsAPI`; the OpenXR eye
cameras are different entities that never see them.

**So bloom and tonemapping have never worked in XR, and could not have.** The only
accurate statement about this anywhere in the tree turns out to be a passing remark
in `set_effect_params`: *"the SDK's XR cameras have no HDR/bloom pass"*, which is
why effect colours are clamped to 1.0.

Two consequences:

- **Colour grading was unaffected**, because it is environment-driven and
  `sync_managed_scene_color_grading_in_world` queries `With<Camera3d>` — which
  matches the eye cameras, exactly as exposure, fog and atmosphere do. That is the
  argument for the environment being its home: bloom's camera-field design is
  precisely what excludes it from XR.
- **The phase 3 question changed.** It is no longer "is one pass affordable" but
  "do XR cameras get `Hdr` at all". Step 0b already priced that: `Hdr` adds a float
  intermediate render target, **paid once per eye**, before any effect renders.
  FXAA, SMAA and CAS do not need HDR; bloom does. They are separate decisions now,
  not one row.

The cost prior still stands for anything that *is* a pass. Procedural atmosphere —
one LUT-based sky pass — measured **+18.3 ms/frame on a Quest 3** against a 13.9 ms
budget (`done/editor-task-queue-and-hdr-conversion.md`, step 0b). It rendered
perfectly and was unaffordable.

## Phases

| Phase | Scope | State |
| --- | --- | --- |
| 1 | Colour grading — scene environment + editor | ✅ built 2026-08-26 |
| 2 | Is the XR post-process path viable? | ✅ answered by reading the code — see above |
| 3 | Anti-aliasing | ✅ built 2026-08-26 — as **MSAA**, not a post-process pass |

### Phase 1 — colour grading (built)

**No spike needed, and it was the only item here that needed none.** `ColorGrading`
is read *during* tonemapping rather than being a pass of its own, so neither the
atmosphere cost prior nor anything about the XR render path applies to it.

It went on the **scene environment**, not the camera — this document originally said
"inspector section under the camera", which was wrong on convention and, as phase 2
then showed, would also have excluded it from XR. Every comparable engine puts
tonemap, bloom and grading on a scene-level environment or volume: Godot's
`WorldEnvironment`, Unity's Volume overrides, Unreal's Post Process Volume. Ours
already held `exposure` there, and separating a tonal control from the exposure
feeding it would have been the odd arrangement.

Shipped as `XrdsSceneColorGradingEnvironment`: **temperature, tint, hue (degrees),
saturation**. Applied through the same managed-marker pattern as exposure and fog,
so it reaches every `Camera3d` — including the OpenXR eye cameras.

Three decisions worth keeping:

- **No `exposure` field**, though Bevy's `ColorGradingGlobal` has one.
  `XrdsSceneExposureEnvironment` already owns scene exposure; two EV controls would
  be one quantity with two owners.
- **Degrees, not radians**, for hue — it is a field an author types.
- **A neutral grade is treated as absent** and clears any managed component, so
  "never graded" and "graded back to neutral" are indistinguishable at runtime.
  They look the same, so they should behave the same.

Four fields rather than Bevy's sixteen: the per-range sections (shadows / midtones /
highlights × saturation, contrast, gamma, gain, lift) are additive when something
asks. Shipping all of them at once would be authorable-but-incomprehensible, the
second-order form of the failure this project keeps recording.

### Phase 2 — answered without a device run

See "What phase 2 actually found". The planned probe — one APK with bloom on —
would have measured a thing that cannot happen: XR cameras have no `Hdr`, so bloom
has nothing to bloom. Reading `session.rs:846` cost minutes and answered it
properly; the APK would have produced a null result and invited the conclusion that
passes are broken in XR rather than that bloom was never wired to them.

### Phase 3 — anti-aliasing (built, and not the way this planned)

**MSAA was already running.** `bevy_render`'s `CameraPlugin` registers `Msaa` as a
*required component of `Camera`*, defaulting to `Sample4` — so every camera,
including the OpenXR eye cameras, has been anti-aliasing at 4× all along. Nothing
was missing except a way to author it.

That makes the CAS / FXAA / SMAA plan actively wrong. On a tile-based mobile GPU
MSAA resolves inside tile memory; a post-process AA pass costs a full-screen pass on
the budget least able to afford one — where a single pass has already measured at
more than a whole frame — and produces blurrier edges than the MSAA it would sit on
top of. Building any of them would have made the image worse and the frame slower.

So this shipped as `XrdsSceneAntiAliasingEnvironment`: the **MSAA sample count**,
which is the knob Unity and Unreal both surface for VR and the real
quality-versus-frame-time trade on a headset.

Three things worth keeping:

- **An enum, not an integer.** `Msaa::from_samples` panics outside {1,2,4,8}, and a
  scene document is exactly where an arbitrary integer would arrive from.
- **Clearing restores 4×, it does not remove the component.** `Msaa` is required by
  `Camera`, so a camera always has one; "no authored anti-aliasing" means the
  default is back, not that the component is gone. Every other sync in
  `environment.rs` removes on clear — this one cannot, and the test says so.
- **8× carries a warning in the editor.** It is frequently unsupported on mobile
  GPUs.

The same four lines of `CameraPlugin` also answered two questions this document had
been guessing at: `ColorGrading` and `Exposure` are required components of
`Camera3d` — which is why both already worked on headsets — and `Hdr` is *not*, which
is independently why bloom is the one effect that cannot.

## Explicitly out of scope

| | Why |
| --- | --- |
| **SSAO** | Compute-based (`SpecializedComputePipelines`, `begin_compute_pass`); Bevy's own docs say unsupported on WebGL2 and WebGPU. `bevy_hanabi`'s compute path rendered nothing on Adreno. Decided out 2026-08-26. |
| **Auto exposure** | Compute-based, same reasoning. Manual exposure already ships and is fully authorable. Decided out 2026-08-26. |
| **Depth of field** | Fights eye accommodation in a headset. Reconsider only if a desktop-only workflow asks. |
| **Motion blur** | Comfort hazard on top of compositor reprojection. |
| **TAA** | Temporal reprojection competing with the compositor's. |
| **DLSS** | NVIDIA desktop only; irrelevant to the platform targets. |

## What "done" means for the Gap Analysis row

The row reads "no DOF, SSAO, or color grading". Two of those three are now
deliberately excluded with reasons, which means **the row closes on colour
grading plus whatever phase 3 ships**, and the note should say what was excluded
rather than leaving a future reader to assume it was forgotten.
