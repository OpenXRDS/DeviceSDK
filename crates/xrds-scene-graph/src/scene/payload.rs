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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XrdsSceneTextureRef {
    pub texture_asset_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum XrdsSceneMaterialTextureSlotKind {
    BaseColor,
    MetallicRoughness,
    Normal,
    Occlusion,
    Emissive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    pub perceptual_roughness: f32,
    pub reflectance: f32,
    pub double_sided: bool,
    pub alpha_mode: XrdsSceneMaterialAlphaMode,
    pub alpha_cutoff: f32,
}

impl Default for XrdsSceneMaterialPbrParams {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            perceptual_roughness: 0.5,
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
            textures: XrdsSceneMaterialTextureSlots::default(),
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
            perceptual_roughness: value.perceptual_roughness,
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
            perceptual_roughness: value.perceptual_roughness,
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
            affects_lightmapped_meshes: value.affects_lightmapped_meshes,
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
    pub affects_lightmapped_meshes: bool,
}

impl Default for XrdsSceneAmbientLight {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            brightness: 1.0,
            affects_lightmapped_meshes: false,
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
