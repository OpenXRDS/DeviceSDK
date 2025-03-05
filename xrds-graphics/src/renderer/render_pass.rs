use crate::{pbr, XrdsIndexBuffer, XrdsMaterialInstance, XrdsVertexBuffer};

pub struct RenderPass<'encoder> {
    inner: wgpu::RenderPass<'encoder>,
}

impl<'e> RenderPass<'e> {
    pub fn new(inner: wgpu::RenderPass<'e>) -> Self {
        RenderPass { inner }
    }

    pub fn bind_material(&mut self, material: &XrdsMaterialInstance) {
        self.inner.set_pipeline(material.pipeline());
        self.inner.set_bind_group(
            pbr::BIND_GROUP_INDEX_MATERIAL_INPUT,
            material.bind_group(),
            &[],
        );
    }

    pub fn bind_vertex_buffers(&mut self, vertex_buffers: &[XrdsVertexBuffer]) {
        for (index, vb) in vertex_buffers.iter().enumerate() {
            self.inner.set_vertex_buffer(index as _, vb.as_slice());
        }
    }

    pub fn bind_index_buffer(&mut self, index_buffer: &XrdsIndexBuffer) {
        self.inner
            .set_index_buffer(index_buffer.as_slice(), index_buffer.format());
    }

    pub fn set_push_constants(&mut self, stages: wgpu::ShaderStages, offset: u32, data: &[u8]) {
        self.inner.set_push_constants(stages, offset, data);
    }
}
