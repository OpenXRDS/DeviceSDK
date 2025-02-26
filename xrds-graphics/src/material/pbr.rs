use std::{borrow::Cow, collections::HashMap};

use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue,
};
use wgpu::{Device, ShaderModuleDescriptor};

#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    pub vertex_input: PbrVertexInputOption,
    pub fragment_output: PbrFragmentOutputOption,
    pub material_input: PbrMaterialInputOption,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ColorChannel {
    Ch3,
    #[default]
    Ch4,
}

#[derive(Debug, Default, Clone, Copy)]
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

#[derive(Debug, Default, Clone, Copy)]
pub struct PbrMaterialInputOption {
    pub base_color: bool,
    pub normal: bool,
    pub emissive: bool,
    pub metallic_roughness: bool,
    pub occlusion: bool,
    pub diffuse: bool,
    pub specular_glossiness: bool,
    pub ibl: bool,
    pub brdf: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PbrFragmentOutputOption {
    pub motion_vector: bool,
    pub final_color: bool,
    pub specular_roughness: bool,
    pub diffuse: bool,
    pub normals: bool,
    pub upscale_reactive: bool,
    pub upscale_transparency_and_composition: bool,
}

#[derive(Debug, Clone)]
pub struct PbrMaterial;

pub struct PbrShader {
    composer: Composer,
}

impl PbrShader {
    pub fn new() -> anyhow::Result<Self> {
        let mut composer = Composer::default();

        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("shader/pbr/vertex_params.wgsl"),
            file_path: "shader/pbr/vertex_params.wgsl",
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
        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("shader/skinning.wgsl"),
            file_path: "shader/skinning.wgsl",
            ..Default::default()
        })?;

        Ok(Self { composer })
    }

    pub fn build_shader_module(
        &mut self,
        device: &Device,
        source: &str,
        file_path: &str,
        defs: &HashMap<String, ShaderDefValue>,
    ) -> anyhow::Result<wgpu::ShaderModule> {
        let naga_module = self.composer.make_naga_module(NagaModuleDescriptor {
            source,
            file_path,
            shader_defs: defs.clone(),
            ..Default::default()
        })?;

        Ok(device.create_shader_module(ShaderModuleDescriptor {
            label: Some(file_path),
            source: wgpu::ShaderSource::Naga(Cow::Owned(naga_module)),
        }))
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
        let fragment_key_values = self.fragment_output.shader_defines();

        vertex_key_values
            .into_iter()
            .chain(material_key_values.into_iter())
            .chain(fragment_key_values.into_iter())
            .collect()
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
        res.push((
            Self::VERTEX_INPUT_POSITION.to_owned(),
            ShaderDefValue::Bool(self.position),
        ));
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
        res.push((
            Self::VERTEX_INPUT_NORMAL.to_owned(),
            ShaderDefValue::Bool(self.normal),
        ));
        res.push((
            Self::VERTEX_INPUT_TANGENT.to_owned(),
            ShaderDefValue::Bool(self.tangent),
        ));
        res.push((
            Self::VERTEX_INPUT_TEXCOORD_0.to_owned(),
            ShaderDefValue::Bool(self.texcoord_0),
        ));
        if self.texcoord_0 {
            res.push((
                Self::VERTEX_INPUT_TEXCOORD_1.to_owned(),
                ShaderDefValue::Bool(self.texcoord_1),
            ));
        } else {
            res.push((
                Self::VERTEX_INPUT_TEXCOORD_1.to_owned(),
                ShaderDefValue::Bool(false),
            ));
        }
        res.push((
            Self::VERTEX_INPUT_WEIGHTS_JOINTS_0.to_owned(),
            ShaderDefValue::Bool(self.weights_joints_0),
        ));
        if self.weights_joints_0 {
            res.push((
                Self::VERTEX_INPUT_WEIGHTS_JOINTS_1.to_owned(),
                ShaderDefValue::Bool(self.weights_joints_1),
            ));
        } else {
            res.push((
                Self::VERTEX_INPUT_WEIGHTS_JOINTS_1.to_owned(),
                ShaderDefValue::Bool(false),
            ));
        }
        res.push((
            Self::VERTEX_INPUT_INSTANCE.to_owned(),
            ShaderDefValue::Bool(self.instance),
        ));

        res
    }
}

impl PbrMaterialInputOption {
    const MATERIAL_INPUT_BASE_COLOR_TEXTURE: &str = "MATERIAL_INPUT_BASE_COLOR_TEXTURE";
    const MATERIAL_INPUT_DIFFUSE_TEXTURE: &str = "MATERIAL_INPUT_DIFFUSE_TEXTURE";
    const MATERIAL_INPUT_EMISSIVE_TEXTURE: &str = "MATERIAL_INPUT_EMISSIVE_TEXTURE";
    const MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE: &str =
        "MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE";
    const MATERIAL_INPUT_NORMAL_TEXTURE: &str = "MATERIAL_INPUT_NORMAL_TEXTURE";
    const MATERIAL_INPUT_OCCLUSION_TEXTURE: &str = "MATERIAL_INPUT_OCCLUSION_TEXTURE";
    const MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE: &str =
        "MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE";
    const MATERIAL_INPUT_IBL: &str = "MATERIAL_INPUT_IBL";
    const MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE: &str = "MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE";
    const MATERIAL_INPUT_IBL_SPECULAR_TEXTURE: &str = "MATERIAL_INPUT_IBL_SPECULAR_TEXTURE";
    const MATERIAL_INPUT_BRDF_TEXTURE: &str = "MATERIAL_INPUT_BRDF_TEXTURE";

    fn shader_defines(&self) -> Vec<(String, ShaderDefValue)> {
        let mut res = Vec::new();
        res.push((
            Self::MATERIAL_INPUT_BASE_COLOR_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.base_color),
        ));
        res.push((
            Self::MATERIAL_INPUT_DIFFUSE_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.diffuse),
        ));
        res.push((
            Self::MATERIAL_INPUT_EMISSIVE_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.emissive),
        ));
        res.push((
            Self::MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.metallic_roughness),
        ));
        res.push((
            Self::MATERIAL_INPUT_NORMAL_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.normal),
        ));
        res.push((
            Self::MATERIAL_INPUT_OCCLUSION_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.occlusion),
        ));
        res.push((
            Self::MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.specular_glossiness),
        ));
        res.push((
            Self::MATERIAL_INPUT_IBL.to_owned(),
            ShaderDefValue::Bool(self.ibl),
        ));
        if self.ibl {
            res.push((
                Self::MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE.to_owned(),
                ShaderDefValue::Bool(true),
            ));
            res.push((
                Self::MATERIAL_INPUT_IBL_SPECULAR_TEXTURE.to_owned(),
                ShaderDefValue::Bool(true),
            ));
        }
        res.push((
            Self::MATERIAL_INPUT_BRDF_TEXTURE.to_owned(),
            ShaderDefValue::Bool(self.brdf),
        ));

        res
    }
}

impl PbrFragmentOutputOption {
    const FRAGMENT_OUTPUT_FINAL_COLOR: &str = "FRAGMENT_OUTPUT_FINAL_COLOR";
    const FRAGMENT_OUTPUT_DIFFUSE: &str = "FRAGMENT_OUTPUT_DIFFUSE";
    const FRAGMENT_OUTPUT_NORMALS: &str = "FRAGMENT_OUTPUT_NORMALS";
    const FRAGMENT_OUTPUT_SPECULAR_ROUGHNESS: &str = "FRAGMENT_OUTPUT_SPECULAR_ROUGHNESS";
    const FRAGMENT_OUTPUT_UPSCALE_REACTIVE: &str = "FRAGMENT_OUTPUT_UPSCALE_REACTIVE";
    const FRAGMENT_OUTPUT_UPSCALE_TRANSPARENCY_AND_COMPOSITION: &str =
        "FRAGMENT_OUTPUT_UPSCALE_TRANSPARENCY_AND_COMPOSITION";
    const FRAGMENT_OUTPUT_MOTION_VECTOR: &str = "FRAGMENT_OUTPUT_MOTION_VECTOR";

    fn shader_defines(&self) -> Vec<(String, ShaderDefValue)> {
        let mut res = Vec::new();
        res.push((
            Self::FRAGMENT_OUTPUT_FINAL_COLOR.to_owned(),
            ShaderDefValue::Bool(self.final_color),
        ));
        res.push((
            Self::FRAGMENT_OUTPUT_DIFFUSE.to_owned(),
            ShaderDefValue::Bool(self.diffuse),
        ));
        res.push((
            Self::FRAGMENT_OUTPUT_NORMALS.to_owned(),
            ShaderDefValue::Bool(self.normals),
        ));
        res.push((
            Self::FRAGMENT_OUTPUT_SPECULAR_ROUGHNESS.to_owned(),
            ShaderDefValue::Bool(self.specular_roughness),
        ));
        res.push((
            Self::FRAGMENT_OUTPUT_UPSCALE_REACTIVE.to_owned(),
            ShaderDefValue::Bool(self.upscale_reactive),
        ));
        res.push((
            Self::FRAGMENT_OUTPUT_UPSCALE_TRANSPARENCY_AND_COMPOSITION.to_owned(),
            ShaderDefValue::Bool(self.upscale_transparency_and_composition),
        ));
        res.push((
            Self::FRAGMENT_OUTPUT_MOTION_VECTOR.to_owned(),
            ShaderDefValue::Bool(self.motion_vector),
        ));

        res
    }
}
