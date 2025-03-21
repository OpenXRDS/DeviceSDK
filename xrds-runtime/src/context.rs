use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use xrds_graphics::{loader::GltfLoader, AssetServer, GraphicsInstance};

use crate::{Object, World};

#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<RwLock<ContextInner>>,
}

impl Context {
    pub(crate) fn new(graphics_instance: Arc<GraphicsInstance>) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(ContextInner {
                graphics_instance: graphics_instance.clone(),
                asset_server: Arc::new(AssetServer::new(graphics_instance.clone())?),
                current_world: World::new(graphics_instance.clone(), 10000usize),
            })),
        })
    }

    pub fn load_objects_from_gltf<P>(&self, gltf_path: P) -> anyhow::Result<Vec<Object>>
    where
        P: AsRef<Path>,
    {
        let asset_server = {
            let lock = self.inner.read().unwrap();
            lock.asset_server.clone()
        };
        let path = gltf_path.as_ref();
        let loader = GltfLoader::new(asset_server, path.parent().unwrap());
        let gltf = futures::executor::block_on(loader.load_from_file(path))?;

        Ok(gltf
            .scenes
            .iter()
            .map(|gltf_scene| Object::new(gltf_scene.clone()))
            .collect())
    }

    pub fn get_current_world(&self) -> World {
        let lock = self.inner.read().unwrap();
        lock.get_current_world()
    }
}

#[derive(Debug)]
pub struct ContextInner {
    graphics_instance: Arc<GraphicsInstance>,
    asset_server: Arc<AssetServer>,
    current_world: World,
}

impl ContextInner {
    pub fn get_current_world(&self) -> World {
        self.current_world.clone()
    }

    pub fn get_graphics_instance(&self) -> &Arc<GraphicsInstance> {
        &self.graphics_instance
    }
}
