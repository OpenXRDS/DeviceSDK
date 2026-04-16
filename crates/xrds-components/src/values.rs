use crate::{XrdsColor, XrdsLinearRgba};

#[derive(Debug, Clone, Copy)]
pub struct CubeGeometryParams {
    pub size: [f32; 3],
}

impl Default for CubeGeometryParams {
    fn default() -> Self {
        Self {
            size: [1.0, 1.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CylinderGeometryParams {
    pub radius: f32,
    pub height: f32,
}

impl Default for CylinderGeometryParams {
    fn default() -> Self {
        Self {
            radius: 0.5,
            height: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SphereGeometryParams {
    pub radius: f32,
}

impl Default for SphereGeometryParams {
    fn default() -> Self {
        Self { radius: 0.5 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Plane3DGeometryParams {
    pub size: [f32; 2],
}

impl Default for Plane3DGeometryParams {
    fn default() -> Self {
        Self { size: [1.0, 1.0] }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TetrahedronGeometryParams {
    pub vertices: [[f32; 3]; 4],
}

impl Default for TetrahedronGeometryParams {
    fn default() -> Self {
        Self {
            vertices: [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsMaterialParams {
    pub base_color: XrdsColor,
    pub emissive: XrdsLinearRgba,
    pub opacity: f32,
    pub unlit: bool,
    pub pbr: XrdsMaterialPbrParams,
    pub textures: XrdsMaterialTextureSlots,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsMaterialTextureRef {
    pub texture_asset_id: String,
    pub uv: XrdsMaterialTextureUvParams,
    pub sampler: XrdsMaterialTextureSamplerParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XrdsMaterialTextureSlotKind {
    BaseColor,
    MetallicRoughness,
    Normal,
    Occlusion,
    Emissive,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct XrdsMaterialTextureSlots {
    pub base_color: Option<XrdsMaterialTextureRef>,
    pub metallic_roughness: Option<XrdsMaterialTextureRef>,
    pub normal: Option<XrdsMaterialTextureRef>,
    pub occlusion: Option<XrdsMaterialTextureRef>,
    pub emissive: Option<XrdsMaterialTextureRef>,
}

impl XrdsMaterialTextureSlots {
    pub fn is_empty(&self) -> bool {
        self.base_color.is_none()
            && self.metallic_roughness.is_none()
            && self.normal.is_none()
            && self.occlusion.is_none()
            && self.emissive.is_none()
    }

    pub fn get(&self, slot: XrdsMaterialTextureSlotKind) -> Option<&XrdsMaterialTextureRef> {
        match slot {
            XrdsMaterialTextureSlotKind::BaseColor => self.base_color.as_ref(),
            XrdsMaterialTextureSlotKind::MetallicRoughness => self.metallic_roughness.as_ref(),
            XrdsMaterialTextureSlotKind::Normal => self.normal.as_ref(),
            XrdsMaterialTextureSlotKind::Occlusion => self.occlusion.as_ref(),
            XrdsMaterialTextureSlotKind::Emissive => self.emissive.as_ref(),
        }
    }

    pub fn set(
        &mut self,
        slot: XrdsMaterialTextureSlotKind,
        texture: Option<XrdsMaterialTextureRef>,
    ) {
        match slot {
            XrdsMaterialTextureSlotKind::BaseColor => self.base_color = texture,
            XrdsMaterialTextureSlotKind::MetallicRoughness => self.metallic_roughness = texture,
            XrdsMaterialTextureSlotKind::Normal => self.normal = texture,
            XrdsMaterialTextureSlotKind::Occlusion => self.occlusion = texture,
            XrdsMaterialTextureSlotKind::Emissive => self.emissive = texture,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsMaterialTextureUvParams {
    pub set: u32,
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation_deg: f32,
    pub transform_mode: XrdsMaterialTextureUvTransformMode,
}

impl Default for XrdsMaterialTextureUvParams {
    fn default() -> Self {
        Self {
            set: 0,
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation_deg: 0.0,
            transform_mode: XrdsMaterialTextureUvTransformMode::Centered,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialTextureUvTransformMode {
    Centered,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XrdsMaterialTextureSamplerParams {
    pub wrap_u: XrdsMaterialTextureWrapMode,
    pub wrap_v: XrdsMaterialTextureWrapMode,
    pub min_filter: XrdsMaterialTextureFilterMode,
    pub mag_filter: XrdsMaterialTextureFilterMode,
    pub mipmap_filter: XrdsMaterialTextureFilterMode,
}

impl Default for XrdsMaterialTextureSamplerParams {
    fn default() -> Self {
        Self {
            wrap_u: XrdsMaterialTextureWrapMode::Repeat,
            wrap_v: XrdsMaterialTextureWrapMode::Repeat,
            min_filter: XrdsMaterialTextureFilterMode::Linear,
            mag_filter: XrdsMaterialTextureFilterMode::Linear,
            mipmap_filter: XrdsMaterialTextureFilterMode::Linear,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialTextureWrapMode {
    Repeat,
    MirroredRepeat,
    ClampToEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialTextureFilterMode {
    Linear,
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialAlphaMode {
    Auto,
    Opaque,
    Mask,
    Blend,
}

impl Default for XrdsMaterialAlphaMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsMaterialPbrParams {
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
    pub double_sided: bool,
    pub alpha_mode: XrdsMaterialAlphaMode,
    pub alpha_cutoff: f32,
}

impl Default for XrdsMaterialPbrParams {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            roughness: 0.5,
            reflectance: 0.5,
            double_sided: false,
            alpha_mode: XrdsMaterialAlphaMode::Auto,
            alpha_cutoff: 0.5,
        }
    }
}

impl Default for XrdsMaterialParams {
    fn default() -> Self {
        Self {
            base_color: XrdsColor::WHITE,
            emissive: XrdsLinearRgba::BLACK,
            opacity: 1.0,
            unlit: false,
            pbr: XrdsMaterialPbrParams::default(),
            textures: XrdsMaterialTextureSlots::default(),
        }
    }
}

/// Transform parameters for XRDS scene objects.
///
/// **Authoritative rotation field**: `rotation_quat_xyzw`. The runtime reads only this field
/// when projecting transforms into the engine. `rotation_euler_xyz_deg` is provided as an
/// authoring convenience and is kept in sync during export, but is never read by the runtime.
/// When constructing or editing a transform, set `rotation_quat_xyzw` directly, or use the
/// typed helpers on the spawn and edit APIs (`set_rotation`, `looking_at`, etc.).
#[derive(Debug, Clone, Copy)]
pub struct TransformParams {
    pub translation: [f32; 3],
    pub rotation_quat_xyzw: [f32; 4],
    pub rotation_euler_xyz_deg: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for TransformParams {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            rotation_euler_xyz_deg: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}
