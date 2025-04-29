use std::{fmt::Debug, ops::Range};

use wgpu::BufferSlice;

use super::XrdsBuffer;

#[derive(Clone)]
pub struct XrdsVertexBuffer {
    pub buffer: XrdsBuffer,
    pub vertex_attributes: [wgpu::VertexAttribute; 1], // Currently support discreted vertex buffer only
    pub offset: usize,
    pub count: usize,
}

impl XrdsVertexBuffer {
    pub fn slice(&self) -> BufferSlice<'_> {
        let start = self.offset as u64;
        let end = start + (self.count as u64 * self.buffer.stride());
        log::trace!("Bind vertex buffer {} - {}", start, end);
        self.buffer.slice(start..end)
    }

    pub fn as_range(&self) -> Range<u32> {
        0..self.count as u32
    }
}

impl Debug for XrdsVertexBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XrdsVertexBuffer")
            .field("buffer", &self.buffer)
            .finish()
    }
}
