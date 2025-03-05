use std::{collections::HashMap, fmt::Debug, sync::Arc};

use crate::XrdsObject;

pub mod loader;

#[derive(Default, Clone)]
pub struct Gltf {
    pub name: String,
    pub scenes: Vec<Arc<XrdsObject>>,
    pub named_scene: HashMap<String, Arc<XrdsObject>>,
    pub default_scene: Option<Arc<XrdsObject>>,
}

impl Gltf {
    pub fn with_scenes(mut self, scene_objects: &[Arc<XrdsObject>]) -> Self {
        self.scenes = scene_objects.to_vec();
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

    pub fn get_default_scene(&self) -> Option<&Arc<XrdsObject>> {
        self.default_scene.as_ref()
    }

    pub fn get_scene_by_name(&self, name: &str) -> Option<&Arc<XrdsObject>> {
        self.named_scene.get(name)
    }

    pub fn get_scene_by_index(&self, index: usize) -> Option<&Arc<XrdsObject>> {
        self.scenes.get(index)
    }

    pub fn get_scenes(&self) -> &[Arc<XrdsObject>] {
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

#[tokio::test]
async fn test_gltf_loader() {
    use crate::{AssetServer, GraphicsInstance};
    use loader::GltfLoader;
    use std::path::PathBuf;
    use std::sync::Arc;

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();
    let buf = PathBuf::from("assets/gltf/StainedGlassLamp/StainedGlassLamp.gltf");
    if !buf.is_file() {
        log::error!("Requested path '{:?}' is not file", buf.as_path());
        return;
    }

    let graphics = GraphicsInstance::new().await;
    let asset_server = Arc::new(AssetServer::new(graphics.clone()).unwrap());
    let gltf_loader = GltfLoader::new(asset_server.clone(), buf.parent().unwrap());

    let _gltf = gltf_loader.load_from_file(buf.as_path()).await.unwrap();
    let _gltf2 = gltf_loader.load_from_file(buf.as_path()).await.unwrap();
}
