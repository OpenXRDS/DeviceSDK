use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use wgpu::Device;

use crate::preprocessor::ShaderValue;

use super::preprocessor::Preprocessor;

pub static BIND_GROUP_INDEX_VIEW_PARAMS: u32 = 0;
pub static BIND_GROUP_INDEX_MATERIAL_INPUT: u32 = 1;
pub static BIND_GROUP_INDEX_SKINNING_MATRICES: u32 = 2;

#[derive(Debug, Default, Clone, Copy, Hash)]
pub struct Options {
    pub vertex_input: PbrVertexInputOption,
    pub material_input: PbrMaterialInputOption,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Hash)]
pub enum ColorChannel {
    Ch3,
    #[default]
    Ch4,
}

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub enum PbrVertexSemantic {
    Position,
    Normal,
    Tangent,
    Color(u32),
    Texcoord(u32),
    Weights(u32),
    Joints(u32),
}

#[derive(Debug, Default, Clone, Copy, Hash)]
pub struct PbrVertexInputOption {
    pub position: bool,
    pub color: Option<ColorChannel>,
    pub texcoord_0: bool,
    pub texcoord_1: bool,
    pub normal: bool,
    pub tangent: bool,
    pub weights_joints_0: bool,
    pub weights_joints_1: bool,
    pub instance: bool,
}

#[derive(Debug, Default, Clone, Copy, Hash)]
pub enum AlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}

#[derive(Debug, Default, Clone, Copy, Hash)]
pub enum PrimitiveMode {
    #[default]
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
    PointList,
}

#[derive(Debug, Default, Clone, Copy, Hash)]
pub struct PbrMaterialInputOption {
    pub base_color: bool,
    pub normal: bool,
    pub emissive: bool,
    pub metallic_roughness: bool,
    pub occlusion: bool,
    pub double_sided: bool,
    pub alpha_mode: AlphaMode,
    pub primitive_mode: PrimitiveMode,
    #[cfg(feature = "material_spec_gloss")]
    pub diffuse: bool,
    #[cfg(feature = "material_spec_gloss")]
    pub specular_glossiness: bool,
    #[cfg(feature = "material_ibl")]
    pub ibl: bool,
    #[cfg(feature = "material_ibl")]
    pub brdf: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PbrMaterialParams {
    pub base_color_factor: glam::Vec4,
    pub emissive_factor: glam::Vec4,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
    pub alpha_cutoff: f32,
    pub texcoord_base_color: u32,
    pub texcoord_emissive: u32,
    pub texcoord_metallic_roughness: u32,
    pub texcoord_normal: u32,
    pub texcoord_occlusion: u32,
    #[cfg(feature = "material_spec_gloss")]
    pub texcoord_diffuse: u32,
    #[cfg(feature = "material_spec_gloss")]
    pub texcoord_specular_glossiness: u32,
    #[cfg(not(feature = "material_spec_gloss"))]
    _pad: [u32; 2],
}

impl Default for PbrMaterialParams {
    fn default() -> Self {
        Self {
            base_color_factor: glam::Vec4::splat(1.0),
            emissive_factor: glam::Vec4::splat(0.0),
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_cutoff: 0.5,
            texcoord_base_color: 0,
            texcoord_emissive: 0,
            texcoord_metallic_roughness: 0,
            texcoord_normal: 0,
            texcoord_occlusion: 0,
            #[cfg(feature = "material_spec_gloss")]
            texcoord_diffuse: 0,
            #[cfg(feature = "material_spec_gloss")]
            texcoord_specular_glossiness: 0,
            #[cfg(not(feature = "material_spec_gloss"))]
            _pad: [0; 2],
        }
    }
}

#[derive(Debug)]
pub struct PbrShaderBuilder {
    preprocessor: Preprocessor,
}

impl PbrShaderBuilder {
    pub fn new() -> anyhow::Result<Self> {
        let mut preprocessor = Preprocessor::default();

        preprocessor.add_include_module("common::utils", include_str!("shader/common/utils.wgsl"));
        preprocessor.add_include_module(
            "common::view_params",
            include_str!("shader/common/view_params.wgsl"),
        );
        preprocessor.add_include_module(
            "common::skinning",
            include_str!("shader/common/skinning.wgsl"),
        );
        preprocessor.add_include_module(
            "pbr::vertex_params",
            include_str!("shader/pbr/vertex_params.wgsl"),
        );
        preprocessor.add_include_module(
            "pbr::fragment_params",
            include_str!("shader/pbr/fragment_params.wgsl"),
        );
        preprocessor.add_include_module(
            "pbr::material_params",
            include_str!("shader/pbr/material_params.wgsl"),
        );

        Ok(Self { preprocessor })
    }

    pub fn build_shader_module(
        &self,
        device: &Device,
        source: &str,
        file_path: &str,
        options: &Options,
    ) -> anyhow::Result<wgpu::ShaderModule> {
        let mut defs = options.shader_defines();

        // Additional defines for device limits
        let limits = device.limits();
        if limits.max_push_constant_size >= 64 {
            defs.insert("PUSH_CONSTANT_SUPPORTED".to_owned(), ShaderValue::Def);
        }

        let descriptor = self.preprocessor.build(source, &defs, Some(file_path))?;

        Ok(device.create_shader_module(descriptor))
    }

    pub fn build_vertex_module(
        &self,
        device: &Device,
        options: &Options,
    ) -> anyhow::Result<wgpu::ShaderModule> {
        self.build_shader_module(
            device,
            include_str!("shader/pbr/vertex.wgsl"),
            "shader/pbr/vertex.wgsl",
            options,
        )
    }

    pub fn build_fragment_module(
        &self,
        device: &Device,
        options: &Options,
    ) -> anyhow::Result<wgpu::ShaderModule> {
        self.build_shader_module(
            device,
            include_str!("shader/pbr/fragment.wgsl"),
            "shader/pbr/fragment.wgsl",
            options,
        )
    }
}

impl From<&Options> for HashMap<String, ShaderValue> {
    fn from(value: &Options) -> Self {
        value.shader_defines().into_iter().collect()
    }
}

impl Options {
    fn shader_defines(&self) -> HashMap<String, ShaderValue> {
        let vertex_key_values = self.vertex_input.shader_defines();
        let material_key_values = self.material_input.shader_defines();
        let option_key_values = vec![];

        option_key_values
            .into_iter()
            .chain(vertex_key_values)
            .chain(material_key_values)
            .collect()
    }

    pub fn as_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash_value = hasher.finish();
        base64::engine::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            hash_value.to_le_bytes(),
        )
    }
}

impl PbrVertexInputOption {
    const VERTEX_INPUT_POSITION: &str = "VERTEX_INPUT_POSITION";
    const VERTEX_INPUT_COLOR: &str = "VERTEX_INPUT_COLOR";
    const VERTEX_INPUT_COLOR_3CH: &str = "VERTEX_INPUT_COLOR_3CH";
    const VERTEX_INPUT_NORMAL: &str = "VERTEX_INPUT_NORMAL";
    const VERTEX_INPUT_TANGENT: &str = "VERTEX_INPUT_TANGENT";
    const VERTEX_INPUT_TEXCOORD_0: &str = "VERTEX_INPUT_TEXCOORD_0";
    const VERTEX_INPUT_TEXCOORD_1: &str = "VERTEX_INPUT_TEXCOORD_1";
    const VERTEX_INPUT_WEIGHTS_JOINTS_0: &str = "VERTEX_INPUT_WEIGHTS_0";
    const VERTEX_INPUT_WEIGHTS_JOINTS_1: &str = "VERTEX_INPUT_WEIGHTS_1";
    const VERTEX_INPUT_INSTANCE: &str = "VERTEX_INPUT_INSTANCE";

    fn shader_defines(&self) -> Vec<(String, ShaderValue)> {
        let mut res = Vec::new();
        self.position
            .then(|| res.push((Self::VERTEX_INPUT_POSITION.to_owned(), ShaderValue::Def)));
        self.color.is_some().then(|| {
            res.push((Self::VERTEX_INPUT_COLOR.to_owned(), ShaderValue::Def));
            if let Some(ch) = self.color {
                if ch == ColorChannel::Ch3 {
                    res.push((Self::VERTEX_INPUT_COLOR_3CH.to_owned(), ShaderValue::Def));
                }
            }
        });
        self.normal
            .then(|| res.push((Self::VERTEX_INPUT_NORMAL.to_owned(), ShaderValue::Def)));
        self.tangent
            .then(|| res.push((Self::VERTEX_INPUT_TANGENT.to_owned(), ShaderValue::Def)));
        self.texcoord_0.then(|| {
            res.push((Self::VERTEX_INPUT_TEXCOORD_0.to_owned(), ShaderValue::Def));
            self.texcoord_1
                .then(|| res.push((Self::VERTEX_INPUT_TEXCOORD_1.to_owned(), ShaderValue::Def)));
        });
        self.weights_joints_0.then(|| {
            res.push((
                Self::VERTEX_INPUT_WEIGHTS_JOINTS_0.to_owned(),
                ShaderValue::Def,
            ));
            self.weights_joints_1.then(|| {
                res.push((
                    Self::VERTEX_INPUT_WEIGHTS_JOINTS_1.to_owned(),
                    ShaderValue::Def,
                ))
            });
        });
        self.instance
            .then(|| res.push((Self::VERTEX_INPUT_INSTANCE.to_owned(), ShaderValue::Def)));

        res
    }
}

impl PbrMaterialInputOption {
    const MATERIAL_INPUT_BASE_COLOR_TEXTURE: &str = "MATERIAL_INPUT_BASE_COLOR_TEXTURE";
    const MATERIAL_INPUT_EMISSIVE_TEXTURE: &str = "MATERIAL_INPUT_EMISSIVE_TEXTURE";
    const MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE: &str =
        "MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE";
    const MATERIAL_INPUT_NORMAL_TEXTURE: &str = "MATERIAL_INPUT_NORMAL_TEXTURE";
    const MATERIAL_INPUT_OCCLUSION_TEXTURE: &str = "MATERIAL_INPUT_OCCLUSION_TEXTURE";
    const MATERIAL_INPUT_ALPHA_MODE_OPAQUE: &str = "MATERIAL_INPUT_ALPHA_MODE_OPAQUE";
    const MATERIAL_INPUT_ALPHA_MODE_MASK: &str = "MATERIAL_INPUT_ALPHA_MODE_MASK";
    const MATERIAL_INPUT_ALPHA_MODE_BLEND: &str = "MATERIAL_INPUT_ALPHA_MODE_BLEND";
    #[cfg(feature = "material_spec_gloss")]
    const MATERIAL_INPUT_DIFFUSE_TEXTURE: &str = "MATERIAL_INPUT_DIFFUSE_TEXTURE";
    #[cfg(feature = "material_spec_gloss")]
    const MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE: &str =
        "MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE";
    #[cfg(feature = "material_ibl")]
    const MATERIAL_INPUT_IBL: &str = "MATERIAL_INPUT_IBL";
    #[cfg(feature = "material_ibl")]
    const MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE: &str = "MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE";
    #[cfg(feature = "material_ibl")]
    const MATERIAL_INPUT_IBL_SPECULAR_TEXTURE: &str = "MATERIAL_INPUT_IBL_SPECULAR_TEXTURE";
    #[cfg(feature = "material_ibl")]
    const MATERIAL_INPUT_BRDF_TEXTURE: &str = "MATERIAL_INPUT_BRDF_TEXTURE";

    fn shader_defines(&self) -> Vec<(String, ShaderValue)> {
        let mut res = Vec::new();
        self.base_color.then(|| {
            res.push((
                Self::MATERIAL_INPUT_BASE_COLOR_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });
        self.emissive.then(|| {
            res.push((
                Self::MATERIAL_INPUT_EMISSIVE_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });
        self.metallic_roughness.then(|| {
            res.push((
                Self::MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });
        self.normal.then(|| {
            res.push((
                Self::MATERIAL_INPUT_NORMAL_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });
        self.occlusion.then(|| {
            res.push((
                Self::MATERIAL_INPUT_OCCLUSION_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });
        res.push(match self.alpha_mode {
            AlphaMode::Opaque => (
                Self::MATERIAL_INPUT_ALPHA_MODE_OPAQUE.to_owned(),
                ShaderValue::Def,
            ),
            AlphaMode::Mask => (
                Self::MATERIAL_INPUT_ALPHA_MODE_MASK.to_owned(),
                ShaderValue::Def,
            ),
            AlphaMode::Blend => (
                Self::MATERIAL_INPUT_ALPHA_MODE_BLEND.to_owned(),
                ShaderValue::Def,
            ),
        });
        #[cfg(feature = "material_spec_gloss")]
        self.diffuse.then(|| {
            res.push((
                Self::MATERIAL_INPUT_DIFFUSE_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });
        #[cfg(feature = "material_spec_gloss")]
        self.specular_glossiness.then(|| {
            res.push((
                Self::MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });
        #[cfg(feature = "material_ibl")]
        self.ibl.then(|| {
            res.push((Self::MATERIAL_INPUT_IBL.to_owned(), ShaderValue::Def));
            res.push((
                Self::MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE.to_owned(),
                ShaderValue::Def,
            ));
            res.push((
                Self::MATERIAL_INPUT_IBL_SPECULAR_TEXTURE.to_owned(),
                ShaderValue::Def,
            ));
        });
        #[cfg(feature = "material_ibl")]
        self.brdf.then(|| {
            res.push((
                Self::MATERIAL_INPUT_BRDF_TEXTURE.to_owned(),
                ShaderValue::Def,
            ))
        });

        res
    }
}

impl PbrVertexSemantic {
    pub fn location(&self) -> u32 {
        match *self {
            Self::Position => 0,
            Self::Color(_n) => 1,
            Self::Texcoord(n) => 2 + n,
            Self::Normal => 4,
            Self::Tangent => 5,
            Self::Weights(n) => 6 + (n * 2),
            Self::Joints(n) => 7 + (n * 2),
        }
    }
}
