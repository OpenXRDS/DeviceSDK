# Editor Readiness Checklist

This checklist is for evaluating whether the current XRDS surface is sufficient to serve as the basement for a future GUI editor built on top of `XrdsAPI`.

## Goal

The editor should be able to use XRDS concepts as its primary runtime and scene-model interface.

- Non-experts should be able to build and edit scenes without needing Bevy knowledge.
- The editor application should own the document, hierarchy, selection, and inspector state.
- `XrdsAPI` should act as the live runtime projection of that document.
- Direct Bevy access should remain an escape hatch for advanced editor subsystems only.

## Current status

Current XRDS coverage is enough to support a first editor basement, but not enough yet for a broadly usable editor product.

Already present:

- Stable object identity via `XrdsId`
- Registry mapping via `XrdsRegistry`
- Canonical XRDS hierarchy index with parent/child id queries, with Bevy hierarchy kept as a derived runtime projection
- Common transform data via `TransformParams`
- SDK-level color type via `XrdsColor`
- Scene nodes for camera, glTF asset, node, cube, cylinder, sphere, plane, tetrahedron, point light, directional light, spot light, and ambient light
- XRDS-level spawn path through `XrdsAPI::spawn(&descriptor)` without exposing Bevy construction APIs
- Runtime editing support for transform, visibility, rename, reparent, duplicate, delete, material base color/emissive, light parameters, and built-in primitive geometry
- XRDS-derived hierarchy queries via parent/child id accessors on the runtime side
- Public scene-document import/export for the built-in XRDS scene set, preserving stable ids, parent links, built-in materials, and glTF references between `xrds-scene-graph` and live runtime state

Current gaps:

- Built-in primitive coverage is better, but still narrow for a full editor-facing palette
- Editor metadata storage and document/session workflow now exist for tags, layers, lock state, hidden-in-editor state, source metadata, and custom properties
- Asset reference authoring now has document-side conventions, fallback behavior, and diagnostics, but broader asset-type coverage can still grow later
- Runtime editing and persistence are now viable for a first editor basement; the largest remaining gaps are broader updater coverage, advanced material policy, and simple-path UX guidance

## Priority checklist

### 1. Scene graph foundation

- [X] Add a public hierarchy-only component such as `XrdsNode` or `XrdsEmpty`
- [X] Make parent/child behavior consistent across all built-in components
- [X] Define whether children are authored explicitly, derived at runtime, or both
- [X] Ensure reparenting can be expressed entirely through XRDS-level APIs

Scene graph contract:

- `parent_id` is the authored hierarchy input on XRDS descriptors
- `children_ids` is derived by XRDS from authored parent links and kept in sync with the live runtime hierarchy
- `XrdsHierarchyIndex` is the XRDS hierarchy source of truth; Bevy `ChildOf` is derived runtime state only
- Reparenting is expressed as an XRDS-level parent patch rather than direct Bevy hierarchy mutation from app code

### 2. Core scene components

- [ ] Keep camera, light, glTF asset, and basic mesh components stable and inspector-friendly
- [X] Add at least `XrdsSphere` and `XrdsPlane3D`
- [ ] Consider `XrdsCapsule` if character/editor workflows are expected
- [X] Define a minimal “empty/group” object story for folders, pivots, and scene organization

### 3. Material and rendering authoring

- [X] Introduce XRDS-native material descriptors instead of Bevy-facing material authoring in the editor-facing layer
- [X] Support simple non-expert material editing first: base color, opacity/alpha, unlit flag, emissive color/intensity
- [X] Decide how advanced PBR settings should be exposed without leaking Bevy types by default
- [X] Keep direct Bevy `StandardMaterial` access only as an expert escape hatch

Current material authoring policy:

- `XrdsSceneMaterial` remains a concrete, resolved XRDS material model rather than a sparse patch format
- `XrdsSceneMaterialPbrParams` stays part of that XRDS-owned material model with canonical default values
- metallic, perceptual roughness, reflectance, double-sided, alpha mode, and alpha cutoff are first-class XRDS material attributes
- those PBR attributes should be presented as advanced XRDS controls by default, not as the first material knobs shown to non-experts
- direct Bevy material mutation remains an expert escape hatch rather than the normal way to reach advanced material controls

### 4. Authoring model clarity

- [X] Decide whether `authoring.rs` is the canonical editor document model or only a temporary helper layer
- [X] Avoid maintaining two competing scene models long term: concrete XRDS components vs `EntityBlueprint` / `EntityKind`
- [X] Remove `authoring.rs` as a competing scene model if editor state will live in the app
- [X] Decide whether any shared app-facing authoring utilities should exist at all

#### Decision on `authoring.rs`

The editor application owns authoritative scene/document state. `XrdsAPI` is the runtime bridge that projects that state into the live engine world.

Because of that, `authoring.rs` should not exist as a second canonical scene model inside the XRDS component layer.

Decision:

- `authoring.rs` has been removed
- The editor app should own document state directly
- XRDS components are runtime-facing building blocks and schema pieces, not the full editor document model

Consequence:

- Shared app-facing authoring utilities should not exist for now; they would enlarge the SDK surface and blur the boundary between editor-owned document state and XRDS runtime projection
- If shared authoring helpers are needed later, they should be introduced only for a demonstrated cross-app need, not as a default SDK layer

### 5. Runtime editing support

- [X] Expand updater coverage for built-in components used by inspectors beyond the current base set
- [X] Support transform edits, visibility, color/material edits, and light parameter edits through XRDS APIs across the current built-in scene set
- [X] Support primitive geometry edits such as sphere radius and plane size through XRDS APIs across the current built-in primitive set
- [X] Define when edits should use queued updates versus immediate mutation APIs
- [X] Ensure editor actions such as duplicate, delete, rename, and reparent have XRDS-level support

Current built-in editing baseline now includes:

- direct descriptor spawning through `XrdsAPI::spawn(&descriptor)`
- transform helpers and typed transform patching
- parent/reparent patches and incremental derived child synchronization
- rename and visibility patches across the core built-in scene set
- camera look-at and projection patching
- glTF asset source and scene-index patching
- point, directional, spot, and ambient light parameter editing
- material base-color and emissive editing for mesh-based built-ins
- cube, cylinder, sphere, plane, and tetrahedron geometry patching
- inspector-friendly queued helper methods for the built-in light and primitive patch set
- explicit immediate-preview versus queued-commit semantics for editor interactions

### 6. Editor metadata

- [X] Add a place for editor-only metadata separate from runtime rendering data
- [X] Support labels/tags, layers, lock state, hidden-in-editor state, and user-defined metadata
- [X] Define how editor metadata is serialized with scene documents

Current metadata baseline now includes:

- per-node tags, layer, lock state, hidden-in-editor state, custom key/value properties, and source-link metadata
- direct JSON serialization on `XrdsSceneNode.editor` in the canonical scene document format
- document/session mutation APIs so a future GUI editor can edit metadata without reaching into raw struct internals

### 7. Asset workflow

- [X] Make glTF asset usage solid enough for editor-driven asset placement and inspector editing
- [X] Decide how asset references are represented in document data
- [X] Define fallback behavior for missing assets and invalid scene indices
- [ ] Plan for future non-glTF asset types if needed

Current asset workflow baseline now includes:

- a document-side asset catalog through `XrdsSceneAsset` and `XrdsSceneAssetKind`
- catalog-backed glTF references on scene nodes through `asset_id`, embedded fallback `asset_uri`, `scene_index`, and export policy
- document/session helpers for registering, ensuring, placing, retargeting, rebinding, renaming, and removing glTF assets
- explicit fallback behavior for missing catalog entries, detached nodes, missing files, parse failures, and invalid scene indices
- usage and health diagnostics for editor UI consumption, including unresolved references and unused assets
- runtime import/export that preserves catalog-backed glTF references and round-trips them through XRDS scene documents

### 8. Serialization and document model

- [X] Define the canonical save/load scene format for editor documents
- [X] Add built-in document/runtime import and export helpers that preserve ids and hierarchy
- [X] Ensure scene data can round-trip cleanly between serialized form and live XRDS runtime state
- [X] Preserve stable object identity across save/load
- [X] Support undo/redo-friendly data structures

### 9. UX for non-experts

- [X] Ensure common actions map to XRDS concepts, not Bevy concepts
- [X] Audit public examples to make sure the simple path is taught first
- [X] Keep direct Bevy examples separate and clearly labeled as expert mode
- [ ] Review naming to keep the default API application-level and non-engine-shaped

## Minimum milestone for a first editor prototype

The project is ready for a first editor prototype when the following are true:

- [X] There is a public hierarchy-only node component
- [X] There are enough built-in scene nodes for typical layout work: camera, lights, glTF, cube, sphere, plane
- [X] Inspector edits for transform, visibility, base color, and light parameters work through XRDS APIs across the current built-in scene set
- [X] The editor can keep its own document model and map document objects to live XRDS handles cleanly
- [X] The authoring model is clear enough that contributors know where new editor-facing scene data should live

## Rule for future work

For any new editor-relevant feature, ask this first:

Can a non-expert use this feature through XRDS concepts alone?

If the answer is no, the feature should either:

- be promoted into the XRDS-facing layer, or
- be explicitly documented as an expert-only escape hatch

## Current highest-value next step

The next basement-level gap to address is planning the first non-glTF asset types that XRDS scene documents should support.

Why this is next:

- The glTF asset workflow is now strong enough for document placement, fallback handling, inspector editing, diagnostics, and runtime round-tripping.
- The remaining asset-workflow gap is not glTF policy but deciding which additional asset classes deserve first-class XRDS document support.
- That decision will shape future material, environment, audio, and media workflows more than another round of generic naming cleanup.

Recommended next concrete milestone:

- define the first non-glTF asset-kind expansion plan and keep it intentionally narrow:
	- textures or images for material-driven authoring and future texture slots
	- environment assets for skyboxes, HDRI lighting, and reflection environments
	- audio assets for ambient, trigger, and spatial sound references
	- video assets only if near-term scene/media workflows require them
	- reusable material assets or presets only if shared-material reuse pressure appears in real editor flows

- audit public XRDS names, example descriptions, and top-level wording so application-facing flows read naturally to non-experts and engine-shaped terminology is pushed to explicitly expert contexts
