# GLB Export Plan (Done)

Export the live XRDS scene to a self-contained `.glb` (binary glTF 2.0) file.

## Architecture decision

`.xrds` is an SDK-owned format (`crates/xrds-scene-graph`).
GLB export is therefore an SDK responsibility, not an editor responsibility.
A new crate `crates/xrds-gltf` owns the conversion.
The editor app simply calls `xrds_gltf::export_glb(document)` and saves the bytes.

```
crates/xrds-scene-graph   owns: XrdsSceneDocument, .xrds format, session
crates/xrds-gltf          new:  XrdsSceneDocument → GLB/glTF bytes (no Bevy dep)
apps/xrds-editor          uses: xrds_gltf::export_glb(document) → Vec<u8>
```

**Primitive mesh geometry** is generated with pure-math tessellation inside
`xrds-gltf` itself — no Bevy runtime dependency needed.  The tessellation is
identical to Bevy's `primitives` module results.

## Scope

| In scope                                                           | Out of scope                        |
| ------------------------------------------------------------------ | ----------------------------------- |
| Transforms, hierarchy                                              | USD/USDZ                            |
| PBR materials (base color, metallic, roughness, opacity, emissive) | Texture baking                      |
| Primitive meshes (Sphere, Cube, Cylinder, Plane, Tetrahedron)      | Skeletal animation                  |
| Cameras (perspective + orthographic)                               | Physics colliders                   |
| Lights via `KHR_lights_punctual`                                 | Audio (URI in `node.extras` only) |
| GltfAsset nodes — source URI in `node.extras`                   |                                     |
| Visibility (skip `visible = false` nodes)                        |                                     |
| Editor metadata →`node.extras`                                  |                                     |

## Subtask list

> All implementation subtasks completed and tested 2026-04-24.
> 133 total tests pass (7 new in xrds-gltf, 126 existing).

### ST-1  New `crates/xrds-gltf` crate

- [X] Create `crates/xrds-gltf/Cargo.toml`
  deps: `xrds-scene-graph` (workspace), `gltf-json = { version = "1.4", features = ["names","extras"] }`,
  `serde_json`
- [X] Add `xrds-gltf` to workspace members in root `Cargo.toml`
- [X] Create `src/lib.rs` with `export_glb(doc: &XrdsSceneDocument) -> Result<Vec<u8>, GlbExportError>`

### ST-2  Tessellation — pure-math mesh generation

- [X] `src/tessellation.rs`
- [X] `sphere(radius, lon_segs, lat_segs)` → `(positions, normals, uvs, indices)`
- [X] `cuboid(w, h, d)` → 24 vertices (4 per face, hard normals), 36 indices
- [X] `cylinder(radius, height, segments)` → side + top cap + bottom cap
- [X] `plane(w, h)` → 4 vertices, 2 triangles (XZ plane, Y-up normal)
- [X] `tetrahedron(radius)` → 12 vertices (3 per face), 12 indices

### ST-3  Buffer packing

- [X] `src/buffers.rs` — `BufferBuilder` that appends byte slices and tracks offsets
- [X] Helper: `push_f32_vec3_slice`, `push_f32_vec2_slice`, `push_u32_slice`
- [X] Returns `Vec<gltf_json::buffer::View>` + `Vec<gltf_json::Accessor>`

### ST-4  Scene graph → glTF JSON

- [X] `src/scene.rs` — walk `XrdsSceneDocument::nodes` → `Vec<gltf_json::Node>`
- [X] Resolve parent/child indices (flat index in the node array)
- [X] Skip `visible = false` nodes (but keep their children's indices consistent)
- [X] Translation / rotation (quaternion) / scale from `XrdsSceneTransform`
- [X] Write `node.name` (requires `names` feature)
- [X] Write XRDS metadata to `node.extras` (requires `extras` feature)

### ST-5  Materials → `pbrMetallicRoughness`

- [X] `src/materials.rs`
- [X] `base_color_factor`, `metallic_factor`, `roughness_factor`, `emissive_factor`
- [X] Opacity < 1 → `alpha_mode = BLEND`; unlit → `KHR_materials_unlit` extension
- [X] Double-sided flag

### ST-6  Cameras

- [X] `src/cameras.rs`
- [X] Perspective: `yfov` (radians), `znear`, `zfar`
- [X] Orthographic: `xmag`, `ymag`, `znear`, `zfar`

### ST-7  Lights (`KHR_lights_punctual`)

- [X] `src/lights.rs`
- [X] Point, Directional, Spot → `KHR_lights_punctual` light objects
- [X] `root.extensions_used` and `extensions_required` populated

### ST-8  GLB binary writer

- [X] `src/glb.rs`
- [X] `write_glb(json_bytes: &[u8], bin_bytes: &[u8]) -> Vec<u8>`
- [X] Header: magic `0x46546C67`, version 2, total length
- [X] JSON chunk: type `0x4E4F534A`, padded to 4 bytes with spaces
- [X] BIN chunk: type `0x004E4942`, padded to 4 bytes with zeros

### ST-9  Editor integration

- [X] `apps/xrds-editor/Cargo.toml` depends on `xrds-gltf`
- [X] `apps/xrds-editor/src/io.rs`: `export_glb(session, state)` function
- [X] "Export GLB" button in toolbar (Ctrl+Shift+E)

### ST-10  Tests  ✅ 2026-04-24 — 7/7 pass

- [X] GLB header magic, version, and total-length fields correct
- [X] JSON chunk is valid JSON with `asset.version = "2.0"`
- [X] Node name and translation preserved in export
- [X] Material base color factor preserved
- [X] Sphere tessellation: vertex count = (lon+1)×(lat+1), index count = lon×lat×6
- [X] Cuboid tessellation: 24 vertices (4 per face), 36 indices
- [X] Invisible nodes (`visible = false`) excluded from export

---

## glTF validator compliance requirements

- `POSITION` accessor must have `min` and `max`
- `bufferView.byteOffset` must be aligned to accessor component size
- `extensionsUsed` must list every extension referenced

## Dependency additions

```toml
# crates/xrds-gltf/Cargo.toml
gltf-json = { version = "1.4", features = ["names", "extras"] }
serde_json = { workspace = true }
xrds-scene-graph = { workspace = true }

# apps/xrds-editor/Cargo.toml (add)
xrds-gltf = { workspace = true }
```
