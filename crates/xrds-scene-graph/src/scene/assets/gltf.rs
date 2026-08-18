use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsSceneGltfNodeAuthoring {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_playback: Option<XrdsSceneGltfPlayback>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morph_target_overrides: Vec<XrdsSceneGltfMorphTargetOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneGltfAnimationSelector {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneAnimationRepeatMode {
    Once,
    Loop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneGltfPlayback {
    pub selector: XrdsSceneGltfAnimationSelector,
    pub repeat: XrdsSceneAnimationRepeatMode,
    pub speed: f32,
    pub start_paused: bool,
}

impl Default for XrdsSceneGltfPlayback {
    fn default() -> Self {
        Self {
            selector: XrdsSceneGltfAnimationSelector::Index(0),
            repeat: XrdsSceneAnimationRepeatMode::Loop,
            speed: 1.0,
            start_paused: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct XrdsSceneGltfNodeLocator {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_index_path: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneGltfMorphTargetSelector {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneGltfMorphTargetWeight {
    pub selector: XrdsSceneGltfMorphTargetSelector,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsSceneGltfMorphTargetOverride {
    #[serde(default)]
    pub node: XrdsSceneGltfNodeLocator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weights: Vec<XrdsSceneGltfMorphTargetWeight>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XrdsSceneGltfAsset {
    pub asset_id: Option<String>,
    pub asset_uri: String,
    pub scene_index: usize,
    pub export_policy: XrdsGltfAssetExportPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsGltfAssetExportPolicy {
    KeepExternalReference,
    InlineOnExport,
}

impl From<&XrdsGltfAsset> for XrdsSceneGltfAsset {
    fn from(value: &XrdsGltfAsset) -> Self {
        Self {
            asset_id: None,
            asset_uri: value.gltf_asset_path.clone(),
            scene_index: value.scene_index,
            export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
        }
    }
}