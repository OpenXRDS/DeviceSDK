pub mod pbr;

use crate::{DiscreteVertex, LinearVertex, XrdsVertexBuffer, XrdsVertexInputType};

#[derive(Debug, Clone)]
pub struct XrdsMaterial {
    pipeline: wgpu::RenderPipeline,
    vertex_input_type: XrdsVertexInputType,
}

impl XrdsMaterial {
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn vertex_input_type(&self) -> &XrdsVertexInputType {
        &self.vertex_input_type
    }

    pub fn is_vertex_supported(&self, vertex: &XrdsVertexBuffer) -> bool {
        match vertex {
            XrdsVertexBuffer::Discrete(v) => self.is_discrete_vertex_supported(v),
            XrdsVertexBuffer::Linear(v) => self.is_linear_vertex_supported(v),
        }
    }

    pub fn is_discrete_vertex_supported(&self, vertex: &DiscreteVertex) -> bool {
        self.vertex_input_type == *vertex.vertex_input_type()
    }

    pub fn is_linear_vertex_supported(&self, vertex: &LinearVertex) -> bool {
        self.vertex_input_type == *vertex.vertex_input_type()
    }
}
