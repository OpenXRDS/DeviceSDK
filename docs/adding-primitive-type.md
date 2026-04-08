# Adding A Primitive Type

This guide defines the default path for adding a new XRDS primitive type.

The goal is to keep primitive growth predictable without turning runtime registration into a monolithic file.

## Rule Of Thumb

- Put descriptor shape in `xrds-components`.
- Put built-in Bevy spawn/storage wiring in `xrds-runtime/src/xrds_api/install.rs`.
- Keep primitive geometry edits on the generic patch path unless a helper clearly pays for itself across many call sites.
- Put built-in runtime patch registration in `xrds-runtime/src/xrds_api/updaters.rs`.
- Keep `xrds-runtime/src/xrds_api/install.rs` limited to install/setup and spawn-recipe plumbing.
- Prefer recipe-backed patch updates over direct Bevy mesh/material mutation when possible.

## Files To Edit

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
- `crates/xrds-components/src/primitives/plane.rs`

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
- `SphereGeometryParams`
- `Plane3DGeometryParams`
- `TetrahedronGeometryParams`

Only add a new patch type when the primitive needs a user-facing mutation API.

### 4. Register The Primitive Interpreter

Add the spawn interpreter in:

- `crates/xrds-runtime/src/xrds_api/install.rs`

This is the place where built-in descriptor types are connected to spawn behavior.

Example pattern:

```rust
registry.register_entity::<XrdsCapsule, _>(|capsule, commands, _asset_server| {
registry.register_recipe_only::<XrdsTetrahedron, _>(|tetrahedron| XrdsGeometrySource::PbrTetrahedron {
    vertices: tetrahedron.vertices.map(Into::into),
    color_rgba: XrdsColor::WHITE.rgba,
});

registry.register_entity::<XrdsTetrahedron, _>(|tetrahedron, commands, _asset_server| {

Current built-in example pattern:
        XrdsGeometrySource::PbrTetrahedron {
            vertices: tetrahedron.vertices.map(Into::into),
            color_rgba: XrdsColor::WHITE.rgba,
    radius: capsule.radius,
        tetrahedron.name.clone(),
        tetrahedron.transform,
        tetrahedron.visible,

    commands.entity(entity).insert(XrdsStored(tetrahedron.clone()));
    let entity = execute_spawn_recipe(
        commands,
        XrdsGeometrySource::PbrCapsule {
            radius: capsule.radius,
            half_height: capsule.height * 0.5,
            color_rgba: XrdsColor::WHITE.rgba,
        },
        capsule.name.clone(),
        capsule.transform,
        capsule.visible,
    );
    commands.entity(entity).insert(XrdsStored(capsule.clone()));
    entity
});
```

### 5. Register Primitive Updaters

Add built-in patch registration in:

- `crates/xrds-runtime/src/xrds_api/updaters.rs`

This is now the main place to extend runtime mutation behavior for primitives.

Use this split:

- stored descriptor behavior: `register_stored_*_updaters(...)`
- primitive-specific descriptor patching: geometry-source-backed updater registration

Preferred pattern:

```rust
register_stored_capsule_updaters(registry);

registry.register::<XrdsTetrahedron, TetrahedronGeometryParams, _>(|world, entity, params| {
    if with_stored_descriptor_mut::<XrdsTetrahedron, _>(world, entity, |tetrahedron| {
        tetrahedron.vertices = params.vertices.map(Into::into);
    })
    .is_none()
    {
        return;
    }

    let Some((geometry, name, transform, visible)) =
        tetrahedron_recipe_and_common_state_for(world, entity)
    else {
        return;
    };

    apply_spawn_recipe_to_entity(world, entity, geometry, name, transform, visible);
});
```

Avoid writing direct `Mesh3d` or `StandardMaterial` mutation code here unless there is a measured reason to avoid geometry-source rebuild.

### 6. Add Public XRDS Convenience Methods Only If They Remove Real Repetition

If the primitive needs obvious high-level methods, add them in:

- `crates/xrds-runtime/src/xrds_api.rs`

For primitive geometry, prefer:

```rust
api.queue_update(&capsule_handle, CapsuleGeometryParams {
    radius: 0.5,
    height: 1.5,
});
```

Do not add a public helper just because a patch type exists. Only add helpers when they remove repeated multi-field boilerplate or express a cross-type concept better than `queue_update(...)`.

### 7. Add Or Update Examples

If the primitive is user-facing, add or update an example under:

- `examples/`

Examples should use the XRDS-facing API first, not raw Bevy systems, unless the example is explicitly demonstrating the expert path.

## Recommended Decision Path

When adding a primitive, use this sequence:

1. Can the runtime representation be rebuilt from the descriptor alone?
2. If yes, use geometry-source-backed updaters.
3. If no, add a specialized low-level updater only for the patch that truly needs it.
4. Keep the descriptor stored through `XrdsStored<C>` and wire transform/parent/name/visibility updates through the runtime-side stored-descriptor helpers.

## What Should Not Grow

These files should stay narrow in responsibility:

- `crates/xrds-runtime/src/xrds_api/install.rs`: install/setup and geometry-source plumbing only
- `crates/xrds-runtime/src/xrds_api.rs`: public XRDS facade only

If primitive work starts pushing a lot of code into one runtime file, move it into:

- `crates/xrds-runtime/src/xrds_api/updaters.rs`

If `updaters.rs` becomes too large, split it again by concern:

- primitive updaters
- light updaters
- camera updaters
- asset updaters

## Minimal Checklist

When adding a new primitive, verify all of these:

- descriptor type exists in `xrds-components`
- primitive is exported publicly
- descriptor stays free of Bevy `Component` derive and Bevy-owned `spawn(...)`
- spawn interpreter is registered
- if rebuildable, a matching `XrdsGeometrySource` variant or existing source is used
- stored-descriptor updaters are registered
- primitive-specific patch updaters are registered if needed
- XRDS public helper methods exist only where useful
- at least one example or validation path covers the primitive
- `cargo check -p xrds-runtime` passes
