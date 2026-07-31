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
    InteractionZone(XrdsSceneInteractionZone),
    PlayerSpawn(XrdsScenePlayerSpawn),
    Player(XrdsScenePlayer),
    PlayerAnchor(XrdsScenePlayerAnchor),
    HudText(XrdsSceneHudText),
    Text(XrdsSceneText),
    ExtrudedText(XrdsSceneExtrudedText),
    PlayerSpawnZone(XrdsScenePlayerSpawnZone),
    WorldPanel(XrdsSceneWorldPanel),
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
            Self::InteractionZone(_) => XrdsGltfExportClass::NodeOnly,
            Self::PlayerSpawn(_) => XrdsGltfExportClass::NodeOnly,
            Self::Player(_) => XrdsGltfExportClass::NodeOnly,
            Self::PlayerAnchor(_) => XrdsGltfExportClass::NodeOnly,
            Self::HudText(_)         => XrdsGltfExportClass::NodeOnly,
            Self::Text(_)            => XrdsGltfExportClass::NodeOnly,
            Self::ExtrudedText(_)    => XrdsGltfExportClass::NodeOnly,
            Self::PlayerSpawnZone(_) => XrdsGltfExportClass::NodeOnly,
            Self::WorldPanel(_)      => XrdsGltfExportClass::NodeOnly,
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

fn default_one() -> f32 { 1.0 }
fn is_one(v: &f32) -> bool { (*v - 1.0).abs() < 1e-5 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneCube {
    pub size: [f32; 3],
    pub material: XrdsSceneMaterial,
    #[serde(default, skip_serializing_if = "XrdsPhysicsBody::is_none")]
    pub physics_body: XrdsPhysicsBody,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub gravity_scale: f32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub mass: f32,
}

impl Default for XrdsSceneCube {
    fn default() -> Self {
        Self {
            size: [1.0, 1.0, 1.0],
            material: XrdsSceneMaterial::default(),
            physics_body: XrdsPhysicsBody::None,
            gravity_scale: 1.0,
            mass: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneCylinder {
    pub radius: f32,
    pub height: f32,
    pub material: XrdsSceneMaterial,
    #[serde(default, skip_serializing_if = "XrdsPhysicsBody::is_none")]
    pub physics_body: XrdsPhysicsBody,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub gravity_scale: f32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub mass: f32,
}

impl Default for XrdsSceneCylinder {
    fn default() -> Self {
        Self {
            radius: 0.5,
            height: 1.0,
            material: XrdsSceneMaterial::default(),
            physics_body: XrdsPhysicsBody::None,
            gravity_scale: 1.0,
            mass: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneSphere {
    pub radius: f32,
    pub material: XrdsSceneMaterial,
    #[serde(default, skip_serializing_if = "XrdsPhysicsBody::is_none")]
    pub physics_body: XrdsPhysicsBody,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub gravity_scale: f32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub mass: f32,
}

impl Default for XrdsSceneSphere {
    fn default() -> Self {
        Self {
            radius: 0.5,
            material: XrdsSceneMaterial::default(),
            physics_body: XrdsPhysicsBody::None,
            gravity_scale: 1.0,
            mass: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsScenePlane3D {
    pub size: [f32; 2],
    pub material: XrdsSceneMaterial,
    #[serde(default, skip_serializing_if = "XrdsPhysicsBody::is_none")]
    pub physics_body: XrdsPhysicsBody,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub gravity_scale: f32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub mass: f32,
}

impl Default for XrdsScenePlane3D {
    fn default() -> Self {
        Self {
            size: [1.0, 1.0],
            material: XrdsSceneMaterial::default(),
            physics_body: XrdsPhysicsBody::None,
            gravity_scale: 1.0,
            mass: 1.0,
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

/// Distance rolloff model for spatial audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsAudioDistanceModel {
    /// Gain decreases linearly from `min_distance` to `max_distance`.
    Linear,
    /// Gain decreases by the inverse of distance (Web Audio default).
    #[default]
    Inverse,
    /// Gain decreases exponentially with distance.
    Exponential,
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
    // ── Spatial audio parameters (only meaningful when `spatial` is true) ──
    #[serde(default, skip_serializing_if = "XrdsAudioDistanceModel::is_default")]
    pub distance_model: XrdsAudioDistanceModel,
    #[serde(default = "default_audio_min_distance")]
    pub min_distance: f32,
    #[serde(default = "default_audio_max_distance")]
    pub max_distance: f32,
    #[serde(default = "default_audio_rolloff")]
    pub rolloff_factor: f32,
    #[serde(default)]
    pub hrtf: bool,
}

impl XrdsAudioDistanceModel {
    fn is_default(&self) -> bool {
        *self == XrdsAudioDistanceModel::Inverse
    }
}

fn default_audio_volume() -> f32 {
    1.0
}

fn default_audio_spatial() -> bool {
    true
}

fn default_audio_min_distance() -> f32 {
    1.0
}

fn default_audio_max_distance() -> f32 {
    50.0
}

fn default_audio_rolloff() -> f32 {
    1.0
}

impl Default for XrdsSceneAudioClip {
    fn default() -> Self {
        Self {
            asset_id: String::new(),
            volume: 1.0,
            looped: false,
            spatial: true,
            autoplay: false,
            distance_model: XrdsAudioDistanceModel::Inverse,
            min_distance: 1.0,
            max_distance: 50.0,
            rolloff_factor: 1.0,
            hrtf: false,
        }
    }
}

// Zone shape and grab-type come from xrds-components so that the runtime
// can use the same types without pulling in xrds-scene-graph.
pub use xrds_components::{XrdsGrabType, XrdsInteractionZoneShape};

/// An invisible volume marking an object as interactable in XR.
/// Has no visible mesh — its bounds are shown in the editor as a wireframe overlay.
///
/// `shape` alone is a valid trigger-detection volume for zone-enter/exit
/// sequencing (see `docs/xrds-scenegraph-trigger-action-sequencing.md`) —
/// `grab_type: None`, `hoverable: false` is normal for a zone meant only
/// to be walked through (a teleport pad, a damage zone), not a sign
/// something's misconfigured.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneInteractionZone {
    pub shape: XrdsInteractionZoneShape,
    pub grab_type: XrdsGrabType,
    pub hoverable: bool,
}

impl Default for XrdsSceneInteractionZone {
    fn default() -> Self {
        Self {
            shape: XrdsInteractionZoneShape::default(),
            grab_type: XrdsGrabType::None,
            hoverable: true,
        }
    }
}

/// Locomotion style for the player pawn at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsPlayerLocomotionMode {
    /// Blink/arc teleport (comfort-friendly default).
    #[default]
    Teleport,
    /// Continuous thumbstick locomotion.
    Smooth,
    /// Free-fly (no gravity, editor-style movement).
    Flying,
}

/// Authored player spawn point.  The runtime spawns the default pawn here when
/// play mode starts.  One document may contain multiple PlayerSpawn nodes; the
/// runtime uses the first one it finds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsScenePlayerSpawn {
    pub locomotion_mode: XrdsPlayerLocomotionMode,
    /// Vertical field of view in degrees for the player camera.
    /// 60° vertical ≈ 90° horizontal on 16:9 — standard comfortable desktop FOV.
    /// For XR the headset overrides this with its own optics.
    pub fov_deg: f32,
}

impl Default for XrdsScenePlayerSpawn {
    fn default() -> Self {
        Self {
            locomotion_mode: XrdsPlayerLocomotionMode::default(),
            fov_deg: 60.0,
        }
    }
}

/// World-space pawn entity.  Parent of one or more `PlayerAnchor` nodes.
///
/// `Player` is the moving root for a playable entity — its transform is driven
/// at runtime by the locomotion system.  `locomotion_mode` and `fov_deg` apply
/// when the player inhabits any child `PlayerAnchor` (unless that anchor
/// overrides them).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsScenePlayer {
    /// Display label shown in the editor hierarchy.
    pub label: String,
    /// Default locomotion mode for all child anchors.
    pub locomotion_mode: XrdsPlayerLocomotionMode,
}

impl Default for XrdsScenePlayer {
    fn default() -> Self {
        Self {
            label: "Player".to_string(),
            locomotion_mode: XrdsPlayerLocomotionMode::default(),
        }
    }
}

/// A playable-entity perspective anchor.
///
/// Children whose `Text` payload has a non-World anchor mode are authored in this
/// node's local coordinate space rather than world space.  Only one `PlayerAnchor`
/// can be active at a time; switching is an API call.
///
/// `PlayerAnchor` with `is_initial: true` replaces `PlayerSpawn` as the scene entry
/// point.  `PlayerSpawn` is kept as a legacy alias for one migration cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsScenePlayerAnchor {
    /// Display label shown in the editor hierarchy and any in-game UI.
    pub label: String,
    /// Locomotion mode when the player inhabits this anchor.
    pub locomotion_mode: XrdsPlayerLocomotionMode,
    /// Vertical FOV in degrees.  Overridden by HMD optics in XR.
    pub fov_deg: f32,
    /// If `true`, the runtime spawns the player pawn at this anchor on play-mode start.
    /// At most one `PlayerAnchor` per scene should have `is_initial: true`.
    pub is_initial: bool,
    /// Optional HUD template to instantiate head-locked for this anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hud_template_id: Option<HudTemplateId>,
    /// Per-anchor exposure override (ev100).  Overrides the scene-wide exposure while
    /// this anchor is active.  `None` = use the scene-wide exposure setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
}

impl Default for XrdsScenePlayerAnchor {
    fn default() -> Self {
        Self {
            label: "Player Anchor".to_string(),
            locomotion_mode: XrdsPlayerLocomotionMode::default(),
            fov_deg: 60.0,
            is_initial: false,
            hud_template_id: None,
            exposure: None,
        }
    }
}

/// A rectangular volume within which a player spawns at a random position.
///
/// `size` is the full width × height × depth in metres.  At runtime the SDK
/// picks a random XZ position within the footprint (Y stays at the zone centre)
/// and teleports the player there.  Multiple zones in a scene are each equally
/// likely to be chosen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsScenePlayerSpawnZone {
    /// Full box dimensions [width, height, depth] in metres.
    pub size: [f32; 3],
    /// Optional Player node ID this zone is reserved for.
    /// `None` = shared (any player may use it); `Some(id)` = exclusive to that Player node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_node_id: Option<u64>,
}

impl Default for XrdsScenePlayerSpawnZone {
    fn default() -> Self {
        Self { size: [4.0, 0.1, 4.0], player_node_id: None }
    }
}

/// Anchor corner/edge for a HUD text overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum XrdsHudAnchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Screen-space HUD text overlay node.  Rendered as a Bevy UI text widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneHudText {
    pub text: String,
    /// Font size in pixels.  Default 16.
    pub font_size: f32,
    /// RGBA colour in 0-1 range.  Default opaque white.
    pub color: [f32; 4],
    pub anchor: XrdsHudAnchor,
    /// Pixel offset from the chosen anchor corner.  Default [8, 8].
    pub offset: [f32; 2],
}

impl Default for XrdsSceneHudText {
    fn default() -> Self {
        Self {
            text: "HUD Text".to_string(),
            font_size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
            anchor: XrdsHudAnchor::TopLeft,
            offset: [8.0, 8.0],
        }
    }
}

/// Text alignment for world-space text nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum XrdsSceneTextAlignment {
    Left,
    #[default]
    Center,
    Right,
}

/// Spatial anchor mode for world-space text nodes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum XrdsSceneTextAnchor {
    /// Rotation is fixed — uses the node's authored transform rotation.
    #[default]
    World,
    /// Rotated every frame to face the active `Camera3d`. Useful for nameplates.
    Billboard,
    /// Follows all head movements (position + rotation). Full HUD anchor.
    HeadLocked,
    /// Follows body position and yaw; ignores head pitch and roll.
    BodyLocked,
    /// Like HeadLocked but Z-distance from camera is clamped to `depth_m`.
    ComfortPinned { depth_m: f32 },
    /// Text on the inside of a cylinder of `radius_m` centred on the player.
    Cylindrical { radius_m: f32 },
}

/// World-space 3D text node rendered via `bevy_rich_text3d`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneText {
    pub text: String,
    pub font_size: f32,
    /// RGBA colour in 0-1 range.  Default opaque white.
    pub color: [f32; 4],
    pub alignment: XrdsSceneTextAlignment,
    /// Spatial anchor mode.  Defaults to `World` (fixed rotation).
    /// `#[serde(default)]` ensures existing scenes without this field load correctly.
    #[serde(default)]
    pub anchor: XrdsSceneTextAnchor,
}

impl Default for XrdsSceneText {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            font_size: 24.0,
            color: [1.0, 1.0, 1.0, 1.0],
            alignment: XrdsSceneTextAlignment::Center,
            anchor: XrdsSceneTextAnchor::World,
        }
    }
}

/// World-space extruded 3D text node rendered via `bevy_fontmesh`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneExtrudedText {
    pub text: String,
    pub font_size: f32,
    /// RGBA colour in 0-1 range.  Default opaque white.
    pub color: [f32; 4],
    /// Z-axis extrusion depth in world units.
    pub depth: f32,
    pub alignment: XrdsSceneTextAlignment,
}

impl Default for XrdsSceneExtrudedText {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            font_size: 24.0,
            color: [1.0, 1.0, 1.0, 1.0],
            depth: 0.1,
            alignment: XrdsSceneTextAlignment::Center,
        }
    }
}

// ── World-space UI (Phase 5) ──────────────────────────────────────────────────

/// Serialisable layout policy for [`XrdsSceneWorldPanel`].
///
/// Mirrors [`xrds_components::XrdsWorldLayout`] but is serde-compatible for document storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsSceneWorldLayout {
    /// Manual positioning — each widget uses its `local_position` as authored.
    None,
    /// Stack top-to-bottom, horizontally centred.
    VStack { gap: f32 },
    /// Stack left-to-right, vertically centred.
    HStack { gap: f32 },
    /// Arrange in a `cols`-wide grid; `gap` is `[x_gap, y_gap]` in metres.
    Grid { cols: usize, gap: [f32; 2] },
}

impl Default for XrdsSceneWorldLayout {
    fn default() -> Self { Self::None }
}

impl XrdsSceneWorldLayout {
    pub fn is_none(&self) -> bool { matches!(self, Self::None) }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneWorldLabel {
    pub text: String,
    /// Em size in metres. 0.05 ≈ 5 cm.
    pub font_size: f32,
    /// RGBA 0–1.
    pub color: [f32; 4],
    /// Panel-local position in metres (X right, Y up from centre).
    pub local_position: [f32; 2],
    /// Slot size [width, height] metres for layout system. Default 20 cm × 6 cm.
    pub layout_size: [f32; 2],
}

impl Default for XrdsSceneWorldLabel {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 0.05,
            color: [1.0, 1.0, 1.0, 1.0],
            local_position: [0.0, 0.0],
            layout_size: [0.20, 0.06],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneWorldButton {
    pub label: String,
    pub font_size: f32,
    pub label_color: [f32; 4],
    pub size: [f32; 2],
    pub local_position: [f32; 2],
    pub normal_color: [f32; 4],
    pub hover_color: [f32; 4],
    pub pressed_color: [f32; 4],
}

impl Default for XrdsSceneWorldButton {
    fn default() -> Self {
        Self {
            label: String::new(),
            font_size: 0.04,
            label_color: [1.0, 1.0, 1.0, 1.0],
            size: [0.18, 0.06],
            local_position: [0.0, 0.0],
            normal_color:  [0.20, 0.20, 0.50, 1.0],
            hover_color:   [0.30, 0.30, 0.70, 1.0],
            pressed_color: [0.10, 0.10, 0.30, 1.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneWorldImage {
    pub asset_path: String,
    pub size: [f32; 2],
    pub local_position: [f32; 2],
    /// RGBA tint multiplier.
    pub tint: [f32; 4],
}

impl Default for XrdsSceneWorldImage {
    fn default() -> Self {
        Self {
            asset_path: String::new(),
            size: [0.10, 0.10],
            local_position: [0.0, 0.0],
            tint: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneWorldSlider {
    pub min: f32,
    pub max: f32,
    pub value: f32,
    /// Track dimensions [width, height] in metres.
    pub size: [f32; 2],
    pub local_position: [f32; 2],
    pub track_color: [f32; 4],
    pub fill_color: [f32; 4],
    pub thumb_color: [f32; 4],
    pub thumb_size: f32,
}

impl Default for XrdsSceneWorldSlider {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 1.0,
            value: 0.5,
            size: [0.24, 0.012],
            local_position: [0.0, 0.0],
            track_color: [0.25, 0.25, 0.25, 1.0],
            fill_color:  [0.30, 0.50, 0.90, 1.0],
            thumb_color: [0.90, 0.90, 0.90, 1.0],
            thumb_size: 0.022,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneWorldToggle {
    pub checked: bool,
    /// Toggle pill dimensions [width, height] in metres.
    pub size: [f32; 2],
    pub local_position: [f32; 2],
    pub track_off_color: [f32; 4],
    pub track_on_color: [f32; 4],
    pub thumb_color: [f32; 4],
}

impl Default for XrdsSceneWorldToggle {
    fn default() -> Self {
        Self {
            checked: false,
            size: [0.07, 0.04],
            local_position: [0.0, 0.0],
            track_off_color: [0.35, 0.35, 0.35, 1.0],
            track_on_color:  [0.20, 0.65, 0.30, 1.0],
            thumb_color:     [1.00, 1.00, 1.00, 1.0],
        }
    }
}

/// Child widget of a [`XrdsSceneWorldPanel`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsSceneWorldWidget {
    Label(XrdsSceneWorldLabel),
    Button(XrdsSceneWorldButton),
    Image(XrdsSceneWorldImage),
    Slider(XrdsSceneWorldSlider),
    Toggle(XrdsSceneWorldToggle),
}

fn is_zero(v: &f32) -> bool { v.abs() < 1e-9 }

/// Authored world-space UI panel with optional child widgets and layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneWorldPanel {
    /// Panel dimensions [width, height] in metres.
    pub size: [f32; 2],
    /// Background RGBA in 0–1 range.
    pub color: [f32; 4],
    /// Reserved for future rounded-corner shader; 0.0 = sharp corners.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub corner_radius: f32,
    /// Overall opacity multiplier (1.0 = fully opaque).
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// Optional auto-layout applied to child widgets.
    #[serde(default, skip_serializing_if = "XrdsSceneWorldLayout::is_none")]
    pub layout: XrdsSceneWorldLayout,
    /// Ordered list of child widgets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widgets: Vec<XrdsSceneWorldWidget>,
}

impl Default for XrdsSceneWorldPanel {
    fn default() -> Self {
        Self {
            size: [0.4, 0.3],
            color: [0.08, 0.08, 0.08, 0.92],
            corner_radius: 0.0,
            opacity: 1.0,
            layout: XrdsSceneWorldLayout::None,
            widgets: Vec::new(),
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
            ..Default::default()
        }
    }
}

