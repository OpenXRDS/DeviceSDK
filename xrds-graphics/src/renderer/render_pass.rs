use crate::XrdsMaterial;

pub struct RenderPass<'encoder> {
    inner: wgpu::RenderPass<'encoder>,
}

impl<'e> RenderPass<'e> {
    pub fn new(inner: wgpu::RenderPass<'e>) -> Self {
        RenderPass { inner }
    }

    pub fn bind_vertex_buffer(&mut self) {
        // self.inner.set_vertex_buffer(slot, buffer_slice);
    }

    pub fn bind_material(&mut self, material: XrdsMaterial) {
        self.inner.set_pipeline(material.pipeline());
    }
}
