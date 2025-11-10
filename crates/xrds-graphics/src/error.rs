use std::fmt::Debug;

use crate::XrdsBufferType;

pub enum XrdsGraphicsError {
    BufferTypeMismatched {
        ty: XrdsBufferType,
        expected: XrdsBufferType,
    },
    VertexFormatMismatched {
        fmt: wgpu::VertexFormat,
        expected: wgpu::VertexFormat,
    },
}

impl Debug for XrdsGraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XrdsGraphicsError::BufferTypeMismatched { ty, expected } => {
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
