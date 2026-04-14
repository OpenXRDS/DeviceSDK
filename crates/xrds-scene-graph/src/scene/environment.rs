use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsSceneEnvironment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ibl: Option<XrdsSceneIblEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skybox: Option<XrdsSceneSkyboxEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<XrdsSceneExposureEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog: Option<XrdsSceneFogEnvironment>,
}

impl XrdsSceneEnvironment {
    pub fn is_empty(&self) -> bool {
        self.ibl.is_none()
            && self.skybox.is_none()
            && self.exposure.is_none()
            && self.fog.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneIblEnvironment {
    pub diffuse_asset_id: String,
    pub specular_asset_id: String,
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneIblAssetSlot {
    Diffuse,
    Specular,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneSkyboxEnvironment {
    pub texture_asset_id: String,
    pub brightness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneSkyboxAssetSlot {
    Texture,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneExposureEnvironment {
    pub ev100: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneFogEnvironment {
    pub color: [f32; 4],
    pub start: f32,
    pub end: f32,
}