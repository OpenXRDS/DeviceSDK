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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<XrdsSceneAtmosphereEnvironment>,
}

/// Procedural atmospheric scattering — a computed sky rather than an image.
///
/// Wraps Bevy's `Atmosphere` (Hillaire 2020). Unlike a skybox this is *lit by the
/// scene*: the sun's position and colour come from the directional lights already
/// present, so the sky and the shadows agree, and moving a light moves the sun. A
/// captured panorama cannot do that.
///
/// **Carries no physical parameters yet, deliberately.** Bevy's component exposes
/// planet radius, Rayleigh and Mie densities, ozone bands and more — perhaps
/// fifteen numbers describing an atmosphere. Exposing them before anyone has asked
/// would be authoring a physics UI on speculation; Earth's defaults are what almost
/// every scene wants, and adding fields later is additive. See
/// `docs/editor-task-queue-and-hdr-conversion.md` step 0b.
///
/// Two consequences worth knowing:
///
/// - **It needs a directional light.** With none there is no sun, and the sky
///   renders as an unlit shell.
/// - **It forces an HDR camera** (`Atmosphere` requires Bevy's `Hdr`), which adds a
///   float intermediate render target. That is a real cost on a mobile GPU and the
///   reason this shipped as a spike to be measured on device rather than assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct XrdsSceneAtmosphereEnvironment {}

impl XrdsSceneEnvironment {
    pub fn is_empty(&self) -> bool {
        self.ibl.is_none()
            && self.skybox.is_none()
            && self.exposure.is_none()
            && self.fog.is_none()
            && self.atmosphere.is_none()
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