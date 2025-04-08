use crate::ObjectInstance;

#[derive(Debug, Clone)]
pub struct TransformSystem {}

impl TransformSystem {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, object_instances: &[ObjectInstance]) {}
}
