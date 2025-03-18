use std::sync::{Arc, RwLock};

use xrds_graphics::RenderPass;

use crate::Object;

#[derive(Debug, Clone)]
pub struct World {
    objects: Arc<RwLock<Vec<Object>>>,
}

impl World {
    pub(crate) fn new() -> Self {
        Self {
            objects: Arc::new(RwLock::new(vec![])),
        }
    }

    pub(crate) fn encode(&self, render_pass: &mut RenderPass) -> anyhow::Result<()> {
        let lock = self.objects.read().unwrap();

        for object in lock.iter() {
            object.encode(render_pass)?;
        }
        Ok(())
    }

    pub fn spawn(&self, objects: &[Object]) -> anyhow::Result<()> {
        let mut lock = self.objects.write().unwrap();

        objects.iter().for_each(|o| lock.push(o.clone()));
        Ok(())
    }
}
