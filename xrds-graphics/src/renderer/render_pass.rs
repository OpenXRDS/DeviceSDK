use std::ops::Range;

use crate::{pbr, XrdsIndexBuffer, XrdsInstanceBuffer, XrdsMaterialInstance, XrdsVertexBuffer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderPassType {
    /// Renderpass for draw g-buffers
    PbrGbuffer,
    /// Rnderpass for draw shadowmaps
    PbrShadow,
    /// Renderpass for deferred lighting
    PbrLight,
    /// Renderpass for postprocess
    Postprocess,
}

#[derive(Debug)]
pub struct RenderPass<'encoder> {
    inner: wgpu::RenderPass<'encoder>,
    ty: RenderPassType,
}

impl<'e> RenderPass<'e> {
    pub fn new(inner: wgpu::RenderPass<'e>, ty: RenderPassType) -> Self {
        RenderPass { inner, ty }
    }

    pub fn bind_pipeline(&mut self, pipeline: &wgpu::RenderPipeline) {
        self.inner.set_pipeline(pipeline);
    }

    pub fn bind_material(&mut self, material: &XrdsMaterialInstance) {
        self.inner.set_pipeline(material.pipeline());
        self.inner.set_bind_group(
            pbr::BIND_GROUP_INDEX_MATERIAL_INPUT,
            material.bind_group(),
            &[],
        );
    }

    pub fn bind_vertex_buffers(&mut self, vertex_buffers: &[XrdsVertexBuffer], base_vertex: u32) {
        for (index, vb) in vertex_buffers.iter().enumerate() {
            // Vertex buffer slot 0 is for instance buffer. So slot index of actual vertices started from 1
            self.inner
                .set_vertex_buffer(index as u32 + 1 + base_vertex, vb.slice());
        }
    }

    pub fn bind_instance_buffer(&mut self, instance_buffer: &XrdsInstanceBuffer) {
        self.inner.set_vertex_buffer(0, instance_buffer.slice());
    }

    pub fn bind_index_buffer(&mut self, index_buffer: &XrdsIndexBuffer) {
        self.inner
            .set_index_buffer(index_buffer.as_slice(), index_buffer.format());
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &wgpu::BindGroup, offsets: &[u32]) {
        self.inner.set_bind_group(index, bind_group, offsets)
    }

    pub fn set_push_constants(&mut self, stages: wgpu::ShaderStages, offset: u32, data: &[u8]) {
        self.inner.set_push_constants(stages, offset, data);
    }

    pub fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.inner.draw_indexed(indices, base_vertex, instances);
    }

    pub fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.inner.draw(vertices, instances);
    }

    pub fn ty(&self) -> RenderPassType {
        self.ty
    }
}
