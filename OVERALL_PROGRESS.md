# DeviceSDK Overall Progress

Last updated: 2026-06-30

## Project Goal

Provide a non-expert-first SDK to build XR applications, with:

- a simple default application surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`)
- a durable scene document model (`xrds-scene-graph`)
- an expert escape hatch for direct engine-level control when needed

## Overall Completion (Estimated)

Estimated overall progress toward a strong SDK basement for XR applications: **95%**.

## General 3D Editor Progress

Estimated progress toward a general 3D content editor: **88%**.

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

- `Text3D` renders via `Text2d` overlay — requires `Camera2d`; does not render as a true billboard mesh in exported apps
- Advanced material GUI (texture slot assignment UI in inspector not yet exposed — API exists, panel not wired)
- Editor visual polish pass not started

## Progress Breakdown

| Area | Status | Percent |
| --- | --- | --- |
| Core SDK surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`) | Stable; 70+ typed methods; material texture slots added | **92%** |
| Scene document model (`xrds-scene-graph`) | All asset kinds, 11 payload types, hierarchy, materials, audio, round-trips | **95%** |
| Runtime projection (`xrds-runtime`) | All built-in types, audio playback, environment, export | **90%** |
| Scene environment policy | IBL + skybox + exposure + linear fog; document-driven and runtime-driven | **97%** |
| Physics | avian3d v0.4; Static/Dynamic/None bodies; per-primitive colliders; grab/throw; raycasting; interaction zone sensors; scene-doc serialized | **90%** |
| Asset workflow | Gltf, Texture, EnvironmentMap, Audio — catalog, validation, diagnostics, runtime | **92%** |
| GUI editor | Functional editor with all core panels; text3d and texture-slot UI gaps remain | **88%** |
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

## Missing Parts / Remaining Work

### 1) Text3D rendering in exported apps

- `XrdsText` currently spawns a `Text2d` component, which requires `Camera2d` to render
- In exported apps (pure 3D scene, no `Camera2d`), text nodes are invisible
- Needs a true 3D text solution (billboard mesh or `Text3d` when Bevy supports it)
- In-editor: egui overlay renders labels correctly as a workaround

### 2) Material texture slot assignment UI in inspector

- `material_textures()` / `set_material_texture_slot()` exist in `XrdsAPI`
- Inspector panel has no UI to browse and assign textures to slots (drag-from-palette or file picker)
- Blocker for fully visual material authoring

### 3) Feature breadth for a fuller primitive palette

- `XrdsCapsule` for character/physics workflows — capsule collider shape useful for character controllers
- No other primitive gaps currently flagged as blocking

### 4) `Video` asset kind

- Deferred until a concrete media/scene workflow requires it
- Pattern is established (follow `Audio` / `EnvironmentMap`)

### 5) Remaining naming polish

- `TransformParams::rotation_quat_xyzw` and `rotation_euler_xyz_deg` dual-field clarified but not resolved structurally
- `*Patch` types (`NamePatch`, `ParentPatch`, etc.) are still ECS-jargon; hidden behind typed helpers

## Suggested Next Steps (Short Horizon)

1. Fix `Text3D` runtime rendering — replace `Text2d` with a billboard mesh or deferred 3D text approach so exported apps show text nodes.
2. Wire material texture slot UI in the inspector panel — browse catalog textures and assign to BaseColor/Normal/MetallicRoughness/Occlusion/Emissive slots.
3. Begin documentation pass now that the feature set has stabilized through Phase 4 QA.

## Gap Analysis vs. Mature 3D Engines

Features present in engines like Unity, Godot, or Unreal that are not yet in DeviceSDK:

| Feature | Priority | Notes |
| --- | --- | --- |
| **Particle systems / VFX** | High | No emitter, trail, or burst effects; needed for almost all interactive XR apps |
| **In-world UI** (3D panels, buttons) | High | World-space canvas/widget surfaces beyond Text3D; critical for XR menus |
| **Animation state machine** | Medium | Playback + morph sliders exist; no blend trees, transition graphs, or IK |
| **Post-processing stack** | Medium | Exposure present; no bloom, DOF, SSAO, color grading |
| **NavMesh / pathfinding** | Medium | AI agent navigation; needed for NPC-driven XR experiences |
| **LOD system** | Medium | Performance at scene scale; no automatic LOD generation or selection |
| **Video playback** | Low | Deferred; pattern established via `Audio` / `EnvironmentMap` |
| **Capsule primitive** | Low | Useful character/physics shape; `XrdsCapsule` not yet added |
| **Networking / multiplayer** | Low | Social XR use cases; large scope, deferred |
| **Terrain system** | Low | Heightmaps, large world; not a near-term XR target |

Already covered that engines also provide: scene graph, PBR materials, lights, cameras, spatial audio, animation playback, physics (rigid body, colliders, raycasting, grab/throw), GLTF pipeline, environment (IBL/fog/skybox), export, interaction zones, editor.
