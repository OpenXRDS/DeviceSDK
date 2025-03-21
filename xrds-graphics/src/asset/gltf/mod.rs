use std::{collections::HashMap, fmt::Debug, sync::Arc};

use crate::Renderable;

pub mod loader;

#[derive(Default, Clone)]
pub struct Gltf {
    pub name: String,
    pub scenes: Vec<Arc<Renderable>>,
    pub named_scene: HashMap<String, Arc<Renderable>>,
    pub default_scene: Option<Arc<Renderable>>,
}

impl Gltf {
    pub fn with_scenes(mut self, scene_objects: Vec<Arc<Renderable>>) -> Self {
        self.scenes = scene_objects;
        for scene in &self.scenes {
            self.named_scene
                .insert(scene.get_name().to_string(), scene.clone());
        }
        self
    }

    pub fn with_default_scene(mut self, default_scene_index: usize) -> Self {
        if let Some(scene) = self.scenes.get(default_scene_index) {
            self.default_scene = Some(scene.clone());
        }
        self
    }

    pub fn get_default_scene(&self) -> Option<&Arc<Renderable>> {
        self.default_scene.as_ref()
    }

    pub fn get_scene_by_name(&self, name: &str) -> Option<&Arc<Renderable>> {
        self.named_scene.get(name)
    }

    pub fn get_scene_by_index(&self, index: usize) -> Option<&Arc<Renderable>> {
        self.scenes.get(index)
    }

    pub fn get_scenes(&self) -> &[Arc<Renderable>] {
        &self.scenes
    }
}

impl Debug for Gltf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("Gltf");

        debug_struct.field("name", &self.name);
        if self.default_scene.is_some() && self.scenes.len() > 1 {
            debug_struct.field("default_scene", &self.default_scene);
        }

        debug_struct.field("scenes", &self.scenes).finish()
    }
}
