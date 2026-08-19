# DeviceSDK Overall Progress

Last updated: 2026-08-19

## Project Goal

Provide a non-expert-first SDK to build XR applications, with:

- a simple default application surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`)
- a durable scene document model (`xrds-scene-graph`)
- an expert escape hatch for direct engine-level control when needed

## Overall Completion (Estimated)

Estimated overall progress toward a strong SDK basement for XR applications: **95%**.

## What 1.0 Means

The remaining work in this document comes almost entirely from the Gap Analysis —
that is, from comparing DeviceSDK against what Unity, Godot, and Unreal already
give an author. That comparison is the definition of done we are using:

> **When the Gap Analysis rows are closed, including their editor halves,
> DeviceSDK is version 1.0.**

This is deliberately a *parity* bar, not a feature-ambition bar. It says an author
who knows a conventional 3D engine can sit down with DeviceSDK and not hit a wall
where a familiar capability is simply absent. It does **not** promise anything
beyond parity, and rows explicitly deferred as Low priority (Terrain, Video) are
judged against whether an XR workflow needs them, not against whether Unity has
them.

The blueprint below sizes that remaining work. Editor-only halves are called out
as such, because several rows are finished on the SDK side and missing only
authoring UI.

## General 3D Editor Progress

Estimated progress toward a general 3D content editor: **91%**. (Up from 88%:
the Text3D-rendering and texture-slot-UI gaps that were the two concrete
blockers here are both closed — see "What still keeps the editor from being
fully polished" below for what remains.)

What is complete:

- Full GUI editor (`xrds-editor`) with hierarchy, inspector, palette, viewport, toolbar, menubar
- Gizmo interaction (translate/rotate/scale), multi-select, copy/paste, undo/redo
- Play mode with locomotion (flying and grounded), ESC-to-exit HUD
- Scene save/load from disk and file dialog
- Primitive creation palette — Cube, Sphere, Cylinder, Plane, Tetrahedron, Camera, 3 light types, Ambient Light, Audio Clip, Interaction Zone, Text 3D, Player Spawn
- Material authoring panel — base color, emissive, opacity, roughness, metallic, unlit, double-sided, alpha mode, texture slots
- Animation playback controls and morph target sliders in inspector
- Scene environment preview (fog, exposure, IBL, skybox) in real-time
- Export as GLB (full scene or selection)
- Export as Application — bundles scene + assets into a standalone Rust runner, builds with `cargo build --release`, reveals binary in explorer

What still keeps the editor from being fully polished:

- Editor visual polish pass **in progress, not complete**: Tailwind + Radix UI
  migration landed for infrastructure and the trigger-action components
  (Stages 1–3); the older shared HUD/library CSS classes and the rest of the
  component tree have not migrated yet.
- `XrdsCapsule` primitive not yet added (character/physics collider shape).
- `Video` asset kind still deferred — no concrete workflow has required it.

Resolved since the last update, corrected below:

- ~~`Text3D` renders via `Text2d` overlay~~ — `XrdsText` and every panel-widget
  text element (`Label`, `Button`) render through `bevy_rich_text3d`'s
  `Text3d`/`Text3dStyling` as real 3D geometry; there is no `Text2d`/`Camera2d`
  dependency anywhere in the runtime. The egui-based editor this workaround
  referenced has itself been deleted (superseded by the Tauri editor, and it
  no longer built).
- ~~Advanced material GUI (texture slot UI not wired)~~ — `TextureSlotRows` in
  `Inspector.tsx` browses catalog textures and assigns them to BaseColor /
  Normal / MetallicRoughness / Occlusion / Emissive via
  `SetNodeMaterialTexture`. Fully wired.

## Progress Breakdown

| Area | Status | Percent |
| --- | --- | --- |
| Core SDK surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`) | Stable; 70+ typed methods; material texture slots added | **92%** |
| Scene document model (`xrds-scene-graph`) | All asset kinds, 11 payload types, hierarchy, materials, audio, round-trips | **95%** |
| Runtime projection (`xrds-runtime`) | All built-in types, audio playback, environment, export | **90%** |
| Scene environment policy | IBL + skybox + exposure + linear fog; document-driven and runtime-driven | **97%** |
| Physics | avian3d v0.4; Static/Dynamic/None bodies; per-primitive colliders; grab/throw; raycasting; interaction zone sensors; scene-doc serialized | **90%** |
| Asset workflow | Gltf, Texture, EnvironmentMap, Audio — catalog, validation, diagnostics, runtime | **92%** |
| GUI editor | Functional editor with all core panels; text3d and texture-slot UI gaps closed; Panels workspace (in-world UI authoring) added; visual polish (Tailwind/Radix) partial | **92%** |
| Export pipeline | Export as Application (Windows/Linux/macOS validated). Scene GLB export **retired** — glTF cannot represent panels/triggers/Tracks/anchors, so it wrote a mesh dump that looked like a scene save. glTF *import* unaffected. | **95%** |
| Docs/examples/test coverage | Round-trip tests for all node types; 3-platform QA passed; regression suite complete | **88%** |

## Completed Highlights

- Non-expert-first SDK layering established and documented.
- Runtime-first and document-first flows both work and are tested.
- Scene document round-trip and runtime import/export operational.
- Scene environment policy end-to-end: document authoring, runtime policy, validated projection.
- `XrdsSceneAssetKind` has four distinct kinds: `Gltf`, `Texture`, `EnvironmentMap`, `Audio`.
- Inspector read/write API complete for camera, all four light types, glTF, all mesh primitives, and material texture slots.
- GUI editor fully functional: hierarchy (drag-drop reparent), inspector (per-payload sections), palette (drag + double-click), viewport (gizmo, orbit/fly camera, orientation indicator), toolbar (undo/redo, status, shortcuts).
- Play mode: viewport hides editor panels, shows crosshair + ESC hint HUD; locomotion (flying / grounded).
- Undo/redo system (Ctrl+Z/Y) with history count display; clipboard (Ctrl+C/V).
- Template system: Simple 3D, Basic Interactive, VR Experience (PlayerSpawn + locomotion), Platformer (kinematic gravity + jump).
- Export as Application: generates standalone Cargo runner, bundles assets, builds with `cargo build --release`, opens output in explorer. Validated on Windows, Linux (Ubuntu), macOS.
- Asset bundling: relative URI generation (forward slashes), absolute URI pass-through, resolve-against-scene-dir, relative asset copy with subdirectory preservation, absolute asset flattening.
- Round-trip tests for all light types, Camera, Text3D; regression tests for all non-XR examples.
- Every XRDS camera automatically becomes the spatial audio listener (`SpatialListener`).
- SVG icon system integrated into editor panels.
- Performance stats overlay (FPS, frame time, mesh/vertex/texture counts).
- Physics system (avian3d v0.4): `XrdsPhysicsBody` (Static/Dynamic/None), per-shape colliders (sphere, cuboid, cylinder, half-space, mesh), gravity scale, mass, SweptCcd tunneling prevention.
- Grab/throw system: XR controller raycast pick-up, kinematic hold, velocity-based throw (clamped 25 m/s).
- Interaction zones: sensor-based AABB triggers, `XrZoneEnterEvent` / `XrZoneExitEvent`, Sphere and Box shapes.
- Runtime physics API: `set_gravity_scale_for_node`, `set_mass_for_node` — live updates without scene reimport.
- Physics properties serialized in `XrdsSceneDocument` (physics_body, gravity_scale, mass on all primitive payload types).
- **In-world UI / panel-template system** (`docs/done/xrds-widget-template-plan.md`):
  unified templates authorable once and instanced either as scene-placed
  world panels or head-locked HUDs, with five widget kinds (Label, Button,
  Image, Slider, Toggle), per-instance trigger bindings (so N instances of
  one template drive N different targets), a dedicated Panels workspace
  (library / elements / canvas / inspector), drag-to-move and per-widget
  property forms on the canvas, and authorable size/colour/opacity. Backed by
  a real pointer surface (`XrdsWorldSurface`) and a grab handle
  (`XrGrabHandle`/`XrGrabHandleOnly`) so a panel's face stays clickable
  while the panel itself stays movable — the Meta Quest model. This closes
  the "In-world UI" gap listed below in earlier revisions of this document.
- Retired scene glTF/GLB export (`xrds-gltf` crate deleted outright — glTF has
  no vocabulary for panels, triggers, Tracks, anchors, or zones, so it wrote a
  file that looked complete and was a mesh dump); glTF *import* is unaffected,
  it runs through Bevy's own loader. Also deleted the dead `xrds-editor-egui`
  app (superseded by the Tauri editor, already unbuildable) and the anchor-link
  panel-attachment path (superseded by parenting a `Panel` node under a
  `PlayerAnchor`).
- `xrds-net`: WebRTC ICE-config bugs fixed, test suite restructured for
  reliability, and a full internal release-readiness pass completed —
  including a real two-machine WebRTC handshake, not just loopback
  (`docs/done/xrds-net-release-readiness.md`). Transport layer is
  internal-milestone-ready; a game-level multiplayer sync feature on top of it
  is not yet built (see Gap Analysis).

## Missing Parts / Remaining Work

Text3D rendering and the material texture-slot UI (previously tracked here
as items #1 and #2) are both done — see Completed Highlights and the
corrected "What still keeps the editor from being fully polished" section
above.

### 1) Feature breadth for a fuller primitive palette

- ~~`XrdsCapsule` for character/physics workflows~~ — **done.**
- `XrdsEffect` (particle effects) — **done**, see `docs/done/vfx-particle-effects-plan.md`.
- No other primitive gaps currently flagged as blocking

### 2) `Video` asset kind

- Deferred until a concrete media/scene workflow requires it
- Pattern is established (follow `Audio` / `EnvironmentMap`)

### 3) Remaining naming polish

- `TransformParams::rotation_quat_xyzw` and `rotation_euler_xyz_deg` dual-field clarified but not resolved structurally
- `*Patch` types (`NamePatch`, `ParentPatch`, etc.) are still ECS-jargon; hidden behind typed helpers

### 4) Panel pointer capture is per-panel, not per-element — policy decided, editor enforcement not yet built

- A panel with one small interactive element still captures the pointer
  across its whole rectangle (matches how visionOS/Quest system panels
  behave — a window is a window). Settled as policy, not a bug:
  `XrdsWorldSurface::enabled` is already `true` only when the template has an
  interactive element, so an info-only panel never captures at all.
- **Consequence accepted, not yet enforced in the editor:** a template with
  an interactive element head-locked to an anchor will capture the pointer
  wherever it sits in the wearer's view, permanently — unlike a world panel,
  which only captures when approached. Decided policy is that a HUD (a
  head-locked `Panel` node) should not be linkable to a template that has any
  interactive element at all; info-only templates (Label/Image) only.
- **Not yet built:** the template picker (wherever a `Panel` node's
  `SetPanelInstanceTemplate` target is chosen) should grey out — and refuse —
  any template with an interactive element when the node is head-locked
  (parented under a `PlayerAnchor`). No diagnostic or enforcement exists for
  this today; an author can currently head-lock an interactive template with
  no warning.

### 5) Editor items inherited from the archived EDITOR_TODO list

`docs/done/EDITOR_TODO.md` was archived on 2026-08-12 after its statuses were
re-verified against the code. These are the ones that were still genuinely open, moved
here so they stay visible:

- **Passthrough / blend-mode toggle — editor half only.** `XrdsXrBlendMode`
  (`Opaque`/`AlphaBlend`) already exists at `scene/node.rs:20` and the runtime handles
  `EnvironmentBlendMode` plus the `fb_passthrough` extension. Only the editor toggle is
  missing, which makes this a small, well-scoped job rather than a feature.
- **Spatial audio parameters — corrected 2026-08-19: this is *not* an editor half.**
  The earlier claim here was that the SDK supported these fields and only inspector UI
  was missing. That is wrong. `XrdsSceneAudioClip` does carry `distance_model`,
  `min_distance`, `max_distance`, `rolloff_factor` and `hrtf`
  (`xrds-scene-graph/src/scene/payload.rs`), but the runtime reads **only `spatial:
  bool`** — `xrds-runtime/src/xrds_api/spawn.rs:926` passes that one flag to Bevy and
  nothing anywhere touches the other five. They are serialized and inert.
  `xrds-audio`, the crate that would have honoured them, was deprecated and excluded
  from the workspace on 2026-08-18 (see the `exclude` note in the root `Cargo.toml`).
  So the open question is not "wire up the inspector" but **decide whether these
  fields can be honoured on Bevy's audio at all, and delete them if not** — shipping
  authorable-but-inert fields is the exact failure mode `player-body-collider-plan.md`
  was written about.
- **Keyframe curve editor.** The Track/Sequencer system covers timed *action*
  sequencing; interpolating a property along a curve is still unbuilt. Note the original
  item's "export as a glTF animation track" is void — glTF export is retired.
- **Spatial anchor node.** Cross-session persisted anchors; needs a new payload plus an
  OpenXR persist-anchor API. Not started.
- **Performance budget panel.** Live triangle/draw-call/texture-memory counters in the
  editor. Editor-only instrumentation; Bevy diagnostics already expose the data.

LOD groups were also on that list and are already tracked in Gap Analysis below.

Separately tracked:

- ~~**A player cannot enter an `InteractionZone`**~~ — **fixed and device-verified on
  Quest 3, 2026-08-18.** This document previously listed it as "deliberately unfixed";
  that was stale. The player now gets a capsule physics body and an `XrdsId` (both
  were required — a collider alone would have compiled and still emitted no events).
  Plan archived to `docs/done/player-body-collider-plan.md`.
- **Android window-lifecycle crash** — intermittent, trigger **not reproduced** under
  controlled conditions; two staged attempts failed to trigger it. Not schedulable as
  planned work until there is a repro. `docs/android-window-lifecycle-plan.md`.

## Road to 1.0 — Small / Medium / Large Blueprint

Recorded 2026-08-19. This is the sizing of everything between here and the 1.0
bar defined above. Sizes are in *phases*, where a phase is a coherent landable
change with its own tests — not in calendar time.

There is no **High**-priority engine gap left open; particle systems/VFX was the
last one and shipped 2026-08-12.

### Small — 1 phase each

Planned in detail in `docs/small-phases-plan.md`.

| # | Item | Crate | Note |
| --- | --- | --- | --- |
| S1 | ~~Spatial-audio params: honour or delete~~ | `xrds-components` / `xrds-scene-graph` / `xrds-runtime` | **Done 2026-08-19, verified on desktop and Quest 3.** Four falloff fields honoured, `hrtf` removed. Also fixed a bug it uncovered: `SpatialListener` was attached only on the `XrdsAPI` camera path, so **spatial audio had no listener in XR at all** — every XRDS app on a headset, not just this feature. Backends were evaluated and rejected; stay on `bevy_audio` (`docs/spatial-audio-backend-spike.md`). |
| S2 | Zone-event `debug!` logging | `xrds-runtime` | Makes the next device pass self-verifying instead of dependent on someone describing what they saw. |
| S3 | Head-locked interactive-template diagnostic | `xrds-scene-graph` | SDK half of §4. Editor grey-out is separate and follows this. |
| S4 | ~~Passthrough blend-mode toggle~~ | `xrds-openxr` / `xrds-runtime` / editor | **Done 2026-08-19, device-verified on Quest 3** — authored from the editor, cube floating in the real room. Was recorded as "editor only"; in fact `XR_FB_passthrough` was never requested and the authored field reached nothing. Also **not** `EnvironmentBlendMode::ALPHA_BLEND` — that is a global frame-blend parameter and using it bleeds reality through every non-opaque surface. Passthrough is a composition layer beneath the projection; recipe in `docs/small-phases-plan.md` S4. |
| S5 | Naming polish (§3) | `xrds-scene-graph` | `TransformParams` dual rotation field; `*Patch` ECS jargon. Mechanical but source-breaking. |

### Medium — 2–3 phases each

| Item | Phases | Where the cost actually is |
| --- | --- | --- |
| **LOD system** | 3: payload + document → runtime selection → authoring/validation | Greenfield — no LOD vocabulary exists in `xrds-scene-graph` (verified). Authored levels only; automatic LOD *generation* is out of scope. Best payoff at scene scale on Quest. |
| **Post-processing depth** (DOF, SSAO, colour grading) | 2–3, roughly one per effect | `XrdsBloom`/`XrdsTonemapping` establish the pattern and Bevy supplies the effects, so this is mostly authorable-surface plumbing. **Verify each on Adreno before committing** — same class of trap that killed `bevy_hanabi`. |
| **Video asset kind** (§2) | 3: asset kind + catalog/validation → runtime playback → round-trip tests | The asset-kind half follows `Audio`/`EnvironmentMap` exactly. The *playback* half has no precedent in the workspace — no video decode path exists. That asymmetry is why it keeps being deferred. |
| **Keyframe curve editor** (SDK half) | 2: curve/interpolation model → runtime property driver | Distinct from Track/Sequencer, which sequences *actions*, not property curves. The editor UI on top is its own larger job. The original item's "export as a glTF animation track" is void — glTF export is retired. |
| **Audio authoring in the editor** (S6) | 2–3: inspector section + bridge commands → radius gizmos → curve preview + audition | Spatial audio now works on device and is reachable **only from Rust**: `Inspector.tsx` has no `AudioClip` section and the bridge has no audio commands, so an authored clip's volume, loop, spatial flag and entire falloff curve are all unauthorable. Not "add rows to the audio panel" — there is no audio panel. Detailed scope, including why the curve preview must be drawn in dB, in `docs/small-phases-plan.md` S6. |

### Large — 4+ phases, each gated on a design decision

None of these should be started as implementation work. Each needs its blocking
question answered first, and for three of the four the design is the larger half.

| Item | Phases | Blocking question |
| --- | --- | --- |
| **Animation state machine** | ~4: state/transition model → blend trees → runtime evaluation → authoring | Largest pure-engine item. Playback and morph sliders exist; blend trees, transition graphs and IK are all unbuilt. Scope question: is IK in 1.0 or not? |
| **Spatial anchor node** | ~4: OpenXR persist-anchor binding → payload → runtime resolve/rebind → cross-session tests | `xrds-openxr` has no anchor persistence today. Vendor-extension territory — Meta's persisted-anchor path differs from Android XR's, so "which platforms does 1.0 promise this on?" has to be answered first. Cross-session testing is manual by nature. |
| **NavMesh / pathfinding** | ~4: bake/import → agent component → runtime steering → authoring | Greenfield. Bake in-editor vs. import a baked mesh are different projects; pick one before scoping. |
| **Networking / multiplayer** | Not sizable yet | Transport (`xrds-net`) is hardened and internal-milestone-ready, two-machine WebRTC handshake included. Missing piece is the *game-level* `XrdsAction` that syncs trigger effects, and **the authority model must be decided first** — see the Networking entry in `docs/xrds-trigger-action-backlog.md`. Do not estimate this before that decision. |

### Deliberately not in the blueprint

- **Documentation pass.** Previously the standing #1 recommendation here. Held
  back on purpose: the GUI manual documents an editor whose Tailwind/Radix
  migration is mid-flight and which is about to grow new inspector sections
  (S1, S4), so writing it now buys a rewrite. The *API reference* half has no
  such problem and is better written as rustdoc on the items themselves, where
  a signature change and its docs move in the same diff. Revisit once the Small
  tier lands.
- **Android window-lifecycle crash** — unreproduced; see §5.
- **Particle blend modes** — a no-op upstream in `bevy_firework`, not our code.
- **Terrain** — Low priority and not an XR-shaped need; judged against workflow,
  not against Unity parity, per the 1.0 definition.

### Honest total

**The Small tier is complete** (S1–S4 done and device-verified where a device
applies; S5's rotation half done and its rename half dropped on review — see
`docs/small-phases-plan.md`). That leaves **five** Medium and four Large.

What the tier actually cost, and why the sizing was not the useful part: it was
scoped as polish and produced nine defects, none of which fails a test or looks
wrong in review. Among them, spatial audio had **no listener at all in XR** —
broken for every XRDS app on a headset, not merely for the feature being added —
and three shipped examples set rotations that did nothing. Every one was found by
running the thing and looking at it.

Three items from the archived `EDITOR_TODO.md` were recorded as "editor half only"
and all three were false. Treat the rest of that list as unverified until grepped. The Small tier is days. The Medium tier is
where most of the value-per-phase sits and compounds with what already exists.
The Large tier is genuinely large, and three of its four items are blocked on a
decision rather than on effort — which means the next real milestone after the
Small tier is a design session, not a sprint.

## Gap Analysis vs. Mature 3D Engines

Features present in engines like Unity, Godot, or Unreal that are not yet in DeviceSDK:

| Feature | Priority | Notes |
| --- | --- | --- |
| ~~**Particle systems / VFX**~~ | **Done** | `XrdsEffect` with `Burst`/`Trail`, editor authoring, and Track-driven `PlayEffect`/`StopEffect`; device-verified on Quest 3. Backend is `bevy_firework` — `bevy_hanabi` was adopted and rejected after its GPU-compute path rendered nothing on Adreno. See `docs/done/vfx-particle-effects-plan.md`. Remaining: blend modes are a no-op upstream, and there is no curve editor. |
| **Animation state machine** | Medium | Playback + morph sliders exist; no blend trees, transition graphs, or IK |
| **Post-processing stack** | Medium | Bloom + tonemapping exist (`XrdsBloom`, `XrdsTonemapping`) and exposure is fully authorable; no DOF, SSAO, or color grading |
| **NavMesh / pathfinding** | Medium | AI agent navigation; needed for NPC-driven XR experiences |
| **LOD system** | Medium | Performance at scene scale; no automatic LOD generation or selection |
| **Video playback** | Low | Deferred; pattern established via `Audio` / `EnvironmentMap` |
| ~~**Capsule primitive**~~ | **Done** | `XrdsCapsule` shipped; also now the shape of the player's physics body. |
| **Networking / multiplayer** | Low | The transport layer (`xrds-net`) is itself hardened and internal-milestone-ready, including a real two-machine WebRTC handshake — see `docs/done/xrds-net-release-readiness.md`. What is still missing is the *game-level* feature: an `XrdsAction` that syncs trigger effects across clients, which needs an authority model decided first (see `docs/xrds-trigger-action-backlog.md`'s Networking entry). Large scope, deferred. |
| **Terrain system** | Low | Heightmaps, large world; not a near-term XR target |

Already covered that engines also provide: scene graph, PBR materials, lights,
cameras, spatial audio, animation playback, physics (rigid body, colliders,
raycasting, grab/throw), GLTF pipeline, environment (IBL/fog/skybox), export,
interaction zones, editor, **in-world UI** (world-space panels with buttons,
sliders, toggles, labels, images, and per-instance trigger bindings — see
Completed Highlights).
