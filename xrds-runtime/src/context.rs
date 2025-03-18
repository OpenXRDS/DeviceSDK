use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use xrds_graphics::{loader::GltfLoader, AssetServer};

use crate::{Object, World};

#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<RwLock<ContextInner>>,
}

impl Context {
    pub(crate) fn new(asset_server: Arc<AssetServer>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ContextInner {
                asset_server,
                current_world: World::new(),
            })),
        }
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
            .map(|scene| Object {
                inner: scene.clone(),
            })
            .collect())
    }

    pub fn get_current_world(&self) -> anyhow::Result<World> {
        let lock = self.inner.read().unwrap();
        lock.get_current_world()
    }
}

#[derive(Debug, Clone)]
struct ContextInner {
    asset_server: Arc<AssetServer>,
    current_world: World,
}

impl ContextInner {
    pub fn get_current_world(&self) -> anyhow::Result<World> {
        Ok(self.current_world.clone())
    }
}
