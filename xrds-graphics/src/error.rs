use std::fmt::Debug;

use crate::XrdsBufferViewType;

pub enum XrdsGraphicsError {
    BufferFormatMismatched {
        ty: XrdsBufferViewType,
        expected: XrdsBufferViewType,
    },
    VertexFormatMismatched {
        fmt: wgpu::VertexFormat,
        expected: wgpu::VertexFormat,
    },
}

impl Debug for XrdsGraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XrdsGraphicsError::BufferFormatMismatched { ty, expected } => {
                write!(
                    f,
                    "Buffer type '{:?}' does not matched expect type '{:?}'",
                    ty, expected
                )
            }
            XrdsGraphicsError::VertexFormatMismatched { fmt, expected } => {
                write!(
                    f,
                    "Vertex format '{:?}' does not match expected format '{:?}",
                    fmt, expected
                )
            }
        }
    }
}
