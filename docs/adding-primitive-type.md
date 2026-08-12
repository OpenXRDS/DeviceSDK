# Adding A Primitive Type

This guide defines the default path for adding a new XRDS primitive type.

The goal is to keep primitive growth predictable without turning runtime
registration into a monolithic file.

**Scope.** Everything through §6 makes a primitive spawnable and mutable
through `XrdsAPI` at runtime — that is the whole surface a non-expert app
needs. §7 covers the *additional* work if the primitive must also be
**document-authorable**: saved in a `scene.json`, placed from the editor
palette, and edited in the Inspector. That is roughly double the file count
of the runtime-only path, and it is easy to think you're done after §6
because everything still compiles — the editor-side sites that are not
compiler-enforced (see §7) fail silently rather than with an error.

Worked example throughout: `XrdsCapsule`, added alongside `XrdsCylinder` as
its closest existing analogue. Every file path and code sample below is the
real, compiling result — not illustrative pseudocode.

## Rule Of Thumb

- Put descriptor shape in `xrds-components`.
- Register the spawn interpreter in `xrds-runtime/src/xrds_api/registry.rs`.
- Put the actual descriptor→entity spawn function in `xrds-runtime/src/xrds_api/spawn.rs`.
- Keep primitive geometry edits on the generic patch path unless a helper clearly pays for itself across many call sites.
- Put built-in runtime patch registration in `xrds-runtime/src/xrds_api/updaters.rs`.
- Keep `xrds-runtime/src/xrds_api/install.rs` limited to plugin/system/resource
  installation — it does **not** hold per-primitive spawn-interpreter
  registration; that lives in `registry.rs`.
- Prefer recipe-backed patch updates over direct Bevy mesh/material mutation when possible.

## Files To Edit — Runtime Layer

### 1. Define The Primitive Descriptor

Add the descriptor type in `crates/xrds-components/src/primitives/`.

Typical work:

- create a new file such as `capsule.rs`
- add the primitive struct fields
- implement `XrdsObject`
- implement `XrdsComponent`
- implement `XrdsMutableComponent`

Built-in descriptors should stay plain XRDS data. Do not derive Bevy `Component` on the descriptor and do not put Bevy spawn methods on it.

Use existing primitives as templates:

- `crates/xrds-components/src/primitives/cube.rs`
- `crates/xrds-components/src/primitives/sphere.rs`
- `crates/xrds-components/src/primitives/cylinder.rs`
- `crates/xrds-components/src/primitives/capsule.rs`
- `crates/xrds-components/src/primitives/plane.rs`

If the primitive's dimensions have a naming ambiguity — `XrdsCapsule::length`
is deliberately not `height`, because avian3d's `Collider::capsule` and
Bevy's `Capsule3d` both mean "the straight segment, **excluding** the two
hemispherical caps" — name the field to match the underlying engine
convention and say so in a doc comment. Total visible extent is
`length + 2 * radius`.

### 2. Export The Primitive From xrds-components

Update the primitive module exports so the new type is publicly reachable.

Usually this means editing:

- `crates/xrds-components/src/primitives/mod.rs`
- any re-export point in `crates/xrds-components/src/lib.rs` if needed

### 3. Add Patch Payload Types If Needed

If the primitive needs runtime-editable geometry parameters, define patch payloads in:

- `crates/xrds-components/src/values.rs`

Examples:

- `CubeGeometryParams`
- `CylinderGeometryParams`
- `CapsuleGeometryParams`
- `SphereGeometryParams`
- `Plane3DGeometryParams`
- `TetrahedronGeometryParams`

Only add a new patch type when the primitive needs a user-facing mutation API.

### 4. Add A `XrdsGeometrySource` Variant And Wire The Two Recipe Matches

If the primitive's mesh can be rebuilt purely from its descriptor (true for
every built-in shape so far), add a variant to `XrdsGeometrySource` in
`crates/xrds-runtime/src/xrds_api.rs`:

```rust
/// Fallback: PBR capsule primitive. `name`, `transform`, and `visible` are filled
/// from the descriptor. `half_length` excludes the hemispherical caps.
PbrCapsule {
    radius: f32,
    half_length: f32,
    material: XrdsMaterialParams,
},
```

Adding a variant here makes `crates/xrds-runtime/src/xrds_api/recipes.rs`
**fail to compile** at its two exhaustive `match recipe { ... }` blocks — this
is deliberate and is the whole reason to prefer a `XrdsGeometrySource` variant
over a one-off closure: the compiler finds every place a new shape needs a
mesh-building arm, instead of a silent gap. Add the matching arm to both:

```rust
XrdsGeometrySource::PbrCapsule { radius, half_length, material } => {
    apply_pbr_recipe_to_entity(
        world, entity,
        Mesh::from(Capsule3d { radius, half_length }),
        material,
    );
}
```

### 5. Register The Spawn Interpreter And Write The Spawn Function

The actual descriptor→entity spawn function goes in
`crates/xrds-runtime/src/xrds_api/spawn.rs`:

```rust
pub(super) fn spawn_capsule_descriptor(
    commands: &mut Commands,
    capsule: &XrdsCapsule,
) -> Entity {
    // ... build the Bevy mesh (Capsule3d::new(radius, length)) and physics
    // collider (avian3d::prelude::Collider::capsule(radius, length)) the same
    // way spawn_cylinder_descriptor does, via insert_physics_components.
}
```

Then register it in `crates/xrds-runtime/src/xrds_api/registry.rs` —
**not** `install.rs`:

```rust
registry.register_recipe_only::<XrdsCapsule, _>(|capsule| XrdsGeometrySource::PbrCapsule {
    radius: capsule.radius,
    half_length: capsule.length * 0.5,
    material: XrdsMaterialParams::default(),
});
registry.register_entity::<XrdsCapsule, _>(|capsule, commands, _asset_server| {
    spawn_capsule_descriptor(commands, capsule)
});
```

Also add `registry.register_clone::<XrdsCapsule>();` in
`register_default_descriptor_cloners()` in the same file.

### 6. Register Primitive Updaters

Add built-in patch registration in:

- `crates/xrds-runtime/src/xrds_api/updaters.rs`

This is the main place to extend runtime mutation behavior for primitives.

Use this split:

- stored descriptor behavior: `register_stored_*_updaters(...)`
- primitive-specific descriptor patching: geometry-source-backed updater registration

Real, compiling pattern:

```rust
fn register_stored_capsule_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsCapsule>(registry);

    registry.register::<XrdsCapsule, XrdsColor, _>(|world, entity, color| {
        let mut params = material_params_for_entity(world, entity).unwrap_or_default();
        params.base_color = *color;
        apply_authored_material_to_entity(world, entity, params);
    });

    registry.register::<XrdsCapsule, XrdsMaterialParams, _>(|world, entity, params| {
        apply_authored_material_to_entity(world, entity, params.clone());
    });

    registry.register::<XrdsCapsule, CapsuleGeometryParams, _>(|world, entity, params| {
        if with_stored_descriptor_mut::<XrdsCapsule, _>(world, entity, |descriptor| {
            descriptor.radius = params.radius;
            descriptor.length = params.length;
        })
        .is_none()
        {
            return;
        }

        let Some((recipe, name, transform, visible)) =
            capsule_recipe_and_common_state_for(world, entity)
        else {
            return;
        };

        apply_spawn_recipe_to_entity(world, entity, recipe, name, transform, visible);
    });
}
```

...plus a `capsule_recipe_and_common_state_for` in `recipes.rs` and a
`capsule_descriptor_ref` in `state.rs`, mirroring cylinder's — see those
files for the exact shape.

Avoid writing direct `Mesh3d` or `StandardMaterial` mutation code here unless there is a measured reason to avoid geometry-source rebuild.

### 6b. Add Public XRDS Convenience Methods Only If They Remove Real Repetition

If the primitive needs obvious high-level methods, add them in:

- `crates/xrds-runtime/src/xrds_api.rs`

For primitive geometry, prefer the existing `set_*_geometry` shape:

```rust
/// Queue a capsule geometry update.
pub fn set_capsule_geometry(
    &mut self,
    handle: &Handle<XrdsCapsule>,
    params: CapsuleGeometryParams,
) -> &mut Self {
    self.queue_update(handle, params)
}
```

Called as `xrds.set_capsule_geometry(&capsule_handle, CapsuleGeometryParams { radius: 0.5, length: 1.5 })`.

Do not add a public helper just because a patch type exists. Only add helpers when they remove repeated multi-field boilerplate or express a cross-type concept better than `queue_update(...)`.

### 6c. Add Or Update Examples

If the primitive is user-facing, add or update an example under:

- `examples/`

Examples should use the XRDS-facing API first, not raw Bevy systems, unless the example is explicitly demonstrating the expert path.

## 7. Files To Edit — Document + Editor Layer (only if the primitive must be authorable)

Everything above makes the primitive usable from Rust via `XrdsAPI`. If it
also needs to be **saved in a scene document and placed/edited from the GUI
editor**, there is a second, larger pass across `xrds-scene-graph` and
`apps/xrds-editor`. Unlike §1–6, most of the editor-side sites below are
**not** compiler-enforced (`_ => ...` catch-alls) — omitting one does not
fail the build, it silently drops a capability (the palette click does
nothing, the Inspector shows "Other", export skips the node). Grep for the
closest existing primitive's name across both directories rather than
trusting any fixed list, including this one.

### `crates/xrds-scene-graph`

- `src/lib.rs`: import the descriptor type from `xrds_components::primitives`.
- `src/scene/payload.rs`:
  - add a variant to `XrdsSceneNodePayload` (**compiler-enforced** — this enum
    has an exhaustive `gltf_export_class()` match with no `_` arm)
  - add the `XrdsScene*` struct (mirrors the descriptor, plus `material:
    XrdsSceneMaterial`, serde defaults for `physics_body`/`gravity_scale`/`mass`)
    and its `Default` impl
- `src/scene/node.rs`:
  - add a variant to `XrdsSceneRuntimeComponent`
  - add an arm to `to_runtime_node_with_gltf_asset_uri()` (**compiler-enforced**)
  - add a `from_xrds_*()` conversion function, mirroring the closest existing one
- `src/document/material.rs`: `node_material_ref()` / `node_material_mut()` —
  **not enforced**; omitting an arm here means the shape has no editable
  material.
- `src/document/assets/gltf.rs`: the asset-id rewrite loop — **not
  enforced**; omitting an arm here silently breaks texture-asset-id rewrites
  when a texture asset is renamed or replaced.

### `crates/xrds-runtime` (two more sites beyond §1–6)

- `xrds_api/helper.rs`: the scene-export `if let Some(descriptor) = world.get::<XrdsStored<T>>(entity)`
  chain — **not enforced, and it's a chain of early returns, not a match**;
  omitting a branch means the shape silently fails to export at all.
- `xrds_api/api.rs`: the exhaustive `match component` in `import_scene_document`
  (**compiler-enforced**) and the `set_*_geometry` public setter (§6b).
- `xrds_api/reimport.rs`: the exhaustive `match component` in
  `spawn_runtime_component` (**compiler-enforced**).

### `apps/xrds-editor/src-tauri` (Rust)

- `bridge.rs`: add a `NodePayloadDto` variant (`#[serde(tag = "type")]`).
- `palette.rs`: the `kind` string → payload match in `build_primitive_node`
  (**not enforced** — a missing arm means clicking the palette entry does
  nothing); the default-placement-transform match; the geometry-overlap-offset
  `matches!` list.
- `hierarchy.rs`: `payload_kind()` (**compiler-enforced**, exhaustive, no `_` arm).
- `inspector.rs`: **six** sites — `build_payload_dto` (not enforced, falls
  back to a generic "Other" DTO with no material/physics UI),
  `payload_kind_name()` (**compiler-enforced**, duplicates `hierarchy::payload_kind`),
  and the `SetPhysicsBody`/`SetGravityScale`/`SetMass`/`set_node_material`
  match arms (each `_ => {}`, not enforced, each silently no-ops).
- `viewport_gizmo.rs`: if the shape has a physics collider, add a
  `ColliderShape` variant and draw its wireframe.
- `viewport_selection.rs`: `pick_radius()` — the bounding-sphere radius used
  for click-picking in the viewport.

### `apps/xrds-editor/src` (frontend)

- `types/bridge.ts`: the `NodePayload` union member (mirrors the Rust DTO)
  and a `KIND_ICON` entry.
- `components/Palette.tsx`: a `PALETTE_META` entry and add the kind string
  to the right group in `PRIMITIVE_GROUPS` — **this string must match the
  `palette.rs` match arm exactly**, there is no shared constant.
- `components/Inspector.tsx`: if the shape reuses the generic material+physics
  panel (`PrimitiveSection`), add its `payload.type` to the disjunction in
  `PayloadSection`. Most primitives (Cube/Sphere/Cylinder/Plane) still have
  **no editor UI for their own dimensions** — only material and physics are
  editable from the Inspector; geometry is Rust-only via `set_*_geometry`.
  `XrdsCapsule` is the first exception: `CapsuleGeometrySection` in
  `Inspector.tsx` adds radius/length sliders, using the same one-command
  live-preview shape as `SetGravityScale`/`SetMass`
  (`SetCapsuleGeometry { id, radius, length }`, sent identically on drag and
  on release — not the separate `Set*`/`Commit*` split `SetMaterial` uses).
  Adding this for another primitive means: the DTO needs the dimension
  fields added (`bridge.rs`/`bridge.ts`), a new `EditorCommand` variant,
  a `pending_*_geometry` field on `EditorState`, a
  `set_*_geometry_for_node(id, params)` on `XrdsUpdateContext` (constructs a
  `Handle<C>` from the resolved `Entity` via `entity.into()`, then calls the
  existing `queue_update` — the `SurfaceUpdateRegistry` wiring from §6 is
  reused as-is, nothing new needed there), and the apply-and-clear block in
  `bevy_scene.rs` that reads the pending field each frame. See
  `set_capsule_geometry_for_node` in `xrds_api/context.rs` for the whole
  pattern in one place.

### Tests

- `crates/xrds-runtime/src/tests/builtins.rs`:
  `built_in_geometry_commit_helpers_update_runtime_and_exported_document` is
  the one test every built-in primitive participates in — spawn, apply a
  geometry patch, export, and assert the exported payload. Add the new shape
  to it.
- There is **no dedicated physics/collider test suite** anywhere in the
  workspace. If the shape's physics collider convention differs from its
  mesh's (check the engine's actual argument semantics — do not assume
  "half of X" without reading the source; this bit `XrdsCylinder`'s collider,
  which was half the height of its visible mesh for an unknown length of
  time with nothing to catch it), that mismatch will not be caught by
  anything here. Verify by reading the physics crate's source directly.

## Recommended Decision Path

When adding a primitive, use this sequence:

1. Can the runtime representation be rebuilt from the descriptor alone?
2. If yes, use geometry-source-backed updaters.
3. If no, add a specialized low-level updater only for the patch that truly needs it.
4. Keep the descriptor stored through `XrdsStored<C>` and wire transform/parent/name/visibility updates through the runtime-side stored-descriptor helpers.
5. Decide up front whether §7 (document + editor authoring) is in scope —
   it roughly doubles the file count and most of its sites fail silently
   rather than with a compiler error, so it is easy to ship a primitive that
   works from Rust but is invisible or broken in the editor.

## What Should Not Grow

These files should stay narrow in responsibility:

- `crates/xrds-runtime/src/xrds_api/install.rs`: plugin/system/resource
  install and setup only — **not** per-primitive spawn-interpreter
  registration (that's `registry.rs`)
- `crates/xrds-runtime/src/xrds_api.rs`: public XRDS facade only

If primitive work starts pushing a lot of code into one runtime file, move it into:

- `crates/xrds-runtime/src/xrds_api/updaters.rs`

If `updaters.rs` becomes too large, split it again by concern:

- primitive updaters
- light updaters
- camera updaters
- asset updaters

## Minimal Checklist

Runtime layer (§1–6):

- descriptor type exists in `xrds-components`
- primitive is exported publicly
- descriptor stays free of Bevy `Component` derive and Bevy-owned `spawn(...)`
- `XrdsGeometrySource` variant added, both `recipes.rs` matches updated
- spawn function written in `spawn.rs`; spawn interpreter registered in `registry.rs`
- if rebuildable, a matching `XrdsGeometrySource` variant or existing source is used
- stored-descriptor updaters are registered in `updaters.rs`
- primitive-specific patch updaters are registered if needed
- XRDS public helper methods exist only where useful
- at least one example or validation path covers the primitive
- `cargo check -p xrds-runtime` passes

Document + editor layer (§7, only if authorable):

- `XrdsSceneNodePayload` variant + struct in `xrds-scene-graph`, both
  compiler-enforced matches (`gltf_export_class`, `to_runtime_node`) updated
- `node_material_ref`/`_mut` and the gltf asset-id rewrite loop updated
  (**neither is compiler-enforced** — verify by hand)
- `xrds_api/helper.rs` export chain, `api.rs` import match, `reimport.rs`
  spawn match all updated
- editor bridge DTO (`bridge.rs`/`bridge.ts`), palette entry, hierarchy/inspector
  kind-name matches, and the four Inspector mutation arms all updated
- frontend palette group + Inspector disjunction updated
- `cargo check --workspace --all-targets` passes (catches the enforced sites);
  manually re-verify the unenforced ones by grepping the closest existing
  primitive's name across `apps/xrds-editor` and `xrds-scene-graph`
