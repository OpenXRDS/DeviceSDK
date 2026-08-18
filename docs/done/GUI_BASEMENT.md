# GUI Editor — SDK Boundary

This document defines the architectural boundary between the XRDS SDK (`DeviceSDK`) and a GUI editor application built on top of it.

The SDK's role is to expose stable, application-level contracts.
The editor app's role is to own the GUI, user interactions, panels, and selection state.

---

## What belongs in the SDK (this repo)

The SDK must provide everything the editor app needs to operate **without reaching into Bevy internals**. As of 2026-04-16, this is complete:

### Scene data (xrds-scene-graph)

| Contract | Status |
| --- | --- |
| Durable node identity (`XrdsSceneNodeId`, `XrdsId`) | ✅ |
| Hierarchy queries (parent, children, ancestors) | ✅ |
| Document save / load (JSON round-trip) | ✅ |
| Undo / redo session (`XrdsSceneDocumentSession`) | ✅ |
| Dirty state and save-path tracking | ✅ |
| Editor metadata per node (tags, layer, lock, hidden, custom props) | ✅ |
| Asset catalog with diagnostics (usage, source health, unused) | ✅ |
| All node types the editor can author | ✅ (`Empty`, `Camera`, `GltfAsset`, `Cube`, `Cylinder`, `Sphere`, `Plane3D`, `Tetrahedron`, `PointLight`, `DirectionalLight`, `SpotLight`, `AmbientLight`, `AudioClip`) |

### Runtime interface (xrds-runtime)

| Contract | Status |
| --- | --- |
| Spawn any node type without Bevy knowledge | ✅ |
| Full inspector read API (camera, lights, glTF, material, textures) | ✅ |
| Immediate-preview write API (all of the above) | ✅ |
| Queued-commit write API for undo-safe edits | ✅ |
| Import document → runtime / export runtime → document | ✅ |
| Hierarchy queries at runtime (parent/child by XrdsId) | ✅ |
| `entity_of_id` expert escape hatch (labeled as expert-only) | ✅ |

### SDK gaps that would block editor work

None currently blocking. Remaining SDK-side work is incremental:

- `XrdsCapsule` if character workflows appear
- `Video` asset kind if media workflows appear
- `TransformParams` dual-rotation clarification (structural, low priority)

---

## What belongs in the editor app (separate repo / crate)

These are **not SDK features**. They are the responsibility of the application built on top.

### Editor state

- Selected node(s), hovered node, clipboard
- Panel layout and window positions
- Camera orbit/pan/zoom state
- Play-mode flag

### GUI panels

- Hierarchy tree (reads `XrdsHierarchyIndex` and `XrdsSceneDocument`)
- Inspector panel (reads `XrdsAPI` inspector methods, writes via session + runtime)
- Asset browser (reads `document.asset_diagnostics()`, calls `session.register_*_asset()`)
- Scene viewport (the Bevy 3D window — already exists)

### Transform gizmo

- Move / rotate / scale handles in the viewport
- On drag: calls `ctx.set_translation` / `ctx.set_rotation` (immediate preview)
- On release: commits to session via `queue_update(handle, TransformParams { ... })`

### Scene management

- New / open / save / save-as (delegates to `XrdsSceneDocumentSession`)
- Undo / redo buttons (delegates to `session.undo()` / `session.redo()`)
- Import glTF from disk (delegates to `session.register_gltf_asset()` + `session.place_gltf_asset()`)

### Play mode

- Snapshot the document, hand runtime control to the app logic, restore on exit

---

## Recommended editor architecture

```text
xrds-editor (new crate or repo)
  ├── XrdsEditorApp  implements XrdsApp
  │     ├── owns XrdsSceneDocumentSession  (authoritative document)
  │     ├── setup()  →  api.import_scene_document(session.document())
  │     └── update() →  read EditorInput, update EditorState,
  │                     call ctx.set_*() for preview,
  │                     commit to session on action complete
  ├── panels/
  │     ├── hierarchy.rs   reads XrdsHierarchyIndex + document nodes
  │     ├── inspector.rs   reads XrdsAPI inspector methods
  │     └── asset_browser.rs  reads document.asset_diagnostics()
  └── gizmo.rs             transform handles using Bevy gizmos
```

The SDK surface the editor app calls:

```rust
// Document authoring (session)
session.set_node_material(id, material)?;
session.rename_node(id, new_name)?;
session.reparent_node(id, new_parent)?;
session.undo(); session.redo();
session.save_as(path)?;

// Runtime preview (context, called from update())
ctx.set_translation(&handle, [x, y, z]);
ctx.set_material_base_color(&handle, color);
ctx.set_material_texture_slot(&handle, BaseColor, Some(tex_ref));
ctx.camera_projection(&camera_handle)  // read
ctx.point_light_params(&light_handle)  // read

// Import / export
api.import_scene_document(&session.document())?;
api.export_scene_document()?;
```

The editor app **never** accesses `world`, `Entity`, or Bevy ECS types directly in panel or session code. If a panel needs data the SDK doesn't expose, the right fix is to add an inspector read method to the SDK — not to query Bevy from UI code.

---

## GUI framework recommendation

`bevy_egui` — immediate-mode panels rendered over the Bevy 3D viewport inside the same process. No IPC required. `egui` is well-suited for dev-tool UIs (trees, property grids, drag-drop).
