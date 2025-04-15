use std::{fmt::Debug, ops::Range};

use wgpu::{RenderPass, ShaderStages};
use xrds_core::Transform;

use crate::{AssetHandle, Constant, XrdsIndexBuffer, XrdsMaterialInstance, XrdsVertexBuffer};

#[derive(Debug, Default, Clone)]
pub struct XrdsMesh {
    name: String,
    primitives: Vec<XrdsPrimitive>,
}

#[derive(Debug, Clone)]
pub struct XrdsPrimitive {
    pub vertices: Vec<XrdsVertexBuffer>,
    pub indices: Option<XrdsIndexBuffer>,
    pub material: AssetHandle<XrdsMaterialInstance>,
    pub position_index: Option<usize>,
}

impl XrdsMesh {
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn with_primitives(mut self, primitives: Vec<XrdsPrimitive>) -> Self {
        self.primitives = primitives;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
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
        render_pass: &mut RenderPass<'_>,
        transform: &Transform,
        instances: &Range<u32>,
    ) {
        for primitive in &self.primitives {
            primitive.encode(render_pass, transform, instances.clone());
        }
    }
}

impl XrdsPrimitive {
    pub fn has_geometry(&self) -> bool {
        self.position_index.is_some()
    }

    pub fn material_handle(&self) -> &AssetHandle<XrdsMaterialInstance> {
        &self.material
    }

    pub fn vertices(&self) -> &[XrdsVertexBuffer] {
        &self.vertices
    }

    pub fn indices(&self) -> Option<&XrdsIndexBuffer> {
        self.indices.as_ref()
    }

    pub fn encode(
        &self,
        render_pass: &mut RenderPass<'_>,
        transform: &Transform,
        instances: Range<u32>,
    ) {
        if self.position_index.is_none() {
            log::warn!("Primitive has no geometry. Skip encode primitive");
            return;
        }
        render_pass.set_push_constants(
            ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&transform.to_model_array()),
        );
        self.vertices.iter().enumerate().for_each(|(i, v)| {
            render_pass.set_vertex_buffer(i as u32 + Constant::VERTEX_ID_BASEMENT, v.slice());
        });
        if let Some(indices) = self.indices.as_ref() {
            render_pass.set_index_buffer(indices.as_slice(), indices.format());
            render_pass.draw_indexed(
                indices.as_range(),
                0, /* all vertex buffers has same count */
                instances,
            );
        } else {
            render_pass.draw(self.vertices[0].as_range(), instances);
        }
    }

    pub fn encode_geometry(
        &self,
        render_pass: &mut RenderPass<'_>,
        transform: &Transform,
        instances: Range<u32>,
    ) {
        if self.position_index.is_none() {
            log::warn!("Primitive has no geometry. Skip encode primitive");
            return;
        }
        render_pass.set_push_constants(
            ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&transform.to_model_array()),
        );
        render_pass.set_vertex_buffer(
            Constant::VERTEX_ID_BASEMENT,
            self.vertices[self.position_index.unwrap() /* must be exists */].slice(),
        );
        if let Some(indices) = self.indices.as_ref() {
            render_pass.set_index_buffer(indices.as_slice(), indices.format());
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
