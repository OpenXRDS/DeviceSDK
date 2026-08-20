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
/// `docs/done/editor-task-queue-and-hdr-conversion.md` step 0b.
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

/// How fog thickens with distance.
///
/// # Why `visibility` and not `density`
///
/// Bevy's exponential falloffs take a `density`, which is a number with no
/// intuitive meaning — nobody knows whether 0.023 is a light haze or pea soup.
/// Bevy also ships `FogFalloff::from_visibility`, which inverts the Koschmieder
/// equation to turn a *distance at which things become indistinct* into that
/// density. An author knows "I want to see about 80 metres"; they do not know a
/// density, and would find it only by dragging a slider until it looked right.
///
/// So visibility is what is stored and authored, and the density is derived. Same
/// reasoning as the skybox storing a yaw rather than a quaternion: keep the
/// authored value the one a person actually has in mind.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum XrdsSceneFogFalloff {
    /// Clear until `start`, fully fogged at `end`.
    ///
    /// Not physical, and the easier one to art-direct precisely — it is the only
    /// mode with a distance at which fog is *exactly* absent, which is what you
    /// want when hiding a draw-distance boundary.
    Linear { start: f32, end: f32 },
    /// Exponential, which is how real haze behaves: no clear zone, thickening
    /// steadily. `visibility` is roughly the distance at which objects fade into
    /// the fog colour.
    Exponential { visibility: f32 },
    /// Exponential-squared — clearer up close and thickening faster far away, for
    /// a heavier, more sudden bank of fog.
    ExponentialSquared { visibility: f32 },
}

impl Default for XrdsSceneFogFalloff {
    fn default() -> Self {
        Self::Linear { start: 10.0, end: 100.0 }
    }
}

impl XrdsSceneFogFalloff {
    /// Clamp to values that produce fog rather than artefacts.
    ///
    /// `end <= start` inverts the ramp and `visibility <= 0` divides by zero inside
    /// Koschmieder, both of which render as garbage rather than as an error.
    pub fn sanitized(self) -> Self {
        match self {
            Self::Linear { start, end } => {
                let start = start.max(0.0);
                Self::Linear { start, end: end.max(start + 0.001) }
            }
            Self::Exponential { visibility } => Self::Exponential {
                visibility: visibility.max(0.001),
            },
            Self::ExponentialSquared { visibility } => Self::ExponentialSquared {
                visibility: visibility.max(0.001),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct XrdsSceneFogEnvironment {
    pub color: [f32; 4],
    pub falloff: XrdsSceneFogFalloff,
}

impl<'de> Deserialize<'de> for XrdsSceneFogEnvironment {
    /// Hand-written so scenes saved before falloff modes existed still load.
    ///
    /// Those documents carry `start`/`end` directly on the fog object. Deriving
    /// this would silently drop them and reset every existing scene's fog to the
    /// default — a data-loss bug that no test would notice unless it opened an old
    /// file, so the compatibility path is explicit and pinned by one.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Compat {
            Modern {
                color: [f32; 4],
                falloff: XrdsSceneFogFalloff,
            },
            Legacy {
                color: [f32; 4],
                start: f32,
                end: f32,
            },
        }

        Ok(match Compat::deserialize(deserializer)? {
            Compat::Modern { color, falloff } => Self { color, falloff },
            Compat::Legacy { color, start, end } => Self {
                color,
                falloff: XrdsSceneFogFalloff::Linear { start, end },
            },
        })
    }
}