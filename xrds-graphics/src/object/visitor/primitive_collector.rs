use std::{collections::HashMap, error::Error, fmt::Display};

use uuid::Uuid;

use crate::{AssetId, AssetServer, Entity, XrdsPrimitive};

use super::Visitor;

/// Contains primitive and it's root entity's id for instancing
pub struct MappedPrimitive {
    pub primitive: XrdsPrimitive,
    pub root_entity_id: Uuid,
}

pub struct PrimitiveCollector {
    primitive_map: HashMap<AssetId, Vec<MappedPrimitive>>,
}

impl PrimitiveCollector {
    pub fn new() -> Self {
        Self {
            primitive_map: HashMap::new(),
        }
    }

    pub fn primitives(&self) -> &HashMap<AssetId, Vec<MappedPrimitive>> {
        &self.primitive_map
    }
}

#[derive(Debug)]
pub enum PrimitiveCollectorError {}

impl Visitor<PrimitiveCollectorError> for PrimitiveCollector {
    fn visit(
        &mut self,
        entity: &Entity,
        asset_server: &AssetServer,
    ) -> Result<(), PrimitiveCollectorError> {
        // found root entity id
        // Not need to visit child. So we don't need implement new visitor
        let mut current_entity = entity;
        while let Some(transform_component) = current_entity.get_transform_component() {
            if let Some(parent_id) = &transform_component.parent {
                current_entity = asset_server.get_entity(parent_id).unwrap();
            } else {
                // No parent: it's Root!
                break;
            }
        }
        let root_entity_id = *current_entity.id();

        if let Some(mesh_component) = entity.get_mesh_component() {
            let mesh = mesh_component.mesh();
            for primitive in mesh.primitives() {
                if let Some(primitives) =
                    self.primitive_map.get_mut(primitive.material_handle().id())
                {
                    primitives.push(MappedPrimitive {
                        primitive: primitive.clone(),
                        root_entity_id,
                    });
                } else {
                    self.primitive_map.insert(
                        primitive.material_handle().id().clone(),
                        vec![MappedPrimitive {
                            primitive: primitive.clone(),
                            root_entity_id,
                        }],
                    );
                }
            }
        }
        Ok(())
    }
}

impl Error for PrimitiveCollectorError {}
impl Display for PrimitiveCollectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}
