use std::{ops::Range, sync::Arc};

use uuid::Uuid;
use xrds_core::Transform;
use xrds_graphics::{RenderPass, Renderable};

#[derive(Debug, Clone)]
pub struct Object {
    uuid: Uuid,
    inner: Arc<Renderable>,
}

#[derive(Debug, Default, Clone)]
pub struct State {
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct SpawnedObject {
    spawn_id: Uuid,
    object_id: Uuid,
    transform: Transform,
    state: State,
}

impl Object {
    pub fn new(inner: Arc<Renderable>) -> Self {
        Self {
            inner,
            uuid: uuid::Uuid::new_v4(),
        }
    }

    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    pub(crate) fn encode(
        &self,
        render_pass: &mut RenderPass,
        instances: &Range<u32>,
    ) -> anyhow::Result<()> {
        self.inner.encode(render_pass, instances);

        Ok(())
    }
}

impl SpawnedObject {
    pub fn new(spawn_id: Uuid, object_id: Uuid, transform: Transform) -> Self {
        Self {
            spawn_id,
            object_id,
            transform,
            state: State::default(),
        }
    }

    pub fn spawn_id(&self) -> &Uuid {
        &self.spawn_id
    }

    pub fn object_id(&self) -> &Uuid {
        &self.object_id
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
