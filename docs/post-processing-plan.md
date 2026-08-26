# Post-processing beyond bloom — plan

**Status:** planned, not started. Written 2026-08-26.

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

## What is unknown, and it is not cost

**No post-processing pass has ever been verified in the XR path.** Bloom ships and
is authorable, but nothing in the tree records it rendering correctly on a headset —
the only mention is a VFX-plan warning not to *assume* it.

The reason to doubt it is specific: the XR path renders **multiview with
`array_size: 2`** (`backends/vulkan.rs`). Every post-process pass therefore has to
operate on a two-layer array texture, and Bevy's effects are not written with that
in mind. So the first question is not "what does it cost" but "does it produce a
correct image in both eyes at all".

The cost question has a prior worth respecting. Procedural atmosphere — *one*
LUT-based sky pass — measured **+18.3 ms/frame on a Quest 3** against a 13.9 ms
budget (`done/editor-task-queue-and-hdr-conversion.md`, step 0b). It rendered
perfectly and was unaffordable. Whether that is atmosphere-specific or true of
full-screen passes generally is exactly what nobody has measured.

## Phases

| Phase | Scope | Answers |
| --- | --- | --- |
| 1 | Colour grading — SDK + editor | — (no pass; see below) |
| 2 | Device probe: does bloom render correctly in both eyes, and what does it cost? | Is the XR post-process path viable at all? |
| 3 | Anti-aliasing (FXAA / SMAA / CAS), gated on phase 2 | Is edge quality affordable? |

### Phase 1 — colour grading

**No spike needed, and it is the only item here that needs none.** `ColorGrading`
is a camera *component* applied just before tonemapping, inside a step the renderer
already runs. There is no additional full-screen pass, so the atmosphere prior does
not apply and neither does the multiview question.

It is also the deepest authoring surface on the list: three tonal ranges (shadows,
midtones, highlights) × five controls each (saturation, contrast, gamma, gain,
lift), plus global exposure, temperature, tint, hue and `post_saturation`. That is
what an author reaches for when a scene "looks wrong" and no single light is at
fault.

Shape follows `XrdsBloom`/`XrdsTonemapping` exactly:

1. `XrdsColorGrading` in `xrds-components`, projected to Bevy's `ColorGrading`.
2. Scene-graph payload on the camera, round-tripped.
3. Runtime projection.
4. Inspector section under the camera, grouped by tonal range.

**Do not expose all sixteen fields at once.** The reason is on record: this SDK's
recurring failure is authorable-but-inert, and the second-order version is
authorable-but-incomprehensible. Ship global exposure/temperature/tint/saturation
first — the four an author actually turns — and add the per-range sections once
something asks for them. `XrdsSceneAtmosphereEnvironment` is kept free of physical
parameters for the same reason.

### Phase 2 — probe the XR post-process path

One APK, bloom on, on device. Two questions, in order:

1. **Does it render correctly in both eyes?** Multiview is the risk. A pass that
   writes only layer 0 gives one correct eye and one stale one — which reads as a
   headset fault, not a rendering bug, and is exactly the class of thing that took
   three debugging rounds during the video work.
2. **What does one pass cost?** Same method as step 0b: two APKs from one scene
   differing only in the effect, frame time from the `handle_events tick frame=N`
   counter.

This is deliberately bloom rather than a new effect: it is already implemented and
authorable, so the probe adds no code to the thing being measured.

**If bloom is wrong or unaffordable in the XR path, phase 3 does not happen** and
the row closes at colour grading plus a recorded reason. That is a legitimate
outcome, and cheaper to discover here than after three effects are built.

### Phase 3 — anti-aliasing, gated on phase 2

Aliasing is worse in a headset than on a monitor — low pixels-per-degree, and the
edges move with the head. If a pass is affordable, this is where the frame time
buys the most.

Three candidates, cheapest first, all raster (no compute):

- **CAS** (`contrast_adaptive_sharpening`) — one cheap pass. Not AA, but it
  recovers apparent sharpness lost to the compositor's own resampling.
- **FXAA** — single pass, cheapest true AA.
- **SMAA** — better edges, more passes.

Pick by measurement, not by preference. Ship one, not three: a settings menu of
near-identical AA modes is a worse authoring surface than one that works.

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
