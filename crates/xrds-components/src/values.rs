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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsMaterialParams {
    pub base_color: XrdsColor,
    pub emissive: XrdsLinearRgba,
    pub opacity: f32,
    pub unlit: bool,
    pub pbr: XrdsMaterialPbrParams,
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
    pub perceptual_roughness: f32,
    pub reflectance: f32,
    pub double_sided: bool,
    pub alpha_mode: XrdsMaterialAlphaMode,
    pub alpha_cutoff: f32,
}

impl Default for XrdsMaterialPbrParams {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            perceptual_roughness: 0.5,
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
        }
    }
}

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
