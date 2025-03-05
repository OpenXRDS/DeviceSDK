mod buffer_view;

use std::fmt::Debug;

pub use buffer_view::*;
use wgpu::BufferSlice;

#[derive(Debug, Clone, Copy)]
pub enum XrdsBufferType {
    Index,
    Vertex,
    Uniform,
}

#[derive(Clone)]
pub struct XrdsBuffer {
    inner: wgpu::Buffer,
    ty: XrdsBufferType,
    stride: u64,
}

#[derive(Clone)]
pub struct XrdsVertexBuffer {
    pub buffer: XrdsBuffer,
    pub vertex_attributes: [wgpu::VertexAttribute; 1], // Currently support discreted vertex buffer only
}

#[derive(Clone)]
pub struct XrdsIndexBuffer {
    pub buffer: XrdsBuffer,
    pub index_format: wgpu::IndexFormat,
}

impl XrdsBuffer {
    pub fn new(inner: wgpu::Buffer, ty: XrdsBufferType, stride: u64) -> Self {
        Self { inner, ty, stride }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.inner
    }

    pub fn ty(&self) -> XrdsBufferType {
        self.ty
    }

    pub fn stride(&self) -> u64 {
        self.stride
    }

    pub fn as_slice(&self) -> BufferSlice<'_> {
        self.inner.slice(..)
    }
}

impl XrdsVertexBuffer {
    pub fn as_slice(&self) -> BufferSlice<'_> {
        self.buffer.as_slice()
    }
}

impl XrdsIndexBuffer {
    pub fn as_slice(&self) -> BufferSlice<'_> {
        self.buffer.as_slice()
    }

    pub fn format(&self) -> wgpu::IndexFormat {
        self.index_format
    }
}

impl From<XrdsBufferType> for wgpu::BufferUsages {
    fn from(value: XrdsBufferType) -> Self {
        match value {
            XrdsBufferType::Index => wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            XrdsBufferType::Vertex => wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
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

impl Debug for XrdsVertexBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XrdsVertexBuffer")
            .field("buffer", &self.buffer)
            .finish()
    }
}

impl Debug for XrdsIndexBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XrdsIndexBuffer")
            .field("buffer", &self.buffer)
            .field("index_format", &self.index_format)
            .finish()
    }
}
