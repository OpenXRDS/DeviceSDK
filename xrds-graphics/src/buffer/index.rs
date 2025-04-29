use std::{fmt::Debug, ops::Range};

use wgpu::BufferSlice;

use super::XrdsBuffer;

#[derive(Debug, Clone, Copy)]
pub enum IndexFormat {
    U8,
    U16,
    U32,
}

#[derive(Clone)]
pub struct XrdsIndexBuffer {
    pub buffer: XrdsBuffer,
    pub index_format: IndexFormat,
    pub offset: usize,
    pub count: usize,
}

impl XrdsIndexBuffer {
    pub fn as_slice(&self) -> BufferSlice<'_> {
        let start = self.offset as u64;
        let end = start + (self.count as u64 * self.buffer.stride());
        log::trace!("Bind index buffer {} - {}", start, end);
        self.buffer.slice(start..end)
    }

    pub fn format(&self) -> IndexFormat {
        self.index_format
    }

    pub fn format_as_wgpu(&self) -> Option<wgpu::IndexFormat> {
        self.index_format.as_wgpu()
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

impl IndexFormat {
    pub fn as_wgpu(&self) -> Option<wgpu::IndexFormat> {
        match self {
            IndexFormat::U16 => Some(wgpu::IndexFormat::Uint16),
            IndexFormat::U32 => Some(wgpu::IndexFormat::Uint32),
            _ => None,
        }
    }

    pub fn byte_size(&self) -> usize {
        match self {
            IndexFormat::U8 => 1,
            IndexFormat::U16 => 2,
            IndexFormat::U32 => 4,
        }
    }
}
