use std::{fmt::Debug, ops::Range};

use wgpu::BufferSlice;

use super::XrdsBuffer;

#[derive(Clone)]
pub struct XrdsIndexBuffer {
    pub buffer: XrdsBuffer,
    pub index_format: wgpu::IndexFormat,
    pub offset: usize,
    pub count: usize,
}

impl XrdsIndexBuffer {
    pub fn as_slice(&self) -> BufferSlice<'_> {
        let start = self.offset as u64;
        let end = start + (self.count as u64 * self.buffer.stride());
        self.buffer.slice(start..end)
    }

    pub fn format(&self) -> wgpu::IndexFormat {
        self.index_format
    }

    pub fn as_range(&self) -> Range<u32> {
        0..self.count as u32
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
