# XRDS Examples

Run an example with:

```bash
cargo run --example <example_name>
```

## Start Here

If you are new to XRDS, start with these in order:

1. [simple_api.rs](./simple_api.rs): the simplest XRDS-first runtime path with built-in cameras, lights, geometry helpers, and update context helpers.
2. [simple_scene.rs](./simple_scene.rs): a compact authored scene built from XRDS descriptors.
3. [simple_update.rs](./simple_update.rs): frame-to-frame runtime mutation through XRDS handles.
4. [scene_document_import.rs](./scene_document_import.rs): importing an authored `XrdsSceneDocument` into live runtime state.
5. [editor_basement_flow.rs](./editor_basement_flow.rs): editing a document through `XrdsSceneDocumentSession`, importing it, then applying runtime commit helpers to the imported handles.
6. [scene_document_export.rs](./scene_document_export.rs): importing authored scene data, committing runtime XRDS edits, and exporting the result back into a scene document snapshot.
7. [scene_document_session_save_load.rs](./scene_document_session_save_load.rs): saving a scene document session to JSON and loading it back through `XrdsSceneDocumentSession`.

These examples teach the intended default path: author in XRDS concepts, keep the document model outside the runtime, and use `XrdsAPI` as the runtime projection layer.

Important rule about ids:

- If you are writing runtime-first SDK code, prefer the typed `Handle<T>` returned by `api.spawn(...)` and let XRDS allocate ids for you.
- If you are writing document import/export code, explicit ids belong to the scene document because they are part of the authored data model and must survive round-trips.
- The document-oriented examples show explicit ids only to demonstrate stable authored identity, not because normal XRDS runtime usage should hard-code ids.

## Common Paths At A Glance

| Goal | Primary types | Identity model | Start here |
| --- | --- | --- | --- |
| Build a live app scene quickly | `XrdsAPI`, runtime descriptors like `XrdsCamera` and `XrdsCube` | Keep typed handles returned by `api.spawn(...)` | [simple_api.rs](./simple_api.rs) |
| Import or export authored scene data | `XrdsSceneDocument`, `XrdsSceneNode`, `XrdsAPI` | Stable document ids preserved across round-trips | [scene_document_import.rs](./scene_document_import.rs), [scene_document_export.rs](./scene_document_export.rs) |
| Save, load, and edit a durable scene document | `XrdsSceneDocumentSession` | Stable document ids owned by the document model | [scene_document_session_save_load.rs](./scene_document_session_save_load.rs), [editor_basement_flow.rs](./editor_basement_flow.rs) |

## XRDS-First Examples

| Example | Purpose |
| --- | --- |
| [simple_api.rs](./simple_api.rs) | Smallest end-to-end XRDS runtime example with typed helper methods. |
| [simple_scene.rs](./simple_scene.rs) | Basic scene authored directly with XRDS descriptors. |
| [simple_update.rs](./simple_update.rs) | Simple per-frame updates through XRDS handles. |
| [parent_child.rs](./parent_child.rs) | Runtime hierarchy edits after spawn. |
| [parent_child_queued.rs](./parent_child_queued.rs) | Hierarchy declared up front in queued XRDS creation flows. |
| [edit_material.rs](./edit_material.rs) | Material editing through XRDS-authored material params. |
| [active_compo_control.rs](./active_compo_control.rs) | XRDS-centric component control flow. |
| [load_gltf.rs](./load_gltf.rs) | Loading authored glTF scene content through XRDS. |
| [load_gltf_fail_case.rs](./load_gltf_fail_case.rs) | Handling glTF load failures through XRDS-facing status queries. |
| [scene_document_import.rs](./scene_document_import.rs) | Importing scene-graph document data into live runtime entities. |
| [editor_basement_flow.rs](./editor_basement_flow.rs) | End-to-end editor-basement flow: document session edits, runtime import, and queued runtime commit helpers. |
| [scene_document_export.rs](./scene_document_export.rs) | Export-focused XRDS flow: runtime edits committed through XRDS and exported back into a document snapshot. |
| [scene_document_session_save_load.rs](./scene_document_session_save_load.rs) | Document-only session workflow: save JSON, edit, save in place, and reload through `XrdsSceneDocumentSession`. |

## Extension-First Examples

These show the open extension model. Use them when built-in XRDS components are not enough and you need custom descriptors or custom patch behavior.

| Example | Purpose |
| --- | --- |
| [generic_update.rs](./generic_update.rs) | Custom descriptor plus custom patch type while still realizing through XRDS-owned geometry/material paths. |
| [net.rs](./net.rs) | XRDS networking path without dropping into engine-shaped app structure. |
| [net_bevy.rs](./net_bevy.rs) | Networking example closer to direct Bevy integration. |

## Expert Escape Hatch

These examples are useful, but they are not the recommended first contact for non-experts or future editor-facing integrations.

They import Bevy directly on purpose. XRDS no longer re-exports Bevy for them.

| Example | Layer | Purpose |
| --- | --- | --- |
| [direct_bevy.rs](./direct_bevy.rs) | `Direct-Bevy` | Direct Bevy scene construction and animation inside an XRDS runtime shell. Use this when XRDS abstractions are intentionally not the goal. |

## Rendering And Feature Demos

These are primarily feature showcases rather than editor-basement teaching examples.

For strict layering, treat the label below as authoritative:

- `XRDS-first`: the scene is primarily authored through `XrdsAPI` and XRDS descriptors, even if a small Bevy bridge is still needed.
- `Direct-Bevy`: the demo is primarily about Bevy render, animation, post-process, mesh, or camera APIs. XRDS may still host the runtime shell, but it is not the teaching surface.

| Example | Layer | Why It Is Labeled That Way | Screenshot |
| --- | --- | --- | --- |
| [2d_bloom.rs](./2d_bloom.rs) | `Direct-Bevy` | Uses Bevy 2D camera, bloom, tonemapping, meshes, and materials directly through `Commands`. | <img src="./screenshots/2d_bloom.png" width="400"> |
| [2d_shapes.rs](./2d_shapes.rs) | `Direct-Bevy` | Pure Bevy 2D shapes and camera setup; XRDS is not the authoring surface. | <img src="./screenshots/2d_shapes.png" width="400"> |
| [3d_bloom.rs](./3d_bloom.rs) | `XRDS-first` | Uses XRDS camera, XRDS spheres, and XRDS material/emissive helpers as the primary scene API. | <img src="./screenshots/3d_bloom.png" width="400"> |
| [3d_shapes.rs](./3d_shapes.rs) | `Direct-Bevy` | Demonstrates Bevy mesh generation, textures, and `StandardMaterial` usage directly. | <img src="./screenshots/3d_shapes.png" width="400"> |
| [atmospheric_fog.rs](./atmospheric_fog.rs) | `Direct-Bevy` | Fog, shadow, glTF scene loading, and sky material setup are all done with Bevy rendering APIs. | <img src="./screenshots/atmospheric_fog.png" width="400"> |
| [custom_skinned_mesh.rs](./custom_skinned_mesh.rs) | `Direct-Bevy` | Skinning, joints, mesh assets, and animated hierarchy are authored directly with Bevy ECS/render types. | <img src="./screenshots/custom_skinned_mesh.png" width="400"> |
| [light_transmission.rs](./light_transmission.rs) | `Direct-Bevy` | The example is centered on Bevy lighting and material configuration, not XRDS descriptors. | <img src="./screenshots/light_transmission.png" width="400"> |
| [morph_targets.rs](./morph_targets.rs) | `XRDS-first` | Loads the morph-target glTF through XRDS and starts playback through the XRDS animation API, so the default teaching surface remains `XrdsAPI` rather than direct Bevy animation wiring. | <img src="./screenshots/morph_targets.png" width="400"> |
| [physical_based_rendering.rs](./physical_based_rendering.rs) | `Direct-Bevy` | PBR grid setup, environment maps, orthographic projection, and labels are all authored directly in Bevy. | <img src="./screenshots/physical_based_rendering.png" width="400"> |
| [postprocessing_builtin.rs](./postprocessing_builtin.rs) | `Direct-Bevy` | Post-processing configuration and scene setup use Bevy camera/material APIs directly. | <img src="./screenshots/postprocessing_builtin.png" width="400"> |
| [simple_xr_scene.rs](./simple_xr_scene.rs) | `XRDS-first` | Scene content is authored through XRDS descriptors; Bevy is only used for the explicit `OpenXrCamera` startup bridge. | <img src="./screenshots/simple_xr_scene.png" width="400"> |
| [skybox.rs](./skybox.rs) | `Direct-Bevy` | Skybox, TAA, SSAO, cubemap resource management, and lighting are all Bevy-first rendering concerns. | <img src="./screenshots/skybox.png" width="400"> |
| [spherical_area_lights.rs](./spherical_area_lights.rs) | `Direct-Bevy` | Area lights, meshes, and materials are configured directly with Bevy render components. | <img src="./screenshots/spherical_area_lights.png" width="400"> |
| [split_screen.rs](./split_screen.rs) | `Direct-Bevy` | Viewports, multiple cameras, UI overlays, and mesh/material setup are all direct Bevy APIs. | <img src="./screenshots/split_screen.png" width="400"> |
| [two_pass.rs](./two_pass.rs) | `Direct-Bevy` | The point of the example is multi-camera render ordering, implemented directly with Bevy camera APIs. | <img src="./screenshots/two_pass.png" width="400"> |
| [vertex_color.rs](./vertex_color.rs) | `Direct-Bevy` | Vertex attribute mutation, mesh construction, and material handling are authored directly with Bevy mesh APIs. | <img src="./screenshots/vertex_color.png" width="400"> |

## Picking The Right Surface

- Prefer XRDS-first examples when you want application-level scene construction, document import, inspector-style runtime edits, or future editor integration.
- Prefer extension-first examples when you need custom descriptors but still want XRDS to own realization and update flow.
- Prefer the expert escape hatch only when direct Bevy control is intentional and you do not want XRDS to be the primary abstraction.
