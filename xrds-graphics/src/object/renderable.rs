use std::{fmt::Debug, ops::Range};

use glam::{Quat, Vec3};
use xrds_core::Transform;

use crate::RenderPass;

use super::XrdsMesh;

#[derive(Default, Clone)]
pub struct Renderable {
    name: String,
    childs: Vec<Renderable>,
    transform: Transform,
    mesh: Option<XrdsMesh>,
}

impl Renderable {
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Create new object with its transfrom.
    /// If object has parent, transform must be pre-calculated global transform
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_childs(mut self, childs: &[Renderable]) -> Self {
        self.childs = childs.to_vec();
        self
    }

    pub fn add_child(&mut self, child: Renderable) {
        self.childs.push(child);
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_childs(&self) -> &[Renderable] {
        &self.childs
    }

    pub fn get_transform(&self) -> &Transform {
        &self.transform
    }

    pub fn translate(&mut self, translation: Vec3) {
        self.transform.translate(translation);
    }

    pub fn scale(&mut self, scale: Vec3) {
        self.transform.scale(scale);
    }

    pub fn rotate(&mut self, rotation: Quat) {
        self.transform.rotate(rotation);
    }

    pub fn get_mesh(&self) -> Option<&XrdsMesh> {
        self.mesh.as_ref()
    }

    pub fn set_mesh(&mut self, mesh: XrdsMesh) {
        self.mesh = Some(mesh);
    }

    pub fn encode(&self, render_pass: &mut RenderPass, instances: &Range<u32>) {
        if let Some(mesh) = &self.mesh {
            mesh.encode(render_pass, &self.transform, instances);
        }
        for child in &self.childs {
            child.encode(render_pass, instances);
        }
    }
}

impl Debug for Renderable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("XrdsObject");

        debug_struct
            .field("name", &self.name)
            .field("transform", &self.transform);
        if let Some(mesh) = &self.mesh {
            debug_struct.field("mesh", mesh);
        }
        if !self.childs.is_empty() {
            debug_struct.field("childs", &self.childs);
        }
        debug_struct.finish()
    }
}
