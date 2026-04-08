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

For maintainer guidance on where to implement new primitive types, see [docs/adding-primitive-type.md](../../docs/adding-primitive-type.md).
