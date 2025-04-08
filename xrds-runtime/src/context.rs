use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use uuid::Uuid;
use xrds_graphics::{
    loader::GltfLoader, AssetServer, GraphicsInstance, LightSystem, RenderSystem, TransformSystem,
};

use crate::World;

#[derive(Debug)]
pub struct Context {
    graphics_instance: GraphicsInstance,
    asset_server: Arc<RwLock<AssetServer>>,
    current_world: World,
}

impl Context {
    pub(crate) fn new(graphics_instance: GraphicsInstance) -> anyhow::Result<Self> {
        let asset_server = AssetServer::new(graphics_instance.clone())?;
        let transform_system = TransformSystem::new();
        let light_system = LightSystem::new();
        let render_system =
            RenderSystem::new(graphics_instance.clone(), asset_server.clone(), None);
        let world = World::new(
            asset_server.clone(),
            &graphics_instance,
            render_system,
            light_system,
            transform_system,
        );

        Ok(Self {
            graphics_instance,
            current_world: world,
            asset_server,
        })
    }

    pub fn load_objects_from_gltf<P>(&self, gltf_path: P) -> anyhow::Result<Vec<Uuid>>
    where
        P: AsRef<Path>,
    {
        let path = gltf_path.as_ref();
        let loader = GltfLoader::new(self.graphics_instance.clone(), path.parent().unwrap());
        let mut asset_server = self.asset_server.write().unwrap();
        let gltf = futures::executor::block_on(loader.load_from_file(path, &mut asset_server))?;

        Ok(gltf
            .scenes
            .iter()
            .map(|gltf_scene| gltf_scene.id.clone())
            .collect())
    }

    pub fn get_current_world(&self) -> &World {
        &self.current_world
    }

    pub fn get_current_world_mut(&mut self) -> &mut World {
        &mut self.current_world
    }

    pub fn get_asset_server(&self) -> &Arc<RwLock<AssetServer>> {
        &self.asset_server
    }
}
