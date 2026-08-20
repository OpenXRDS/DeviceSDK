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
    /// Rotation about the vertical axis, in degrees.
    ///
    /// **Why yaw and not a quaternion**, when `Skybox::rotation` is a `Quat`: the
    /// authoring question is "where is the sun", and that is a yaw. A cubemap
    /// arrives in whatever orientation it was captured, so turning it to place the
    /// sun — or to line a horizon feature up with the scene — is the one adjustment
    /// an author actually makes.
    ///
    /// Bevy's own doc for the field describes the other use, correcting for a Z-up
    /// source. That is a property of the file rather than of the scene, and belongs
    /// wherever the cubemap is produced; storing a full quaternion per scene to
    /// express it would make the common case unauthorable to avoid a conversion-time
    /// fix.
    #[serde(default)]
    pub yaw_deg: f32,
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