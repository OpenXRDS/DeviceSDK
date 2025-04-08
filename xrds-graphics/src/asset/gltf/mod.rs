use std::{collections::HashMap, fmt::Debug};

use uuid::Uuid;

pub mod loader;

#[derive(Debug, Default, Clone)]
pub struct Gltf {
    pub name: String,
    pub scenes: Vec<GltfScene>,
    pub named_scene: HashMap<String, usize>,
    pub default_scene_index: usize,
}

#[derive(Debug, Default, Clone)]
pub struct GltfScene {
    pub name: Option<String>,
    pub id: Uuid,
}

impl Gltf {
    pub fn with_scenes(mut self, scenes: Vec<GltfScene>) -> Self {
        self.scenes = scenes;
        for (i, scene) in self.scenes.iter().enumerate() {
            // First entity is scene entity
            if let Some(name) = &scene.name {
                self.named_scene.insert(name.clone(), i);
            }
        }
        self
    }

    pub fn with_default_scene(mut self, default_scene_index: usize) -> Self {
        self.default_scene_index = default_scene_index;

        self
    }

    pub fn get_default_scene(&self) -> Option<&GltfScene> {
        self.scenes.get(self.default_scene_index)
    }

    pub fn get_scene_by_name(&self, name: &str) -> Option<&GltfScene> {
        self.scenes.get(*self.named_scene.get(name).unwrap_or(&0))
    }

    pub fn get_scene_by_index(&self, index: usize) -> Option<&GltfScene> {
        self.scenes.get(index)
    }

    pub fn get_scenes(&self) -> &[GltfScene] {
        &self.scenes
    }
}
