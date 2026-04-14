# DeviceSDK Architecture

This repository is organized around a layered SDK model so non-experts can build XR apps without dropping into engine internals, while advanced users still have an escape hatch.

## SDK Layering (Most Important)

### 1) Default SDK Layer (app-facing)

Primary types:

- `XrdsApp`
- `XrdsAPI`
- `XrdsUpdateContext`

Responsibilities:

- spawn/update scene content through XRDS descriptors and handles
- drive scene-wide runtime policy (for example environment policy)
- keep normal app code independent from direct Bevy ECS/system wiring

Design intent:

- this is the main supported path for most users
- new features should be surfaced here first when possible

### 2) Expert Layer (engine-facing)

Primary types:

- `RuntimeHandler`
- direct Bevy systems/components/resources

Responsibilities:

- advanced engine control and custom integration points
- low-level rendering/runtime behavior when XRDS abstractions are not enough

Design intent:

- optional escape hatch, not the default development model
- `xrds` does not re-export Bevy; expert code imports Bevy directly

### 3) Document/Authoring Layer (durable scene model)

Primary crate and types:

- `xrds-scene-graph`
- `XrdsSceneDocument`, `XrdsSceneNode`, asset catalog/document editing APIs

Responsibilities:

- save/load JSON scene documents
- durable ids, hierarchy, metadata, and validation
- import/export boundary between authored scene meaning and runtime state

How it relates to SDK layering:

- authoring data is edited in document APIs
- runtime behavior is realized through `XrdsAPI` in `xrds-runtime`

## Crate Roles (Brief)

- `xrds` (root crate): SDK entry surface and workspace integration layer.
- `xrds-runtime`: runtime projection layer that realizes XRDS concepts in the live engine.
- `xrds-scene-graph`: document model, persistence, and authored workflow operations.
- `xrds-components`: shared XRDS component descriptors/types used by runtime and SDK surfaces.
- `xrds-openxr`: OpenXR backend integration.
- `xrds-net`: networking-related runtime integrations/samples.
- `xrds-audio`: audio-related runtime integrations.
- `xrds-internal`: internal plumbing and lower-level implementation support.

## Data Flow

Two common paths are intentionally supported:

1. Runtime-first: app logic calls `XrdsAPI` directly for live scene control.
2. Document-first: author in `XrdsSceneDocument`, then import through runtime APIs.

Both paths converge in `xrds-runtime`, which applies XRDS-authored policy/components to the live world.

## Current Environment Policy Example

Scene environment policy currently supports IBL, skybox, manual exposure, and linear fog.

- Document-driven: author in `XrdsSceneDocument`, import into runtime.
- Runtime-driven: call `merge_scene_assets(...)`, `set_scene_environment(...)`, and `clear_scene_environment(...)`.
