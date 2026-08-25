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
    /// A video clip, played onto a material texture slot.
    ///
    /// Accepted as `.mp4` carrying **H.264 or HEVC**, because those are what a
    /// Quest decodes in hardware. The container extension cannot prove the codec,
    /// so the document layer checks only the extension and the editor probes the
    /// stream on import — accepting a file that imports on a desktop and plays
    /// nothing on a headset is the failure this SDK keeps having to unlearn.
    Video,
}
