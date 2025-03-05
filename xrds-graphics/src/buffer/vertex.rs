use crate::XrdsGraphicsError;

use super::{XrdsBuffer, XrdsBufferView, XrdsBufferViewType};

#[derive(Debug, Clone, Default)]
pub struct XrdsVertexInputType {
    pub normal: bool,
    pub tangent: bool,
    pub texcoords_n: u32,
    pub colors_n: u32,
    pub joints_n: u32,
    pub weights_n: u32,
}

#[derive(Debug, Clone)]
pub struct DiscreteVertex {
    position: XrdsBufferView,
    normal: Option<XrdsBufferView>,
    tangent: Option<XrdsBufferView>,
    texcoords: Vec<XrdsBufferView>,
    colors: Vec<XrdsBufferView>,
    joints: Vec<XrdsBufferView>,
    weights: Vec<XrdsBufferView>,
    vertex_input_type: XrdsVertexInputType,
    vertex_attributes: Vec<wgpu::VertexAttribute>,
}

#[derive(Debug, Clone)]
pub struct LinearVertex {
    vertices: XrdsBufferView,
    vertex_input_type: XrdsVertexInputType,
}

impl PartialEq<XrdsVertexInputType> for XrdsVertexInputType {
    fn eq(&self, other: &XrdsVertexInputType) -> bool {
        (self.normal == other.normal)
            && (self.tangent == other.tangent)
            && (self.texcoords_n == other.texcoords_n)
            && (self.colors_n == other.colors_n)
            && (self.joints_n == other.joints_n)
            && (self.weights_n == other.weights_n)
    }
}
