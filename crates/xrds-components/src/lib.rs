//! Shared XRDS descriptor types, used by both the runtime and the SDK surface.
//!
//! These are the "what to build" half of the SDK: `XrdsCube`, `XrdsCamera`,
//! `XrdsPointLight` and their parameter structs. They carry no Bevy components and
//! no world access — [`xrds-runtime`](https://docs.rs/xrds-runtime) turns them into
//! entities.
//!
//! Separated from the runtime so a descriptor can be constructed, stored and passed
//! around without a live world — which is what lets the same type be spawned
//! immediately, saved to a document, or handed to an editor.
//!
//! Most types here have an `XrdsScene`-prefixed counterpart in `xrds-scene-graph`.
//! These are the runtime-facing half: use them for live objects, and the scene half
//! for data that must survive save and load.

mod camera_params;
mod interaction;
mod color;
mod core;
mod light_params;
mod patches;
pub mod primitives;
mod values;
pub mod world;
pub mod world_ui;

pub use camera_params::{
    CameraKind, CameraProjectionParams, OrthographicCameraParams, PerspectiveCameraParams,
    XrdsBloom, XrdsClearColorConfig, XrdsTonemapping,
};
pub use color::{XrdsColor, XrdsLinearRgba};
pub use interaction::{
    XrDropEvent, XrGrabEvent, XrGrabHand, XrGrabHandle, XrGrabHandleOnly, XrGrabbable,
    XrGrabbed, XrRayhit,
    XrdsGrabType, XrdsInteractionZone, XrdsInteractionZoneShape, XrdsPhysicsBody,
    XrdsPlayerSpawnZone, XrZoneEnterEvent, XrZoneExitEvent,
};
pub use core::{
    default_component_name, XrdsActor, XrdsAssetComponent, XrdsComponent, XrdsComponentsPlugin,
    XrdsId, XrdsMutableComponent, XrdsObject, XrdsRegistry,
};
pub use light_params::{
    AmbientLightParams, DirectionalLightParams, LightKind, PointLightParams, SpotLightParams,
};
pub use world::audio::{XrdsAudioClip, XrdsAudioDistanceModel};
pub use world_ui::{
    XrdsWorldButton, XrdsWorldButtonParams, XrdsWorldButtonState,
    XrdsWorldElementDisabled,
    XrdsWorldImage, XrdsWorldImageParams,
    XrdsWorldLabel, XrdsWorldLabelParams,
    XrdsWorldLayout,
    XrdsWorldPanel, XrdsWorldPointerCursors, XrdsWorldPointerHit, XrdsWorldPointerState,
    XrdsWorldSlider, XrdsWorldSliderParams,
    XrdsWorldSurface, XrdsWorldToggle, XrdsWorldToggleParams,
    XrWorldButtonPressEvent, XrWorldButtonReleaseEvent,
    XrWorldHoverEnterEvent, XrWorldHoverExitEvent,
    XrWorldSliderChangeEvent, XrWorldToggleEvent,
};
pub use patches::{
    CameraLookAtPatch, CameraProjectionPatch, ExtrudedTextParams, GltfAssetSourcePatch, NamePatch,
    ParentPatch, TextParams, VisibilityPatch,
};
pub use values::{
    CapsuleGeometryParams, CubeGeometryParams, CylinderGeometryParams, EffectParams,
    Plane3DGeometryParams,
    SphereGeometryParams, TetrahedronGeometryParams, TransformParams, XrdsMaterialAlphaMode,
    XrdsMaterialParams,
    XrdsMaterialPbrParams, XrdsMaterialTextureFilterMode, XrdsMaterialTextureRef,
    XrdsMaterialTextureSamplerParams, XrdsMaterialTextureSlotKind, XrdsMaterialTextureSlots,
    XrdsMaterialTextureUvParams, XrdsMaterialTextureUvTransformMode, XrdsMaterialTextureWrapMode,
};
