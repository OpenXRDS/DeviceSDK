mod shadowmaps;

pub use shadowmaps::*;

use crate::Entity;

#[derive(Debug, Clone)]
pub struct LightSystem {}

impl LightSystem {
    pub fn new() -> Self {
        Self {}
    }
    pub fn update(&mut self, entities: &mut Vec<Entity>) {}
}
