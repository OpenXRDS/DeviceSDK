use wgpu::{BufferDescriptor, BufferUsages, VertexFormat};
use xrds_core::Transform;

use crate::GraphicsInstance;

use super::{XrdsBuffer, XrdsBufferType};

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct XrdsInstance {
    model: glam::Mat4,
}

#[derive(Debug, Clone)]
pub struct XrdsInstanceBuffer {
    buffers: [XrdsBuffer; 2], // represent current and prev
    index: usize,
}

impl XrdsInstance {
    pub fn new(transform: &Transform) -> Self {
        Self {
            model: transform.to_model_matrix(),
        }
    }

    pub fn update(&mut self, transform: &Transform) {
        self.model = transform.to_model_matrix();
    }
}

impl XrdsInstanceBuffer {
    pub fn new(graphics_instance: &GraphicsInstance, max_instances: usize) -> Self {
        let buffer_creation = || {
            let buffer = graphics_instance.device().create_buffer(&BufferDescriptor {
                label: None,
                size: (std::mem::size_of::<XrdsInstance>() * max_instances) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let xrds_buffer = XrdsBuffer::new(
                buffer,
                XrdsBufferType::Vertex(VertexFormat::Float32x4),
                Some(VertexFormat::Float32x4.size() * 8),
            );
            xrds_buffer
        };
        let buffers = [buffer_creation(), buffer_creation()];

        Self { buffers, index: 0 }
    }

    pub fn write(&mut self, queue: &wgpu::Queue, instances: &[XrdsInstance]) {
        self.index = 1 - self.index;
        queue.write_buffer(
            &self.buffers[self.index].buffer(),
            0,
            bytemuck::cast_slice(instances),
        );
    }

    pub fn encode(&self, render_pass: &mut wgpu::RenderPass<'_>, slot: u32) {
        render_pass.set_vertex_buffer(slot, self.buffers[self.index].slice(..));
        render_pass.set_vertex_buffer(slot + 1, self.buffers[1 - self.index].slice(..));
    }
}
