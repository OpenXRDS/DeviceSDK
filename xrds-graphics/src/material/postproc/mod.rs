mod deferred_lighting;
mod shadow_mapping;

pub use deferred_lighting::*;
pub use shadow_mapping::*;
use wgpu::RenderPass;

#[derive(Debug, Clone)]
pub struct Postproc {
    pipeline: wgpu::RenderPipeline,
}

impl Postproc {
    pub fn new(pipeline: wgpu::RenderPipeline) -> Self {
        Self { pipeline }
    }

    pub fn encode(&self, render_pass: &mut RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.draw(0..3, 0..1);
    }
}
