# XRDS Runtime

Runtime library for XRDS application

## API direction

The runtime is intentionally split into two layers.

- Default layer: `XrdsApp`, `XrdsAPI`, and `XrdsUpdateContext`
- Expert layer: `RuntimeHandler` and direct Bevy systems

The default layer is designed for non-experts and should cover normal XR application workflows without requiring familiarity with Bevy.

The expert layer exists for advanced integrations, custom engine-level behavior, and cases where direct Bevy control is necessary.

When adding features to this crate, prefer extending the XRDS-facing layer first. Dropping to Bevy should be optional for routine work and expected only for advanced use cases.

For strict layering, this crate does not re-export Bevy. Expert-layer code should depend on and import `bevy` explicitly.

## Scene Environment Policy

Scene environment support now has two intended entry points:

- document-driven: import a saved `XrdsSceneDocument` and let XRDS project its authored environment metadata into runtime
- runtime-driven: call `merge_scene_assets(...)`, `set_scene_environment(...)`, and `clear_scene_environment(...)` directly on `XrdsAPI`

Use the document-driven path when environment belongs to durable authored scene meaning. Use the runtime-driven path when environment belongs to live application state, game mode, quality tier, or similar runtime policy.

The runtime-driven path still uses `XrdsSceneAsset` ids and `XrdsSceneEnvironment` because some scene-wide environment features need durable references to texture assets rather than transient render handles.

Today the scene-wide environment surface includes IBL, skybox, manual exposure, and linear fog.

For maintainer guidance on where to implement new primitive types, see [docs/adding-primitive-type.md](../../docs/adding-primitive-type.md).
