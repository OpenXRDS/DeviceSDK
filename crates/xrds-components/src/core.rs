use std::collections::HashMap;

use bevy::prelude::*;

use crate::TransformParams;

/// Initialises SDK-level resources. Add to your Bevy `App` before spawning any XRDS components.
pub struct XrdsComponentsPlugin;

impl Plugin for XrdsComponentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<XrdsRegistry>();
    }
}

/// Bidirectional map between stable [`XrdsId`] blueprint identities and live Bevy [`Entity`] handles.
///
/// Used by every GUI panel interaction: tree selection, inspector edits, scene picking,
/// reparenting, deletion, undo/redo, and serialisation.
#[derive(Resource, Default)]
pub struct XrdsRegistry {
    id_to_entity: HashMap<XrdsId, Entity>,
    entity_to_id: HashMap<Entity, XrdsId>,
}

impl XrdsRegistry {
    /// Register a newly spawned entity. Overwrites any previous mapping for the same id.
    pub fn register(&mut self, id: XrdsId, entity: Entity) {
        self.id_to_entity.insert(id, entity);
        self.entity_to_id.insert(entity, id);
    }

    /// Remove all mappings for a despawned entity.
    pub fn unregister(&mut self, entity: Entity) {
        if let Some(id) = self.entity_to_id.remove(&entity) {
            self.id_to_entity.remove(&id);
        }
    }

    /// Look up the live Bevy entity for a blueprint node.
    pub fn entity(&self, id: XrdsId) -> Option<Entity> {
        self.id_to_entity.get(&id).copied()
    }

    /// Look up the blueprint identity for a live Bevy entity.
    pub fn id(&self, entity: Entity) -> Option<XrdsId> {
        self.entity_to_id.get(&entity).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct XrdsId(pub u64);

pub fn default_component_name<T>() -> String {
    std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .unwrap_or("Component")
        .strip_prefix("Xrds")
        .unwrap_or("Component")
        .to_owned()
}

pub trait XrdsObject {
    fn name(&self) -> &str;
    fn is_enabled(&self) -> bool {
        true
    }
    fn is_visible(&self) -> bool {
        true
    }
}

pub trait XrdsComponent: XrdsObject {
    fn local_transform(&self) -> &TransformParams;

    fn local_transform_mut(&mut self) -> &mut TransformParams;
}

pub trait XrdsMutableComponent: XrdsComponent {
    fn set_name(&mut self, name: String);

    fn set_visible(&mut self, visible: bool);
}

pub trait XrdsAssetComponent: XrdsComponent {
    fn asset_path(&self) -> &str;

    fn scene_index(&self) -> usize {
        0
    }
}

pub trait XrdsActor: XrdsComponent {
    fn on_interact(&mut self) {}
}
