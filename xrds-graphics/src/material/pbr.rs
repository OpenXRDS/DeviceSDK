use std::{
    borrow::Cow,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::RwLock,
};

use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue,
};
use wgpu::{naga::valid::Capabilities, Device, ShaderModuleDescriptor};

pub static BIND_GROUP_INDEX_VIEW_PARAMS: u32 = 0;
pub static BIND_GROUP_INDEX_MATERIAL_INPUT: u32 = 1;
pub static BIND_GROUP_INDEX_SKINNING_MATRICES: u32 = 2;

#[derive(Debug, Default, Clone, Copy, Hash)]
pub struct Options {
    pub vertex_input: PbrVertexInputOption,
    pub material_input: PbrMaterialInputOption,
    pub view_count: u32,
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
#[derive(Debug, Default, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
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

pub struct PbrShaderBuilder {
    composer: RwLock<Composer>,
}

impl PbrShaderBuilder {
    pub fn new() -> anyhow::Result<Self> {
        let mut composer = Composer::default()
            .with_capabilities(Capabilities::MULTIVIEW | Capabilities::PUSH_CONSTANT);

        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("shader/pbr/vertex_params.wgsl"),
            file_path: "shader/pbr/vertex_params.wgsl",
            ..Default::default()
        })?;
        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("shader/view_params.wgsl"),
            file_path: "shader/view_params.wgsl",

            ..Default::default()
        })?;
        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("shader/skinning.wgsl"),
            file_path: "shader/skinning.wgsl",
            ..Default::default()
        })?;
        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("shader/pbr/fragment_params.wgsl"),
            file_path: "shader/pbr/fragment_params.wgsl",
            ..Default::default()
        })?;
        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("shader/pbr/material_params.wgsl"),
            file_path: "shader/pbr/material_params.wgsl",
            ..Default::default()
        })?;

        Ok(Self {
            composer: RwLock::new(composer),
        })
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
            defs.insert(
                "PUSH_CONSTANT_SUPPORTED".to_owned(),
                ShaderDefValue::Bool(true),
            );
        }

        let naga_module = {
            let mut lock = self.composer.write().unwrap();
            lock.make_naga_module(NagaModuleDescriptor {
                source,
                file_path,
                shader_defs: defs,
                shader_type: naga_oil::compose::ShaderType::Wgsl,
                ..Default::default()
            })?
        };

        Ok(device.create_shader_module(ShaderModuleDescriptor {
            label: Some(file_path),
            source: wgpu::ShaderSource::Naga(Cow::Owned(naga_module)),
        }))
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

impl From<&Options> for HashMap<String, ShaderDefValue> {
    fn from(value: &Options) -> Self {
        value.shader_defines().into_iter().collect()
    }
}

impl Options {
    fn shader_defines(&self) -> HashMap<String, ShaderDefValue> {
        let vertex_key_values = self.vertex_input.shader_defines();
        let material_key_values = self.material_input.shader_defines();
        let option_key_values = vec![(
            "VIEW_COUNT".to_owned(),
            ShaderDefValue::UInt(self.view_count),
        )];

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

    fn shader_defines(&self) -> Vec<(String, ShaderDefValue)> {
        let mut res = Vec::new();
        self.position.then(|| {
            res.push((
                Self::VERTEX_INPUT_POSITION.to_owned(),
                ShaderDefValue::Bool(self.position),
            ))
        });
        self.color.is_some().then(|| {
            res.push((
                Self::VERTEX_INPUT_COLOR.to_owned(),
                ShaderDefValue::Bool(self.color.is_some()),
            ));
            if let Some(ch) = self.color {
                if ch == ColorChannel::Ch3 {
                    res.push((
                        Self::VERTEX_INPUT_COLOR_3CH.to_owned(),
                        ShaderDefValue::Bool(true),
                    ));
                }
            }
        });
        self.normal.then(|| {
            res.push((
                Self::VERTEX_INPUT_NORMAL.to_owned(),
                ShaderDefValue::Bool(self.normal),
            ))
        });
        self.tangent.then(|| {
            res.push((
                Self::VERTEX_INPUT_TANGENT.to_owned(),
                ShaderDefValue::Bool(self.tangent),
            ))
        });
        self.texcoord_0.then(|| {
            res.push((
                Self::VERTEX_INPUT_TEXCOORD_0.to_owned(),
                ShaderDefValue::Bool(self.texcoord_0),
            ));
            self.texcoord_1.then(|| {
                res.push((
                    Self::VERTEX_INPUT_TEXCOORD_1.to_owned(),
                    ShaderDefValue::Bool(false),
                ))
            });
        });
        self.weights_joints_0.then(|| {
            res.push((
                Self::VERTEX_INPUT_WEIGHTS_JOINTS_0.to_owned(),
                ShaderDefValue::Bool(self.weights_joints_0),
            ));
            self.weights_joints_1.then(|| {
                res.push((
                    Self::VERTEX_INPUT_WEIGHTS_JOINTS_1.to_owned(),
                    ShaderDefValue::Bool(self.weights_joints_1),
                ))
            });
        });
        self.instance.then(|| {
            res.push((
                Self::VERTEX_INPUT_INSTANCE.to_owned(),
                ShaderDefValue::Bool(self.instance),
            ))
        });

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

    fn shader_defines(&self) -> Vec<(String, ShaderDefValue)> {
        let mut res = Vec::new();
        self.base_color.then(|| {
            res.push((
                Self::MATERIAL_INPUT_BASE_COLOR_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.base_color),
            ))
        });
        self.emissive.then(|| {
            res.push((
                Self::MATERIAL_INPUT_EMISSIVE_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.emissive),
            ))
        });
        self.metallic_roughness.then(|| {
            res.push((
                Self::MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.metallic_roughness),
            ))
        });
        self.normal.then(|| {
            res.push((
                Self::MATERIAL_INPUT_NORMAL_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.normal),
            ))
        });
        self.occlusion.then(|| {
            res.push((
                Self::MATERIAL_INPUT_OCCLUSION_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.occlusion),
            ))
        });
        #[cfg(feature = "material_spec_gloss")]
        self.diffuse.then(|| {
            res.push((
                Self::MATERIAL_INPUT_DIFFUSE_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.diffuse),
            ))
        });
        #[cfg(feature = "material_spec_gloss")]
        self.specular_glossiness.then(|| {
            res.push((
                Self::MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.specular_glossiness),
            ))
        });
        #[cfg(feature = "material_ibl")]
        self.ibl.then(|| {
            res.push((
                Self::MATERIAL_INPUT_IBL.to_owned(),
                ShaderDefValue::Bool(self.ibl),
            ));
            res.push((
                Self::MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE.to_owned(),
                ShaderDefValue::Bool(true),
            ));
            res.push((
                Self::MATERIAL_INPUT_IBL_SPECULAR_TEXTURE.to_owned(),
                ShaderDefValue::Bool(true),
            ));
        });
        #[cfg(feature = "material_ibl")]
        self.brdf.then(|| {
            res.push((
                Self::MATERIAL_INPUT_BRDF_TEXTURE.to_owned(),
                ShaderDefValue::Bool(self.brdf),
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
