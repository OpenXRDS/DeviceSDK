mod deferred_lighting;

pub use deferred_lighting::*;

#[derive(Debug, Clone)]
pub struct Postproc {
    pipeline: wgpu::RenderPipeline,
}

impl Postproc {
    pub fn new(pipeline: wgpu::RenderPipeline) -> Self {
        Self { pipeline }
    }

    pub fn encode(&self, render_pass: &mut wgpu::RenderPass, bind_group: &wgpu::BindGroup) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}
