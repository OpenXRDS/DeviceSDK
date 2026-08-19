use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsSceneNodePayload {
    Empty,
    Camera(XrdsSceneCamera),
    GltfAsset(XrdsSceneGltfAsset),
    Cube(XrdsSceneCube),
    Cylinder(XrdsSceneCylinder),
    Capsule(XrdsSceneCapsule),
    Sphere(XrdsSceneSphere),
    Plane3D(XrdsScenePlane3D),
    Tetrahedron(XrdsSceneTetrahedron),
    Effect(XrdsSceneEffect),
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
    /// An instance of a reusable [`XrdsPanelTemplate`], placed in the scene by
    /// this node's own transform.
    ///
    /// The scene half of "attachment is the only difference" — the camera half
    /// is `XrdsScenePlayerAnchor`. Carries no content of its own: everything
    /// authored lives on the template, which is what lets one panel appear in
    /// several places and be edited in one.
    Panel(XrdsScenePanelInstance),
}

/// A placed instance of a panel template: which template, and what its elements
/// are wired to *here*.
///
/// **Where attachment is decided.** A Panel node parented under the scene root is
/// a world panel; parented under a `PlayerAnchor` it is head-locked, using its own
/// `transform` as the camera-local offset. Nothing else distinguishes them — the
/// hierarchy is the attachment, which is why there is no `depth` or `is_hud`
/// field here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsScenePanelInstance {
    pub template_id: XrdsPanelTemplateId,
    /// Per-element trigger bindings, keyed by [`XrdsPanelElement::name`].
    ///
    /// **Bindings live here, not on the template**, because the template is
    /// shared. Three floors instancing one elevator panel each need their own
    /// door; with the bindings on the template all three fired the same Track at
    /// the same fixed node, and nothing could express "my door" —
    /// `XrdsActionTarget::TriggerSource` resolves to the button that fired, not
    /// to anything near it. This is the fix for the hazard the plan's §5
    /// documented and §A4 could only warn about.
    ///
    /// A `BTreeMap` rather than a `Vec` of pairs: duplicate keys become
    /// structurally impossible, and the ordering is deterministic so two saves of
    /// the same scene produce identical JSON.
    ///
    /// A key naming an element the template does not have is **not** silently
    /// dropped — it is reported by `panel_diagnostics`. That happens when an
    /// element is deleted after instances were wired, and dropping it would
    /// discard authored work with no way back.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub element_triggers: BTreeMap<String, Vec<XrdsTriggerBinding>>,
}

impl XrdsScenePanelInstance {
    /// Bindings for `element`, or an empty slice — callers overwhelmingly want to
    /// iterate, not to distinguish "no entry" from "empty entry".
    pub fn triggers_for(&self, element: &str) -> &[XrdsTriggerBinding] {
        self.element_triggers.get(element).map_or(&[], Vec::as_slice)
    }

    /// Replaces `element`'s bindings, removing the key when `bindings` is empty.
    ///
    /// Removing rather than storing an empty vec keeps the document free of
    /// entries that look like wiring and are not, and it is what makes
    /// [`XrdsScenePanelInstance::element_triggers`]'s dangling-key diagnostic
    /// mean something.
    pub fn set_triggers(&mut self, element: impl Into<String>, bindings: Vec<XrdsTriggerBinding>) {
        let element = element.into();
        if bindings.is_empty() {
            self.element_triggers.remove(&element);
        } else {
            self.element_triggers.insert(element, bindings);
        }
    }

    /// Rewrites a binding key after the template renamed an element.
    ///
    /// Renames propagate (a delete does not) because the intent is unambiguous:
    /// the element still exists and is still the thing that was wired. Leaving
    /// the old key would break every instance on a rename.
    pub fn rename_element(&mut self, from: &str, to: impl Into<String>) {
        if let Some(bindings) = self.element_triggers.remove(from) {
            self.element_triggers.insert(to.into(), bindings);
        }
    }
}

impl Default for XrdsPanelTemplateId {
    fn default() -> Self {
        Self(0)
    }
}

impl XrdsSceneNodePayload {
    pub fn gltf_export_class(&self) -> XrdsGltfExportClass {
        match self {
            Self::Empty => XrdsGltfExportClass::NodeOnly,
            Self::Camera(_) => XrdsGltfExportClass::Camera,
            Self::GltfAsset(_) => XrdsGltfExportClass::ExternalSceneReference,
            Self::Cube(_)
            | Self::Cylinder(_)
            | Self::Capsule(_)
            | Self::Sphere(_)
            | Self::Plane3D(_)
            | Self::Tetrahedron(_) => XrdsGltfExportClass::ProceduralMeshBake,
            Self::AmbientLight(_)
            | Self::DirectionalLight(_)
            | Self::PointLight(_)
            | Self::SpotLight(_) => XrdsGltfExportClass::Light,
            // NodeOnly: a particle effect has no bakeable mesh, and glTF has no
            // vocabulary for emitters. Scene GLB export is retired project-wide
            // anyway (see xrds-gltf in CLAUDE.md).
            Self::Effect(_) => XrdsGltfExportClass::NodeOnly,
            Self::AudioClip(_) => XrdsGltfExportClass::NodeOnly,
            Self::InteractionZone(_) => XrdsGltfExportClass::NodeOnly,
            Self::PlayerSpawn(_) => XrdsGltfExportClass::NodeOnly,
            Self::Player(_) => XrdsGltfExportClass::NodeOnly,
            Self::PlayerAnchor(_) => XrdsGltfExportClass::NodeOnly,
            Self::HudText(_)         => XrdsGltfExportClass::NodeOnly,
            Self::Text(_)            => XrdsGltfExportClass::NodeOnly,
            Self::ExtrudedText(_)    => XrdsGltfExportClass::NodeOnly,
            Self::PlayerSpawnZone(_) => XrdsGltfExportClass::NodeOnly,
            Self::Panel(_)           => XrdsGltfExportClass::NodeOnly,
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

impl From<XrdsSceneMaterialTextureSlotKind> for XrdsMaterialTextureSlotKind {
    fn from(value: XrdsSceneMaterialTextureSlotKind) -> Self {
        match value {
            XrdsSceneMaterialTextureSlotKind::BaseColor => Self::BaseColor,
            XrdsSceneMaterialTextureSlotKind::MetallicRoughness => Self::MetallicRoughness,
            XrdsSceneMaterialTextureSlotKind::Normal => Self::Normal,
            XrdsSceneMaterialTextureSlotKind::Occlusion => Self::Occlusion,
            XrdsSceneMaterialTextureSlotKind::Emissive => Self::Emissive,
        }
    }
}

impl From<XrdsMaterialTextureSlotKind> for XrdsSceneMaterialTextureSlotKind {
    fn from(value: XrdsMaterialTextureSlotKind) -> Self {
        match value {
            XrdsMaterialTextureSlotKind::BaseColor => Self::BaseColor,
            XrdsMaterialTextureSlotKind::MetallicRoughness => Self::MetallicRoughness,
            XrdsMaterialTextureSlotKind::Normal => Self::Normal,
            XrdsMaterialTextureSlotKind::Occlusion => Self::Occlusion,
            XrdsMaterialTextureSlotKind::Emissive => Self::Emissive,
        }
    }
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
pub struct XrdsSceneCapsule {
    pub radius: f32,
    /// Excludes the two hemispherical caps — see `XrdsCapsule::length`.
    pub length: f32,
    pub material: XrdsSceneMaterial,
    #[serde(default, skip_serializing_if = "XrdsPhysicsBody::is_none")]
    pub physics_body: XrdsPhysicsBody,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub gravity_scale: f32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub mass: f32,
}

impl Default for XrdsSceneCapsule {
    fn default() -> Self {
        Self {
            radius: 0.5,
            length: 1.0,
            material: XrdsSceneMaterial::default(),
            physics_body: XrdsPhysicsBody::None,
            gravity_scale: 1.0,
            mass: 1.0,
        }
    }
}

/// Serialized form of an `XrdsEffect`.
///
/// Field-for-field with the runtime descriptor's tunable set, minus
/// identity/placement, which every node carries already. Colours are `[f32; 4]`
/// linear RGBA to match the rest of this format.
///
/// `#[serde(default)]` on every field with a sane default: a scene authored
/// before a field existed still loads, which matters because this format is
/// already in users' hands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneEffect {
    #[serde(default)]
    pub kind: XrdsSceneEffectKind,
    /// Emit as soon as the node loads. `false` leaves it idle for a trigger to
    /// fire — see `XrdsEffect::auto_play`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub auto_play: bool,
    /// Particles per firing; `Burst` only.
    #[serde(default = "default_burst_count")]
    pub burst_count: u32,
    /// Particles per second; `Trail` only.
    #[serde(default = "default_spawn_rate")]
    pub spawn_rate: f32,
    #[serde(default = "default_lifetime_secs")]
    pub lifetime_secs: f32,
    #[serde(default = "default_size_min")]
    pub size_min: f32,
    #[serde(default = "default_size_max")]
    pub size_max: f32,
    /// Linear RGBA. Keep components <= 1.0 — brighter values clamp, because the
    /// SDK's XR cameras have no HDR pass. See `XrdsEffect::color_start`.
    #[serde(default = "default_effect_color_start")]
    pub color_start: [f32; 4],
    #[serde(default = "default_effect_color_end")]
    pub color_end: [f32; 4],
    #[serde(default = "default_speed_min")]
    pub speed_min: f32,
    #[serde(default = "default_speed_max")]
    pub speed_max: f32,
    /// Emit in all directions, ignoring `spread_deg`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub omnidirectional: bool,
    /// Cone half-angle about local +Y in degrees; ignored if `omnidirectional`.
    #[serde(default = "default_spread_deg")]
    pub spread_deg: f32,
    #[serde(default = "default_effect_gravity")]
    pub gravity: [f32; 3],
    #[serde(default = "default_emission_radius")]
    pub emission_radius: f32,
    #[serde(default)]
    pub blend: XrdsSceneEffectBlend,
    /// End-of-life size multiplier; `1.0` holds size constant.
    #[serde(default = "default_one_f32")]
    pub size_end: f32,
    #[serde(default = "default_drag")]
    pub drag: f32,
    #[serde(default = "default_fade_edge")]
    pub fade_edge: f32,
    #[serde(default = "default_one_f32")]
    pub fade_scene: f32,
}

/// Wire form of `XrdsEffectBlend`. Separate from the runtime enum for the same
/// reason as `XrdsSceneEffectKind`: these names are a file-format contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum XrdsSceneEffectBlend {
    #[default]
    Blend,
    Add,
    Multiply,
}

fn default_one_f32() -> f32 {
    1.0
}
fn default_drag() -> f32 {
    0.2
}
fn default_fade_edge() -> f32 {
    0.7
}

/// Wire form of `XrdsEffectKind`. Separate from the runtime enum so the
/// serialized names are ours to keep stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum XrdsSceneEffectKind {
    #[default]
    Burst,
    Trail,
}

fn default_true() -> bool {
    true
}
fn is_true(value: &bool) -> bool {
    *value
}
fn default_burst_count() -> u32 {
    300
}
fn default_spawn_rate() -> f32 {
    100.0
}
fn default_lifetime_secs() -> f32 {
    1.5
}
fn default_size_min() -> f32 {
    0.05
}
fn default_size_max() -> f32 {
    0.15
}
fn default_effect_color_start() -> [f32; 4] {
    [1.0, 0.85, 0.35, 1.0]
}
fn default_effect_color_end() -> [f32; 4] {
    [0.5, 0.08, 0.0, 0.0]
}
fn default_speed_min() -> f32 {
    0.8
}
fn default_speed_max() -> f32 {
    1.6
}
fn default_spread_deg() -> f32 {
    45.0
}
fn default_effect_gravity() -> [f32; 3] {
    [0.0, -1.2, 0.0]
}
fn default_emission_radius() -> f32 {
    0.05
}

impl Default for XrdsSceneEffect {
    fn default() -> Self {
        Self {
            kind: XrdsSceneEffectKind::Burst,
            auto_play: true,
            burst_count: default_burst_count(),
            spawn_rate: default_spawn_rate(),
            lifetime_secs: default_lifetime_secs(),
            size_min: default_size_min(),
            size_max: default_size_max(),
            color_start: default_effect_color_start(),
            color_end: default_effect_color_end(),
            speed_min: default_speed_min(),
            speed_max: default_speed_max(),
            omnidirectional: true,
            spread_deg: default_spread_deg(),
            gravity: default_effect_gravity(),
            emission_radius: default_emission_radius(),
            blend: XrdsSceneEffectBlend::Blend,
            size_end: 1.0,
            drag: default_drag(),
            fade_edge: default_fade_edge(),
            fade_scene: 1.0,
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

// `XrdsAudioDistanceModel` moved to xrds-components so the runtime can evaluate a
// falloff curve without depending on the document layer — same reason
// `XrdsGrabType` and `XrdsInteractionZoneShape` live there. Re-exported so the
// document-facing path to it is unchanged.
pub use xrds_components::XrdsAudioDistanceModel;

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
    #[serde(default, skip_serializing_if = "is_default_distance_model")]
    pub distance_model: XrdsAudioDistanceModel,
    #[serde(default = "default_audio_min_distance")]
    pub min_distance: f32,
    #[serde(default = "default_audio_max_distance")]
    pub max_distance: f32,
    #[serde(default = "default_audio_rolloff")]
    pub rolloff_factor: f32,
    // `hrtf: bool` lived here. Removed 2026-08-19: nothing ever read it, and
    // nothing could — binaural rendering needs an HRTF-convolving audio path,
    // which neither bevy_audio nor rodio has (rodio downmixes to mono and applies
    // two per-ear gains; `grep -i hrtf` over its source finds nothing). Shipping a
    // flag that silently does nothing is worse than not offering it. A verified
    // route to real binaural, should it ever be wanted, is written up in
    // `docs/spatial-audio-backend-spike.md`; re-adding the field is a one-liner
    // once the capability behind it exists.
    //
    // Removal is document-compatible: the field was `#[serde(default)]`, so older
    // scenes still load and simply drop a value that never had an effect.
}

fn is_default_distance_model(model: &XrdsAudioDistanceModel) -> bool {
    *model == XrdsAudioDistanceModel::Inverse
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
/// sequencing (see `docs/done/xrds-scenegraph-trigger-action-sequencing.md`) —
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

/// Serialisable layout policy for an [`XrdsPanelTemplate`](crate::XrdsPanelTemplate).
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
            // Not empty. An element you add and then cannot see reads as broken
            // rather than as unfilled, and there is nothing on the canvas to click
            // in order to discover the text field. Same reasoning as the palette
            // bootstrapping a template for an author who has not opened the Panels
            // workspace yet.
            text: "Label".to_string(),
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
            // See `XrdsSceneWorldLabel::default` — a blank button is indistinguishable
            // from a broken one.
            label: "Button".to_string(),
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

/// What kind of thing an [`XrdsPanelElement`](crate::XrdsPanelElement) is.
///
/// Named "world widget" for historical reasons — it began as the child-widget
/// vocabulary of the retired `XrdsSceneWorldPanel`, and is now the element
/// vocabulary of `XrdsPanelTemplate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsSceneWorldWidget {
    Label(XrdsSceneWorldLabel),
    Button(XrdsSceneWorldButton),
    Image(XrdsSceneWorldImage),
    Slider(XrdsSceneWorldSlider),
    Toggle(XrdsSceneWorldToggle),
}

// `XrdsSceneWorldPanel` lived here: an authored world-space panel with inline
// widgets and layout, both fields directly on the node. Retired because inline
// widgets carry no `triggers` — every button on one was permanently dead, unlike
// an `XrdsPanelTemplate` element wired through a `Panel` node's own instance. No
// tracked document ever used it. See docs/done/xrds-widget-template-plan.md §A4b-2.
//
// `XrdsSceneWorldLayout` (below) and `XrdsSceneWorldWidget` are unrelated and
// stay: `XrdsPanelTemplate::layout` and `XrdsPanelElement::kind` are the live
// replacement's own use of exactly these types.

impl From<&XrdsAudioClip> for XrdsSceneAudioClip {
    fn from(value: &XrdsAudioClip) -> Self {
        // Every field is listed explicitly, and `..Default::default()` is
        // deliberately not used here. It was, until 2026-08-19, and it silently
        // reset the falloff fields to defaults on the way from runtime to
        // document — harmless only for as long as nothing read them. Listing the
        // fields means a future addition fails to compile instead of being
        // quietly dropped on save.
        Self {
            asset_id: value.audio_asset_id.clone(),
            volume: value.volume,
            looped: value.looped,
            spatial: value.spatial,
            autoplay: value.autoplay,
            distance_model: value.distance_model,
            min_distance: value.min_distance,
            max_distance: value.max_distance,
            rolloff_factor: value.rolloff_factor,
        }
    }
}

