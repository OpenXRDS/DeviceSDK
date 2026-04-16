use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsSceneNodePayload {
    Empty,
    Camera(XrdsSceneCamera),
    GltfAsset(XrdsSceneGltfAsset),
    Cube(XrdsSceneCube),
    Cylinder(XrdsSceneCylinder),
    Sphere(XrdsSceneSphere),
    Plane3D(XrdsScenePlane3D),
    Tetrahedron(XrdsSceneTetrahedron),
    AmbientLight(XrdsSceneAmbientLight),
    DirectionalLight(XrdsSceneDirectionalLight),
    PointLight(XrdsScenePointLight),
    SpotLight(XrdsSceneSpotLight),
    AudioClip(XrdsSceneAudioClip),
}

impl XrdsSceneNodePayload {
    pub fn gltf_export_class(&self) -> XrdsGltfExportClass {
        match self {
            Self::Empty => XrdsGltfExportClass::NodeOnly,
            Self::Camera(_) => XrdsGltfExportClass::Camera,
            Self::GltfAsset(_) => XrdsGltfExportClass::ExternalSceneReference,
            Self::Cube(_)
            | Self::Cylinder(_)
            | Self::Sphere(_)
            | Self::Plane3D(_)
            | Self::Tetrahedron(_) => XrdsGltfExportClass::ProceduralMeshBake,
            Self::AmbientLight(_)
            | Self::DirectionalLight(_)
            | Self::PointLight(_)
            | Self::SpotLight(_) => XrdsGltfExportClass::Light,
            Self::AudioClip(_) => XrdsGltfExportClass::NodeOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsGltfExportClass {
    NodeOnly,
    Camera,
    Light,
    ExternalSceneReference,
    ProceduralMeshBake,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum XrdsSceneCameraProjection {
    Perspective {
        fov_deg: f32,
        near: f32,
        far: Option<f32>,
        order: isize,
    },
    Orthographic {
        scale: f32,
        near: f32,
        far: f32,
        order: isize,
    },
}

impl Default for XrdsSceneCameraProjection {
    fn default() -> Self {
        Self::Perspective {
            fov_deg: 60.0,
            near: 0.1,
            far: Some(1000.0),
            order: 0,
        }
    }
}

impl From<CameraProjectionParams> for XrdsSceneCameraProjection {
    fn from(value: CameraProjectionParams) -> Self {
        match value {
            CameraProjectionParams::Perspective(params) => Self::Perspective {
                fov_deg: params.fov_deg,
                near: params.near,
                far: params.far,
                order: params.order,
            },
            CameraProjectionParams::Orthographic(params) => Self::Orthographic {
                scale: params.scale,
                near: params.near,
                far: params.far,
                order: params.order,
            },
        }
    }
}

impl From<XrdsSceneCameraProjection> for CameraProjectionParams {
    fn from(value: XrdsSceneCameraProjection) -> Self {
        match value {
            XrdsSceneCameraProjection::Perspective {
                fov_deg,
                near,
                far,
                order,
            } => Self::Perspective(PerspectiveCameraParams {
                fov_deg,
                near,
                far,
                order,
            }),
            XrdsSceneCameraProjection::Orthographic {
                scale,
                near,
                far,
                order,
            } => Self::Orthographic(OrthographicCameraParams {
                scale,
                near,
                far,
                order,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneCamera {
    pub projection: XrdsSceneCameraProjection,
    pub look_at: Option<[f32; 3]>,
}

impl Default for XrdsSceneCamera {
    fn default() -> Self {
        Self {
            projection: XrdsSceneCameraProjection::default(),
            look_at: None,
        }
    }
}

impl From<&XrdsCamera> for XrdsSceneCamera {
    fn from(value: &XrdsCamera) -> Self {
        Self {
            projection: value.projection.into(),
            look_at: value.look_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneMaterial {
    pub base_color: [f32; 4],
    pub emissive: [f32; 4],
    pub opacity: f32,
    pub unlit: bool,
    #[serde(default)]
    pub pbr: XrdsSceneMaterialPbrParams,
    #[serde(
        default,
        skip_serializing_if = "XrdsSceneMaterialTextureSlots::is_empty"
    )]
    pub textures: XrdsSceneMaterialTextureSlots,
}

impl Default for XrdsSceneMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            emissive: [0.0, 0.0, 0.0, 1.0],
            opacity: 1.0,
            unlit: false,
            pbr: XrdsSceneMaterialPbrParams::default(),
            textures: XrdsSceneMaterialTextureSlots::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneTextureRef {
    pub texture_asset_id: String,
    #[serde(default, skip_serializing_if = "XrdsSceneTextureUvParams::is_default")]
    pub uv: XrdsSceneTextureUvParams,
    #[serde(default, skip_serializing_if = "XrdsSceneTextureSamplerParams::is_default")]
    pub sampler: XrdsSceneTextureSamplerParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneTextureUvParams {
    pub set: u32,
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation_deg: f32,
    #[serde(default, skip_serializing_if = "XrdsSceneTextureUvTransformMode::is_default")]
    pub transform_mode: XrdsSceneTextureUvTransformMode,
}

impl Default for XrdsSceneTextureUvParams {
    fn default() -> Self {
        Self {
            set: 0,
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation_deg: 0.0,
            transform_mode: XrdsSceneTextureUvTransformMode::Centered,
        }
    }
}

impl XrdsSceneTextureUvParams {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsSceneTextureUvTransformMode {
    #[default]
    Centered,
    Raw,
}

impl XrdsSceneTextureUvTransformMode {
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Centered)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct XrdsSceneTextureSamplerParams {
    pub wrap_u: XrdsSceneTextureWrapMode,
    pub wrap_v: XrdsSceneTextureWrapMode,
    pub min_filter: XrdsSceneTextureFilterMode,
    pub mag_filter: XrdsSceneTextureFilterMode,
    pub mipmap_filter: XrdsSceneTextureFilterMode,
}

impl Default for XrdsSceneTextureSamplerParams {
    fn default() -> Self {
        Self {
            wrap_u: XrdsSceneTextureWrapMode::Repeat,
            wrap_v: XrdsSceneTextureWrapMode::Repeat,
            min_filter: XrdsSceneTextureFilterMode::Linear,
            mag_filter: XrdsSceneTextureFilterMode::Linear,
            mipmap_filter: XrdsSceneTextureFilterMode::Linear,
        }
    }
}

impl XrdsSceneTextureSamplerParams {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneTextureWrapMode {
    Repeat,
    MirroredRepeat,
    ClampToEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneTextureFilterMode {
    Linear,
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum XrdsSceneMaterialTextureSlotKind {
    BaseColor,
    MetallicRoughness,
    Normal,
    Occlusion,
    Emissive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsSceneMaterialTextureSlots {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color: Option<XrdsSceneTextureRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_roughness: Option<XrdsSceneTextureRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<XrdsSceneTextureRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occlusion: Option<XrdsSceneTextureRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive: Option<XrdsSceneTextureRef>,
}

impl XrdsSceneMaterialTextureSlots {
    pub fn is_empty(&self) -> bool {
        self.base_color.is_none()
            && self.metallic_roughness.is_none()
            && self.normal.is_none()
            && self.occlusion.is_none()
            && self.emissive.is_none()
    }

    pub fn get(&self, slot: XrdsSceneMaterialTextureSlotKind) -> Option<&XrdsSceneTextureRef> {
        match slot {
            XrdsSceneMaterialTextureSlotKind::BaseColor => self.base_color.as_ref(),
            XrdsSceneMaterialTextureSlotKind::MetallicRoughness => self.metallic_roughness.as_ref(),
            XrdsSceneMaterialTextureSlotKind::Normal => self.normal.as_ref(),
            XrdsSceneMaterialTextureSlotKind::Occlusion => self.occlusion.as_ref(),
            XrdsSceneMaterialTextureSlotKind::Emissive => self.emissive.as_ref(),
        }
    }

    pub fn set(
        &mut self,
        slot: XrdsSceneMaterialTextureSlotKind,
        texture: Option<XrdsSceneTextureRef>,
    ) {
        match slot {
            XrdsSceneMaterialTextureSlotKind::BaseColor => self.base_color = texture,
            XrdsSceneMaterialTextureSlotKind::MetallicRoughness => {
                self.metallic_roughness = texture
            }
            XrdsSceneMaterialTextureSlotKind::Normal => self.normal = texture,
            XrdsSceneMaterialTextureSlotKind::Occlusion => self.occlusion = texture,
            XrdsSceneMaterialTextureSlotKind::Emissive => self.emissive = texture,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneMaterialAlphaMode {
    Auto,
    Opaque,
    Mask,
    Blend,
}

impl Default for XrdsSceneMaterialAlphaMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneMaterialPbrParams {
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
    pub double_sided: bool,
    pub alpha_mode: XrdsSceneMaterialAlphaMode,
    pub alpha_cutoff: f32,
}

impl Default for XrdsSceneMaterialPbrParams {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            roughness: 0.5,
            reflectance: 0.5,
            double_sided: false,
            alpha_mode: XrdsSceneMaterialAlphaMode::Auto,
            alpha_cutoff: 0.5,
        }
    }
}

impl From<XrdsMaterialParams> for XrdsSceneMaterial {
    fn from(value: XrdsMaterialParams) -> Self {
        Self {
            base_color: value.base_color.rgba,
            emissive: value.emissive.rgba,
            opacity: value.opacity,
            unlit: value.unlit,
            pbr: value.pbr.into(),
            textures: value.textures.into(),
        }
    }
}

impl From<XrdsSceneMaterial> for XrdsMaterialParams {
    fn from(value: XrdsSceneMaterial) -> Self {
        Self {
            base_color: XrdsColor {
                rgba: value.base_color,
            },
            emissive: XrdsLinearRgba {
                rgba: value.emissive,
            },
            opacity: value.opacity,
            unlit: value.unlit,
            pbr: value.pbr.into(),
            textures: value.textures.into(),
        }
    }
}

impl From<XrdsMaterialTextureSlots> for XrdsSceneMaterialTextureSlots {
    fn from(value: XrdsMaterialTextureSlots) -> Self {
        Self {
            base_color: value.base_color.map(Into::into),
            metallic_roughness: value.metallic_roughness.map(Into::into),
            normal: value.normal.map(Into::into),
            occlusion: value.occlusion.map(Into::into),
            emissive: value.emissive.map(Into::into),
        }
    }
}

impl From<XrdsSceneMaterialTextureSlots> for XrdsMaterialTextureSlots {
    fn from(value: XrdsSceneMaterialTextureSlots) -> Self {
        Self {
            base_color: value.base_color.map(Into::into),
            metallic_roughness: value.metallic_roughness.map(Into::into),
            normal: value.normal.map(Into::into),
            occlusion: value.occlusion.map(Into::into),
            emissive: value.emissive.map(Into::into),
        }
    }
}

impl From<XrdsMaterialTextureRef> for XrdsSceneTextureRef {
    fn from(value: XrdsMaterialTextureRef) -> Self {
        Self {
            texture_asset_id: value.texture_asset_id,
            uv: value.uv.into(),
            sampler: value.sampler.into(),
        }
    }
}

impl From<XrdsSceneTextureRef> for XrdsMaterialTextureRef {
    fn from(value: XrdsSceneTextureRef) -> Self {
        Self {
            texture_asset_id: value.texture_asset_id,
            uv: value.uv.into(),
            sampler: value.sampler.into(),
        }
    }
}

impl From<XrdsMaterialTextureUvParams> for XrdsSceneTextureUvParams {
    fn from(value: XrdsMaterialTextureUvParams) -> Self {
        Self {
            set: value.set,
            offset: value.offset,
            scale: value.scale,
            rotation_deg: value.rotation_deg,
            transform_mode: value.transform_mode.into(),
        }
    }
}

impl From<XrdsSceneTextureUvParams> for XrdsMaterialTextureUvParams {
    fn from(value: XrdsSceneTextureUvParams) -> Self {
        Self {
            set: value.set,
            offset: value.offset,
            scale: value.scale,
            rotation_deg: value.rotation_deg,
            transform_mode: value.transform_mode.into(),
        }
    }
}

impl From<XrdsMaterialTextureUvTransformMode> for XrdsSceneTextureUvTransformMode {
    fn from(value: XrdsMaterialTextureUvTransformMode) -> Self {
        match value {
            XrdsMaterialTextureUvTransformMode::Centered => Self::Centered,
            XrdsMaterialTextureUvTransformMode::Raw => Self::Raw,
        }
    }
}

impl From<XrdsSceneTextureUvTransformMode> for XrdsMaterialTextureUvTransformMode {
    fn from(value: XrdsSceneTextureUvTransformMode) -> Self {
        match value {
            XrdsSceneTextureUvTransformMode::Centered => Self::Centered,
            XrdsSceneTextureUvTransformMode::Raw => Self::Raw,
        }
    }
}

impl From<XrdsMaterialTextureSamplerParams> for XrdsSceneTextureSamplerParams {
    fn from(value: XrdsMaterialTextureSamplerParams) -> Self {
        Self {
            wrap_u: value.wrap_u.into(),
            wrap_v: value.wrap_v.into(),
            min_filter: value.min_filter.into(),
            mag_filter: value.mag_filter.into(),
            mipmap_filter: value.mipmap_filter.into(),
        }
    }
}

impl From<XrdsSceneTextureSamplerParams> for XrdsMaterialTextureSamplerParams {
    fn from(value: XrdsSceneTextureSamplerParams) -> Self {
        Self {
            wrap_u: value.wrap_u.into(),
            wrap_v: value.wrap_v.into(),
            min_filter: value.min_filter.into(),
            mag_filter: value.mag_filter.into(),
            mipmap_filter: value.mipmap_filter.into(),
        }
    }
}

impl From<XrdsMaterialTextureWrapMode> for XrdsSceneTextureWrapMode {
    fn from(value: XrdsMaterialTextureWrapMode) -> Self {
        match value {
            XrdsMaterialTextureWrapMode::Repeat => Self::Repeat,
            XrdsMaterialTextureWrapMode::MirroredRepeat => Self::MirroredRepeat,
            XrdsMaterialTextureWrapMode::ClampToEdge => Self::ClampToEdge,
        }
    }
}

impl From<XrdsSceneTextureWrapMode> for XrdsMaterialTextureWrapMode {
    fn from(value: XrdsSceneTextureWrapMode) -> Self {
        match value {
            XrdsSceneTextureWrapMode::Repeat => Self::Repeat,
            XrdsSceneTextureWrapMode::MirroredRepeat => Self::MirroredRepeat,
            XrdsSceneTextureWrapMode::ClampToEdge => Self::ClampToEdge,
        }
    }
}

impl From<XrdsMaterialTextureFilterMode> for XrdsSceneTextureFilterMode {
    fn from(value: XrdsMaterialTextureFilterMode) -> Self {
        match value {
            XrdsMaterialTextureFilterMode::Linear => Self::Linear,
            XrdsMaterialTextureFilterMode::Nearest => Self::Nearest,
        }
    }
}

impl From<XrdsSceneTextureFilterMode> for XrdsMaterialTextureFilterMode {
    fn from(value: XrdsSceneTextureFilterMode) -> Self {
        match value {
            XrdsSceneTextureFilterMode::Linear => Self::Linear,
            XrdsSceneTextureFilterMode::Nearest => Self::Nearest,
        }
    }
}

impl From<XrdsMaterialAlphaMode> for XrdsSceneMaterialAlphaMode {
    fn from(value: XrdsMaterialAlphaMode) -> Self {
        match value {
            XrdsMaterialAlphaMode::Auto => Self::Auto,
            XrdsMaterialAlphaMode::Opaque => Self::Opaque,
            XrdsMaterialAlphaMode::Mask => Self::Mask,
            XrdsMaterialAlphaMode::Blend => Self::Blend,
        }
    }
}

impl From<XrdsSceneMaterialAlphaMode> for XrdsMaterialAlphaMode {
    fn from(value: XrdsSceneMaterialAlphaMode) -> Self {
        match value {
            XrdsSceneMaterialAlphaMode::Auto => Self::Auto,
            XrdsSceneMaterialAlphaMode::Opaque => Self::Opaque,
            XrdsSceneMaterialAlphaMode::Mask => Self::Mask,
            XrdsSceneMaterialAlphaMode::Blend => Self::Blend,
        }
    }
}

impl From<XrdsMaterialPbrParams> for XrdsSceneMaterialPbrParams {
    fn from(value: XrdsMaterialPbrParams) -> Self {
        Self {
            metallic: value.metallic,
            roughness: value.roughness,
            reflectance: value.reflectance,
            double_sided: value.double_sided,
            alpha_mode: value.alpha_mode.into(),
            alpha_cutoff: value.alpha_cutoff,
        }
    }
}

impl From<XrdsSceneMaterialPbrParams> for XrdsMaterialPbrParams {
    fn from(value: XrdsSceneMaterialPbrParams) -> Self {
        Self {
            metallic: value.metallic,
            roughness: value.roughness,
            reflectance: value.reflectance,
            double_sided: value.double_sided,
            alpha_mode: value.alpha_mode.into(),
            alpha_cutoff: value.alpha_cutoff,
        }
    }
}

impl From<&XrdsAmbientLight> for XrdsSceneAmbientLight {
    fn from(value: &XrdsAmbientLight) -> Self {
        Self {
            color: value.color.rgba,
            brightness: value.brightness,
            affects_baked_lighting: value.affects_baked_lighting,
        }
    }
}

impl From<&XrdsDirectionalLight> for XrdsSceneDirectionalLight {
    fn from(value: &XrdsDirectionalLight) -> Self {
        Self {
            color: value.color.rgba,
            illuminance: value.illuminance,
            shadows: value.shadows,
        }
    }
}

impl From<&XrdsPointLight> for XrdsScenePointLight {
    fn from(value: &XrdsPointLight) -> Self {
        Self {
            color: value.color.rgba,
            intensity: value.intensity,
            range: value.range,
            radius: value.radius,
            shadows: value.shadows,
        }
    }
}

impl From<&XrdsSpotLight> for XrdsSceneSpotLight {
    fn from(value: &XrdsSpotLight) -> Self {
        Self {
            color: value.color.rgba,
            intensity: value.intensity,
            range: value.range,
            inner_angle: value.inner_angle,
            outer_angle: value.outer_angle,
            shadows: value.shadows,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneCube {
    pub size: [f32; 3],
    pub material: XrdsSceneMaterial,
}

impl Default for XrdsSceneCube {
    fn default() -> Self {
        Self {
            size: [1.0, 1.0, 1.0],
            material: XrdsSceneMaterial::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneCylinder {
    pub radius: f32,
    pub height: f32,
    pub material: XrdsSceneMaterial,
}

impl Default for XrdsSceneCylinder {
    fn default() -> Self {
        Self {
            radius: 0.5,
            height: 1.0,
            material: XrdsSceneMaterial::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneSphere {
    pub radius: f32,
    pub material: XrdsSceneMaterial,
}

impl Default for XrdsSceneSphere {
    fn default() -> Self {
        Self {
            radius: 0.5,
            material: XrdsSceneMaterial::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsScenePlane3D {
    pub size: [f32; 2],
    pub material: XrdsSceneMaterial,
}

impl Default for XrdsScenePlane3D {
    fn default() -> Self {
        Self {
            size: [1.0, 1.0],
            material: XrdsSceneMaterial::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneTetrahedron {
    pub vertices: [[f32; 3]; 4],
    pub material: XrdsSceneMaterial,
}

impl Default for XrdsSceneTetrahedron {
    fn default() -> Self {
        Self {
            vertices: [
                [0.0, 0.577_350_26, 0.0],
                [-0.5, -0.288_675_13, 0.5],
                [0.5, -0.288_675_13, 0.5],
                [0.0, -0.288_675_13, -0.5],
            ],
            material: XrdsSceneMaterial::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneAmbientLight {
    pub color: [f32; 4],
    pub brightness: f32,
    pub affects_baked_lighting: bool,
}

impl Default for XrdsSceneAmbientLight {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            brightness: 1.0,
            affects_baked_lighting: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneDirectionalLight {
    pub color: [f32; 4],
    pub illuminance: f32,
    pub shadows: bool,
}

impl Default for XrdsSceneDirectionalLight {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            illuminance: 1000.0,
            shadows: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsScenePointLight {
    pub color: [f32; 4],
    pub intensity: f32,
    pub range: f32,
    pub radius: f32,
    pub shadows: bool,
}

impl Default for XrdsScenePointLight {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            intensity: 1000.0,
            range: 10.0,
            radius: 0.0,
            shadows: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneSpotLight {
    pub color: [f32; 4],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub shadows: bool,
}

impl Default for XrdsSceneSpotLight {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            intensity: 1000.0,
            range: 10.0,
            inner_angle: 0.0,
            outer_angle: std::f32::consts::FRAC_PI_4,
            shadows: false,
        }
    }
}

/// Authored audio clip node referencing a catalog `Audio` asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneAudioClip {
    /// Catalog asset id. Must reference an `XrdsSceneAssetKind::Audio` asset.
    pub asset_id: String,
    #[serde(default = "default_audio_volume")]
    pub volume: f32,
    #[serde(default)]
    pub looped: bool,
    /// `true` = 3-D positional audio, `false` = scene-wide ambient.
    #[serde(default = "default_audio_spatial")]
    pub spatial: bool,
    #[serde(default)]
    pub autoplay: bool,
}

fn default_audio_volume() -> f32 {
    1.0
}

fn default_audio_spatial() -> bool {
    true
}

impl Default for XrdsSceneAudioClip {
    fn default() -> Self {
        Self {
            asset_id: String::new(),
            volume: 1.0,
            looped: false,
            spatial: true,
            autoplay: false,
        }
    }
}

impl From<&XrdsAudioClip> for XrdsSceneAudioClip {
    fn from(value: &XrdsAudioClip) -> Self {
        Self {
            asset_id: value.audio_asset_id.clone(),
            volume: value.volume,
            looped: value.looped,
            spatial: value.spatial,
            autoplay: value.autoplay,
        }
    }
}
