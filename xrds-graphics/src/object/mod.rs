mod entity;
mod light;
mod mesh;
mod visitor;

pub use entity::*;
pub use light::*;
pub use mesh::*;
pub use visitor::*;

use uuid::Uuid;
use xrds_core::Transform;

#[derive(Debug, Default, Clone)]
pub struct State {
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct ObjectData {
    entity_id: Uuid,
    transform: Transform,
    state: State,
}

#[derive(Debug, Clone)]
pub struct ObjectInstance {
    spawn_id: Uuid,
}

impl ObjectData {
    pub fn new(entity_id: Uuid, transform: Transform, state: State) -> Self {
        Self {
            entity_id,
            transform,
            state,
        }
    }

    pub fn entity_id(&self) -> &Uuid {
        &self.entity_id
    }

    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    pub fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }
}

impl ObjectInstance {
    pub fn new(spawn_id: Uuid) -> Self {
        Self { spawn_id }
    }

    pub fn spawn_id(&self) -> &Uuid {
        &self.spawn_id
    }
}
