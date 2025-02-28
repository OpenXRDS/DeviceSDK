use xrds_core::{XrdsComponent, XrdsObject};

use crate::{AssetHandle, XrdsIndexBuffer, XrdsMaterial, XrdsVertexBuffer};

#[derive(Debug, Clone)]
pub struct XrdsMesh {
    pub name: String,
    pub primitives: Vec<XrdsPrimitive>,
}

#[derive(Debug, Clone)]
pub struct XrdsPrimitive {
    pub vertices: Vec<XrdsVertexBuffer>,
    pub indices: Option<XrdsIndexBuffer>,
    pub material: AssetHandle<XrdsMaterial>,
}

impl XrdsMesh {}

impl XrdsComponent for XrdsMesh {
    fn update(&mut self, _elapsed: std::time::Duration) {
        // nothing to do
    }
    fn query_resources(&self) -> Vec<xrds_core::XrdsResource> {
        todo!()
    }
}

impl XrdsObject for XrdsMesh {
    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }
    fn on_construct(&self) {}
    fn on_destroy(&self) {}
}
