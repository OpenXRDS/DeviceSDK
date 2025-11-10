use xrds_core::{XrdsComponent, XrdsObject};

use crate::{XrdsMaterial, XrdsVertexBuffer};

#[derive(Debug, Clone)]
pub struct XrdsStaticMesh {
    name: String,
    vertex: XrdsVertexBuffer,
}

impl XrdsStaticMesh {
    pub fn is_material_supported(&self, material: &XrdsMaterial) -> bool {
        material.is_vertex_supported(&self.vertex)
    }
}

impl XrdsComponent for XrdsStaticMesh {
    fn update(&mut self, _elapsed: std::time::Duration) {
        // nothing to do
    }
    fn query_resources(&self) -> Vec<xrds_core::XrdsResource> {
        todo!()
    }
}

impl XrdsObject for XrdsStaticMesh {
    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }
    fn on_construct(&self) {}
    fn on_destroy(&self) {}
}
