use crate::{RenderPass, XrdsBuffer};

mod postproc;

pub use postproc::*;
pub mod pbr;

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
        render_pass.bind_material(self);
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn material_params(&self) -> &XrdsBuffer {
        &self.material_params
    }
}
