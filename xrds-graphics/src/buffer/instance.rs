use wgpu::{BufferDescriptor, BufferSlice, BufferUsages, VertexFormat};
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
    buffer: XrdsBuffer,
}

impl XrdsInstance {
    pub fn new(transform: Transform) -> Self {
        Self {
            model: transform.to_model_matrix(),
        }
    }
}

impl XrdsInstanceBuffer {
    pub fn new(graphics_instance: &GraphicsInstance, max_instances: usize) -> Self {
        let buffer = graphics_instance.device().create_buffer(&BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<XrdsInstance>() * max_instances) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let xrds_buffer = XrdsBuffer::new(
            buffer,
            XrdsBufferType::Vertex(VertexFormat::Float32x4),
            Some(VertexFormat::Float32x4.size() * 4),
        );

        Self {
            buffer: xrds_buffer,
        }
    }

    pub fn write(&self, queue: &wgpu::Queue, instances: &[XrdsInstance]) {
        queue.write_buffer(&self.buffer.buffer(), 0, bytemuck::cast_slice(instances));
    }

    pub fn slice(&self) -> BufferSlice<'_> {
        // Instance buffer always bound all
        self.buffer.slice(..)
    }
}
