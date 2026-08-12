# DeviceSDK Overall Progress

Last updated: 2026-08-11

## Project Goal

Provide a non-expert-first SDK to build XR applications, with:

- a simple default application surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`)
- a durable scene document model (`xrds-scene-graph`)
- an expert escape hatch for direct engine-level control when needed

## Overall Completion (Estimated)

Estimated overall progress toward a strong SDK basement for XR applications: **95%**.

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

- `XrdsCapsule` for character/physics workflows — capsule collider shape useful for character controllers
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

## Suggested Next Steps (Short Horizon)

Both items previously listed here (Text3D rendering, texture-slot UI) are
done. Next up:

1. **Documentation pass.** More overdue now than when this was first
   suggested: the in-world UI/panel-template system, the physics/grab system,
   and `xrds-net`'s hardening are all substantial, all shipped, and none are
   covered by user-facing docs yet — only internal `docs/done/*` design
   records.
2. **`XrdsCapsule` primitive** — small, well-scoped, closes the one concrete
   primitive-palette gap.
3. **Pick one "High priority" item from the Gap Analysis table below.**
   Particle systems/VFX is the most-requested-shape gap for interactive XR
   apps and has no groundwork started, unlike animation/post-processing which
   have partial coverage already (see the table).

## Gap Analysis vs. Mature 3D Engines

Features present in engines like Unity, Godot, or Unreal that are not yet in DeviceSDK:

| Feature | Priority | Notes |
| --- | --- | --- |
| **Particle systems / VFX** | High | No emitter, trail, or burst effects; needed for almost all interactive XR apps. No groundwork started — highest-value gap with nothing to build on yet. |
| **Animation state machine** | Medium | Playback + morph sliders exist; no blend trees, transition graphs, or IK |
| **Post-processing stack** | Medium | Bloom + tonemapping exist (`XrdsBloom`, `XrdsTonemapping`) and exposure is fully authorable; no DOF, SSAO, or color grading |
| **NavMesh / pathfinding** | Medium | AI agent navigation; needed for NPC-driven XR experiences |
| **LOD system** | Medium | Performance at scene scale; no automatic LOD generation or selection |
| **Video playback** | Low | Deferred; pattern established via `Audio` / `EnvironmentMap` |
| **Capsule primitive** | Low | Useful character/physics shape; `XrdsCapsule` not yet added |
| **Networking / multiplayer** | Low | The transport layer (`xrds-net`) is itself hardened and internal-milestone-ready, including a real two-machine WebRTC handshake — see `docs/done/xrds-net-release-readiness.md`. What is still missing is the *game-level* feature: an `XrdsAction` that syncs trigger effects across clients, which needs an authority model decided first (see `docs/xrds-trigger-action-backlog.md`'s Networking entry). Large scope, deferred. |
| **Terrain system** | Low | Heightmaps, large world; not a near-term XR target |

Already covered that engines also provide: scene graph, PBR materials, lights,
cameras, spatial audio, animation playback, physics (rigid body, colliders,
raycasting, grab/throw), GLTF pipeline, environment (IBL/fog/skybox), export,
interaction zones, editor, **in-world UI** (world-space panels with buttons,
sliders, toggles, labels, images, and per-instance trigger bindings — see
Completed Highlights).
