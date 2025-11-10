use wgpu::BufferSlice;

#[derive(Debug, Clone)]
pub enum XrdsBufferType {
    Vertex(wgpu::VertexFormat),
    Index(wgpu::IndexFormat),
    Uniform,
}

#[derive(Debug, Clone)]
pub struct XrdsBufferView {
    buffer: wgpu::Buffer,
    offset: u64,
    ty: XrdsBufferType,
    count: u64,
}

impl XrdsBufferView {
    pub fn new(buffer: wgpu::Buffer, offset: u64, ty: XrdsBufferType, count: u64) -> Self {
        Self {
            buffer,
            offset,
            ty,
            count,
        }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn ty(&self) -> &XrdsBufferType {
        &self.ty
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn slice(&self) -> BufferSlice<'_> {
        let unit_size = match self.ty {
            XrdsBufferType::Vertex(vty) => vty.size(),
            XrdsBufferType::Index(ity) => ity.byte_size() as _,
            XrdsBufferType::Uniform => 1,
        };
        self.buffer
            .slice(self.offset..(self.offset + unit_size * self.count))
    }
}
