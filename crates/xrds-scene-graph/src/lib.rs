//! Authored scene document types for XRDS editor-side concepts.
//!
//! This crate is the intended source-of-truth layer for editor documents, not a second runtime
//! scene graph. The design target is: author in XRDS terms first, then export deterministically
//! to glTF.
//!
//! The important constraint is glTF-convertible, not glTF-shaped. XRDS document data therefore
//! keeps stable ids, editor-only metadata, and procedural primitive intent even when some of that
//! must later be baked or lowered during glTF export.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use xrds_components::primitives::{
    XrdsCube, XrdsCylinder, XrdsExtrudedText, XrdsExtrudedTextAlignment,
    XrdsPlane3D, XrdsSphere, XrdsTetrahedron, XrdsText, XrdsTextAlignment,
    XrdsTextAnchor,
};
use xrds_components::world::lights::{
    XrdsAmbientLight, XrdsDirectionalLight, XrdsPointLight, XrdsSpotLight,
};
use xrds_components::world::{XrdsAudioClip, XrdsCamera, XrdsGltfAsset, XrdsNode};
use xrds_components::{
    CameraProjectionParams, OrthographicCameraParams, PerspectiveCameraParams, TransformParams,
    XrdsColor, XrdsId, XrdsLinearRgba, XrdsMaterialAlphaMode, XrdsMaterialParams,
    XrdsMaterialPbrParams, XrdsMaterialTextureFilterMode, XrdsMaterialTextureRef,
    XrdsMaterialTextureSamplerParams, XrdsMaterialTextureSlots, XrdsMaterialTextureUvParams,
    XrdsMaterialTextureUvTransformMode,
    XrdsMaterialTextureWrapMode,
};
pub use xrds_components::XrdsPhysicsBody;

mod scene;
pub use scene::*;

mod document;
pub use document::*;

mod session;
pub use session::*;

#[cfg(test)]
mod tests;
