pub mod pbr;

#[derive(Debug, Clone)]
pub struct XrdsMaterial {
    pub pipeline: wgpu::RenderPipeline,
}

impl XrdsMaterial {
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}
