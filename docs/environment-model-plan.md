# Environment Model Plan

This document tracks the scene-environment basement and the remaining expansion space after IBL, skybox, exposure, and fog.

## Progress Snapshot

- Implemented and validated: scene-level IBL, skybox, manual exposure, and linear fog
- Supported control paths: document-driven authoring and runtime-driven policy
- Verified with focused examples:
	- `runtime_scene_environment.rs` for the full runtime policy surface
	- `scene_document_environment_import.rs` for authored import behavior
	- `environment_map_visual_check.rs` for a quick visual check that environment maps are landing on the runtime material path

## Current Basement

Today XRDS supports scene-level IBL, skybox, manual exposure, and linear fog policy with two control paths:

- document-driven authoring through `XrdsSceneDocument`
- runtime-driven policy through `XrdsAPI`

That slice is intentionally narrow. It proves the asset model, persistence model, import/export behavior, and runtime projection path before XRDS grows a larger environment surface.

## Next Model Targets

No additional next model is committed yet. The current basement is complete enough to validate the scene-wide policy shape before XRDS takes on local volumes, blending, or more engine-shaped environment controls.

## Implemented: Skybox

Goal: authored or runtime-driven background environment, separate from reflection/lighting intensity.

Delivered shape:

- optional skybox block under `XrdsSceneEnvironment`
- one durable scene asset id for the skybox texture
- runtime projection as a managed scene-wide skybox policy
- explicit expert-layer camera skyboxes preserved rather than overwritten

Important rule:

- do not force skybox and IBL to be the same asset
- allow them to be authored independently, even if many apps choose to reuse one source

## Implemented: Exposure

Goal: scene-wide authored exposure policy that survives save/load and can also be driven by runtime logic.

Delivered shape:

- add an optional exposure block under `XrdsSceneEnvironment`
- manual exposure only through `ev100`
- runtime projection through Bevy camera `Exposure`
- explicit expert-layer camera exposure preserved rather than overwritten

Important rule:

- keep exposure scene-wide first
- do not jump directly to per-camera authored exposure unless a real use case forces it

## Implemented: Fog

Goal: durable fog policy that can be projected into runtime without requiring direct engine knowledge.

Delivered shape:

- add an optional fog block under `XrdsSceneEnvironment`
- start with one clear fog model instead of multiple variants
- use a simple linear distance fog shape with `color`, `start`, and `end`
- runtime projection through Bevy `DistanceFog`
- explicit expert-layer camera fog preserved rather than overwritten

Important rule:

- avoid a grab-bag of engine-native fog knobs
- define XRDS fog in terms the document model can own stably

## Current Verification Surface

- `runtime_scene_environment.rs` proves live runtime control over IBL, skybox, exposure, and fog
- `scene_document_environment_import.rs` proves authored scene environment metadata survives import
- `environment_map_visual_check.rs` gives a fast visual check for environment-map lighting and skybox readability across different material roughness levels

## Cross-Cutting Constraints

- Scene-wide first: environment remains a scene policy before XRDS considers local volumes or blends.
- Durable references first: asset-backed environment features should continue to reference scene asset ids.
- Import/export fidelity: any new environment block should survive JSON save/load and runtime export.
- Override clarity: XRDS-managed policy should never silently overwrite explicit expert-layer state.
- Keep the basement honest: if a field cannot round-trip or cannot be applied coherently at runtime, it should not be added yet.

## Not In The Next Slice

The following remain intentionally out of scope until a real workflow forces them:

- local reflection volumes
- layered or blended environment stacks
- automatic reverse-inference of authored environment from arbitrary Bevy state
- per-camera authored environment overrides as a default document feature

Those can come later if real editor or runtime workflows prove the need.
