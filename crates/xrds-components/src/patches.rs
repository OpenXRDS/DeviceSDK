//! Update payloads for [`XrdsAPI::queue_update`].
//!
//! These were `*Patch` until 2026-08-26. "Patch" is ECS vocabulary, and this is the
//! surface aimed at people who have deliberately not been asked to learn ECS — the
//! same reasoning that keeps Bevy out of the default layer. They pair with
//! `queue_update` and `register_updater`, so `queue_update(&h, NameUpdate { .. })`
//! now reads as one idea rather than two.
//!
//! [`GltfSource`] is the exception and keeps a plain noun: it is *read* as well as
//! written (`XrdsAPI::gltf_source`), and a getter returning something called an
//! "update" would be its own small confusion.
//!
//! The old names remain as deprecated aliases, so this is a warning rather than a
//! break.

use crate::{CameraProjectionParams, XrdsId};
use crate::primitives::{XrdsExtrudedTextAlignment, XrdsTextAlignment};

/// Sets a node's parent.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParentUpdate {
    pub parent_id: Option<XrdsId>,
}

/// Renames a node.
#[derive(Debug, Clone)]
pub struct NameUpdate {
    pub name: String,
}

/// Shows or hides a node.
#[derive(Debug, Clone, Copy, Default)]
pub struct VisibilityUpdate {
    pub visible: bool,
}

/// Points a camera at a target, or clears the target.
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraLookAtUpdate {
    pub look_at: Option<[f32; 3]>,
}

/// Which glTF file a node shows, and which scene within it.
///
/// Read as well as written — see the module note.
#[derive(Debug, Clone)]
pub struct GltfSource {
    pub gltf_asset_path: String,
    pub scene_index: usize,
}

/// Replaces a camera's projection.
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraProjectionUpdate {
    pub projection: CameraProjectionParams,
}

/// 3-D text content and styling.
#[derive(Debug, Clone)]
pub struct TextParams {
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
    pub alignment: XrdsTextAlignment,
}

/// Extruded 3-D text content and styling.
#[derive(Debug, Clone)]
pub struct ExtrudedTextParams {
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
    pub depth: f32,
    pub alignment: XrdsExtrudedTextAlignment,
}

// -- Deprecated aliases ------------------------------------------------------
//
// Renamed 2026-08-26 (see the module note). Kept so the rename is a warning
// rather than a break; remove at the next breaking release.

#[deprecated(since = "0.1.0", note = "renamed to `ParentUpdate`")]
pub type ParentPatch = ParentUpdate;
#[deprecated(since = "0.1.0", note = "renamed to `NameUpdate`")]
pub type NamePatch = NameUpdate;
#[deprecated(since = "0.1.0", note = "renamed to `VisibilityUpdate`")]
pub type VisibilityPatch = VisibilityUpdate;
#[deprecated(since = "0.1.0", note = "renamed to `CameraLookAtUpdate`")]
pub type CameraLookAtPatch = CameraLookAtUpdate;
#[deprecated(since = "0.1.0", note = "renamed to `GltfSource`")]
pub type GltfAssetSourcePatch = GltfSource;
#[deprecated(since = "0.1.0", note = "renamed to `CameraProjectionUpdate`")]
pub type CameraProjectionPatch = CameraProjectionUpdate;

#[cfg(test)]
mod tests {
    use super::*;

    /// The old names still resolve, so the rename is a warning and not a break.
    ///
    /// Asserted rather than assumed: a deprecated alias that fails to compile is a
    /// breaking change wearing a deprecation notice, and nothing else in the tree
    /// uses the old names any more — so without this, nothing would notice.
    #[test]
    #[allow(deprecated)]
    fn the_old_patch_names_still_resolve() {
        let _: ParentPatch = ParentUpdate { parent_id: None };
        let _: NamePatch = NameUpdate { name: "x".into() };
        let _: VisibilityPatch = VisibilityUpdate { visible: true };
        let _: CameraLookAtPatch = CameraLookAtUpdate { look_at: None };
        let _: CameraProjectionPatch = CameraProjectionUpdate {
            projection: CameraProjectionParams::default(),
        };
        let _: GltfAssetSourcePatch = GltfSource {
            gltf_asset_path: "a.glb".into(),
            scene_index: 0,
        };
    }
}
