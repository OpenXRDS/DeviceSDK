use std::{fmt::Debug, ops::Range};

use wgpu::ShaderStages;
use xrds_core::Transform;

use crate::{RenderPass, XrdsIndexBuffer, XrdsMaterialInstance, XrdsVertexBuffer};

#[derive(Default, Clone)]
pub struct XrdsMesh {
    name: String,
    primitives: Vec<XrdsPrimitive>,
}

#[derive(Clone)]
pub struct XrdsPrimitive {
    pub vertices: Vec<XrdsVertexBuffer>,
    pub indices: Option<XrdsIndexBuffer>,
    pub material: XrdsMaterialInstance,
}

impl XrdsMesh {
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_owned();
        self
    }

    pub fn with_primitives(mut self, primitives: Vec<XrdsPrimitive>) -> Self {
        self.primitives = primitives;
        self
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_primitives(&mut self, primitives: Vec<XrdsPrimitive>) {
        self.primitives = primitives;
    }

    pub fn add_primitive(&mut self, primitive: XrdsPrimitive) {
        self.primitives.push(primitive);
    }

    pub fn primitives(&self) -> &[XrdsPrimitive] {
        &self.primitives
    }

    pub fn primitives_mut(&mut self) -> &mut [XrdsPrimitive] {
        &mut self.primitives
    }

    pub fn encode(
        &self,
        render_pass: &mut RenderPass,
        transform: &Transform,
        instances: &Range<u32>,
    ) {
        for primitive in &self.primitives {
            primitive.encode(render_pass, transform, instances.clone());
        }
    }
}

impl XrdsPrimitive {
    pub fn encode(
        &self,
        render_pass: &mut RenderPass,
        transform: &Transform,
        instances: Range<u32>,
    ) {
        render_pass.bind_material(&self.material);
        render_pass.set_push_constants(
            ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&transform.to_model_array()),
        );
        render_pass.bind_vertex_buffers(&self.vertices, 0);
        if let Some(indices) = self.indices.as_ref() {
            render_pass.bind_index_buffer(indices);
            render_pass.draw_indexed(
                indices.as_range(),
                0, /* all vertex buffers has same count */
                instances,
            );
        } else {
            render_pass.draw(self.vertices[0].as_range(), instances);
        }
    }
}

impl Debug for XrdsMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XrdsMesh")
            .field("name", &self.name)
            .field("primitives", &self.primitives)
            .finish()
    }
}

impl Debug for XrdsPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XrdsPrimitive")
            .field("vertices", &self.vertices)
            .field("indices", &self.indices)
            .field("material", &self.material)
            .finish()
    }
}
