use crate::{Constant, XrdsBuffer};

mod postproc;
mod shadow_mapping;

pub mod pbr;
pub mod preprocessor;

pub use postproc::*;
pub use shadow_mapping::*;

use wgpu::RenderPass;

/// Material interface object
#[derive(Debug, Clone)]
pub struct XrdsMaterial {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

#[derive(Debug, Clone)]
pub struct XrdsMaterialInstance {
    pub inner: XrdsMaterial,
    pub material_params: XrdsBuffer,
    pub bind_group: wgpu::BindGroup,
}

impl XrdsMaterial {
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn bind_group_layouts(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

impl XrdsMaterialInstance {
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        self.inner.pipeline()
    }

    pub fn encode(&self, render_pass: &mut RenderPass) {
        render_pass.set_pipeline(self.pipeline());
        render_pass.set_bind_group(Constant::BIND_GROUP_ID_TEXTURE_INPUT, &self.bind_group, &[]);
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn material_params(&self) -> &XrdsBuffer {
        &self.material_params
    }
}
