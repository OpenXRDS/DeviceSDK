use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{XrdsObject, XrdsWorldComponent};

use super::XrdsResource;

#[derive(Debug)]
pub struct XrdsWorldInner {
    name: String,
    components: Vec<Arc<RwLock<dyn XrdsWorldComponent>>>,
}

impl XrdsObject for XrdsWorldInner {
    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }
    fn on_construct(&self) {}
    fn on_destroy(&self) {}
}

impl XrdsWorldInner {
    pub fn register(&mut self, component: Arc<RwLock<dyn XrdsWorldComponent>>) {
        self.components.push(component);
    }

    pub fn components(&self) -> &[Arc<RwLock<dyn XrdsWorldComponent>>] {
        &self.components
    }

    pub fn update(&mut self, elapsed: Duration) {
        self.components
            .iter()
            .for_each(|c| c.write().unwrap().update(elapsed));
    }

    pub fn query_resources(&self) -> Vec<XrdsResource> {
        self.components
            .iter()
            .flat_map(|c| c.read().unwrap().query_resources())
            .collect()
    }
}
