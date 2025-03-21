mod index;
mod instance;
mod vertex;

pub use index::*;
pub use instance::*;
pub use vertex::*;

use std::{fmt::Debug, ops::RangeBounds};

use wgpu::{BufferAddress, BufferSlice, IndexFormat, VertexFormat};

#[derive(Debug, Clone, Copy)]
pub enum XrdsBufferType {
    Index(IndexFormat),
    Vertex(VertexFormat),
    Uniform,
}

#[derive(Clone)]
pub struct XrdsBuffer {
    inner: wgpu::Buffer,
    ty: XrdsBufferType,
    stride: Option<u64>,
}

impl XrdsBuffer {
    pub fn new(inner: wgpu::Buffer, ty: XrdsBufferType, stride: Option<u64>) -> Self {
        Self { inner, ty, stride }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.inner
    }

    pub fn ty(&self) -> XrdsBufferType {
        self.ty
    }

    pub fn stride(&self) -> u64 {
        self.stride.unwrap_or(match self.ty {
            XrdsBufferType::Index(format) => format.byte_size() as u64,
            XrdsBufferType::Vertex(format) => format.size(),
            XrdsBufferType::Uniform => 16,
        })
    }

    pub fn slice<S>(&self, bounds: S) -> BufferSlice<'_>
    where
        S: RangeBounds<BufferAddress>,
    {
        self.inner.slice(bounds)
    }
}

impl From<XrdsBufferType> for wgpu::BufferUsages {
    fn from(value: XrdsBufferType) -> Self {
        match value {
            XrdsBufferType::Index(_) => wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            XrdsBufferType::Vertex(_) => wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            XrdsBufferType::Uniform => wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        }
    }
}

impl Debug for XrdsBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XrdsBuffer")
            .field("type", &self.ty)
            .field("stride", &self.stride)
            .field("size", &self.inner.size())
            .finish()
    }
}
