use std::collections::HashMap;

use glam::Mat4;

use crate::{pbr::PbrMaterial, Transform};

pub mod loader;

#[derive(Debug, Default, Clone)]
pub struct Gltf<'a> {
    pub name: String,
    pub scenes: Vec<GltfScene>,
    pub named_scene: HashMap<String, &'a GltfScene>,
    pub default_scene: Option<&'a GltfScene>,
    pub meshes: Vec<GltfMesh>,
    pub named_meshes: HashMap<String, &'a GltfMesh>,
    // pub material: Vec<GltfMaterial>,
    // pub named_material: HashMap<String, &'a GltfMaterial>,
    pub nodes: Vec<GltfNode>,
    pub named_nodes: HashMap<String, &'a GltfNode>,
    pub skins: Vec<GltfSkin>,
    pub named_skins: HashMap<String, &'a GltfSkin>,
}

#[derive(Debug, Clone)]
pub struct GltfScene {}

#[derive(Debug, Clone)]
pub struct GltfMesh {}

#[derive(Debug, Clone)]
pub struct GltfPrimitive {
    pub index: usize,
    pub name: String,
    pub mesh: GltfMesh,
    pub material: Option<Material>,
    pub extras: Option<String>,
    pub material_extras: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Material {
    material: PbrMaterial,
}

#[derive(Debug, Clone)]
pub struct GltfNode {
    pub index: usize,
    pub name: String,
    pub children: Vec<GltfNode>,
    pub mesh: Vec<GltfMesh>,
    pub skin: Vec<GltfSkin>,
    pub transform: Transform,
    pub is_animation_root: bool,
    pub extras: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GltfSkin {
    pub index: usize,
    pub name: String,
    pub joints: Vec<GltfNode>,
    pub inverse_bind_metrices: Vec<Mat4>,
    pub extras: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GltfImage {
    pub name: String,
    pub index: usize,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct GltfTexture {}

#[tokio::test]
async fn test_gltf_loader() {
    use crate::{AssetServer, GraphicsInstance};
    use loader::GltfLoader;
    use std::path::PathBuf;

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();
    let buf = PathBuf::from("assets/gltf/StainedGlassLamp/StainedGlassLamp.gltf");
    if !buf.is_file() {
        log::error!("Requested path '{:?}' is not file", buf.as_path());
        return;
    }

    let graphics = GraphicsInstance::new().await;
    let asset_server = AssetServer::new(graphics.clone());
    let gltf_loader = GltfLoader::new(asset_server.clone(), buf.parent().unwrap());

    let gltf = gltf_loader.load_from_file(buf.as_path()).await.unwrap();

    println!("{:?}", gltf);
}
