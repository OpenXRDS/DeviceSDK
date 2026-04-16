# DeviceSDK Overall Progress

Last updated: 2026-04-16

## Project Goal

Provide a non-expert-first SDK to build XR applications, with:

- a simple default application surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`)
- a durable scene document model (`xrds-scene-graph`)
- an expert escape hatch for direct engine-level control when needed

## Overall Completion (Estimated)

Estimated overall progress toward a strong SDK basement for XR applications: **87%**.

## General 3D Editor Backend Progress (Excluding GUI)

Estimated progress toward a general 3D content editor backend, excluding GUI: **85%**.

What is already strong enough for an editor backend:

- scene graph and hierarchy foundation
- stable identity and document persistence
- runtime import/export bridge
- baseline transform, visibility, material, and light editing
- asset catalog with three first-class kinds: `Gltf`, `Texture`, `EnvironmentMap`, `Audio`
- scene environment policy (IBL, skybox, exposure, fog) — document-driven and runtime-driven
- inspector read/write API for camera, all four light types, glTF, and all mesh primitives
- audio clip scene nodes with runtime playback, spatial audio, and load-failure error handling
- naming aligned to application-level conventions (not engine-shaped)
- integration tests covering import, live edit, export, and environment map round-trips

What still keeps this from being editor-backend complete:

- no GUI editor yet — basement is ready, GUI layer has not started
- advanced material authoring: texture slot inspector helpers missing from runtime API
- `XrdsCapsule` and broader primitive palette for character/physics workflows
- `Video` asset kind if media playback workflows appear

## Progress Breakdown

| Area | Status | Percent |
| --- | --- | --- |
| Core SDK surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`) | Stable baseline, inspector reads complete, naming clean | **90%** |
| Scene document model (`xrds-scene-graph`) | All asset kinds, hierarchy, materials, audio, validation, round-trips | **92%** |
| Runtime projection (`xrds-runtime`) | All built-in types, audio playback, environment, export | **88%** |
| Scene environment policy | IBL + skybox + exposure + linear fog; EnvironmentMap kind separated | **95%** |
| Asset workflow | Gltf, Texture, EnvironmentMap, Audio — catalog, validation, diagnostics, runtime | **90%** |
| Editor-basement readiness | Minimum prototype milestone fully met; remaining gaps are GUI and advanced material | **85%** |
| Docs/examples/test coverage | Examples for all major flows; integration tests for import/edit/export | **84%** |

## Completed Highlights

- Non-expert-first SDK layering established and documented.
- Runtime-first and document-first flows both work and are tested.
- Scene document round-trip and runtime import/export operational.
- Scene environment policy end-to-end: document authoring, runtime policy, validated projection.
- `XrdsSceneAssetKind` now has four distinct kinds: `Gltf`, `Texture`, `EnvironmentMap`, `Audio`.
  - `EnvironmentMap`: HDR/EXR/KTX2/DDS only; IBL and skybox validation enforce this kind; environment-referenced assets excluded from unused diagnostics.
  - `Audio`: MP3/OGG/WAV/FLAC; `XrdsSceneAudioClip` scene node; runtime playback with `AudioPlayer`; pre-validation system prevents Bevy panics on unrecognised formats.
- Inspector read API complete for camera (projection, look-at), all four light types (full params), and glTF source.
- Naming aligned: `roughness` (was `perceptual_roughness`), `affects_baked_lighting` (was `affects_lightmapped_meshes`), `UvParams`/`SamplerParams` (was `UvMetadata`/`SamplerMetadata`).
- `entity_of_id` documented as expert escape hatch; `TransformParams` dual-rotation clarified.
- Integration tests for: environment map round-trip, live material edit → export, live rename → export.
- Every XRDS camera automatically becomes the spatial audio listener (`SpatialListener`).

## Missing Parts / Remaining Work

### 1) Advanced material authoring via runtime inspector

- texture slot inspector helpers missing from `XrdsAPI` / `XrdsUpdateContext`:
  - `material_textures()` read method (parallel to `material_params()`)
  - `set_material_texture_slot()` write method
- without these, an editor inspector can't populate/edit individual texture slots through the runtime API without dropping to `set_material_params` for the whole struct

### 2) Feature breadth for a fuller primitive palette

- `XrdsCapsule` for character/physics workflows (evaluate if target apps need it)
- no other primitive gaps currently flagged as blocking

### 3) `Video` asset kind

- deferred until a concrete media/scene workflow requires it
- pattern is established (follow `Audio` / `EnvironmentMap`)

### 4) GUI editor

- the basement is ready — minimum milestone fully met
- GUI layer (inspector panels, hierarchy tree, asset browser) has not been started
- this is the highest-value remaining investment once the basement is declared stable

### 5) Remaining naming polish

- `TransformParams::rotation_quat_xyzw` and `rotation_euler_xyz_deg` dual-field clarified but not resolved structurally — still a potential confusion point for new contributors
- `*Patch` types (`NamePatch`, `ParentPatch`, etc.) are still ECS-jargon; low priority since they are hidden behind typed helpers

## Suggested Next Steps (Short Horizon)

1. Add texture slot inspector read/write helpers to `XrdsAPI` and `XrdsUpdateContext` — closes the last material authoring gap before the GUI editor is built.
2. Decide whether to start the GUI editor basement now or harden the SDK surface further first.
3. If hardening: one more naming pass focused on `TransformParams` dual-rotation and any remaining method names that read like engine internals.
