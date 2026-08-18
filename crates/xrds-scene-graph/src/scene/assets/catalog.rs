use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneAsset {
    pub id: String,
    pub uri: String,
    pub kind: XrdsSceneAssetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum XrdsSceneAssetKind {
    Gltf,
    Texture,
    EnvironmentMap,
    Audio,
}
