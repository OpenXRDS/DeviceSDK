use crate::{CameraProjectionParams, XrdsId};

/// Runtime patch payload for scene-graph parenting.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParentPatch {
    pub parent_id: Option<XrdsId>,
}

/// Runtime patch payload for object renaming.
#[derive(Debug, Clone)]
pub struct NamePatch {
    pub name: String,
}

/// Runtime patch payload for runtime visibility toggles.
#[derive(Debug, Clone, Copy, Default)]
pub struct VisibilityPatch {
    pub visible: bool,
}

/// Runtime patch payload for authored camera look-at targets.
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraLookAtPatch {
    pub look_at: Option<[f32; 3]>,
}

/// Runtime patch payload for glTF source and scene selection.
#[derive(Debug, Clone)]
pub struct GltfAssetSourcePatch {
    pub gltf_asset_path: String,
    pub scene_index: usize,
}

/// Runtime patch payload for camera projection updates.
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraProjectionPatch {
    pub projection: CameraProjectionParams,
}
