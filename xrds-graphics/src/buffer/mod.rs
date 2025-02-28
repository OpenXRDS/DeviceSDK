mod buffer_view;

pub use buffer_view::*;

#[derive(Debug, Clone, Copy)]
pub enum XrdsBufferType {
    Index,
    Vertex,
    Uniform,
}

#[derive(Debug, Clone)]
pub struct XrdsBuffer {
    inner: wgpu::Buffer,
    ty: XrdsBufferType,
    stride: u64,
}

#[derive(Debug, Clone)]
pub struct XrdsVertexBuffer {
    pub buffer: XrdsBuffer,
    pub vertex_attributes: [wgpu::VertexAttribute; 1], // Currently support discreted vertex buffer only
}

#[derive(Debug, Clone)]
pub struct XrdsIndexBuffer {
    pub buffer: XrdsBuffer,
    pub index_format: wgpu::VertexFormat,
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
