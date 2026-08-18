use crate::{
    default_component_name, TransformParams, XrdsAssetComponent, XrdsComponent,
    XrdsMutableComponent, XrdsObject,
};

#[derive(Debug, Clone)]
pub struct XrdsGltfAsset {
    pub name: String,
    pub transform: TransformParams,
    pub visible: bool,
    pub gltf_asset_path: String,
    pub scene_index: usize,
}

impl XrdsGltfAsset {
    pub fn new(gltf_asset_path: impl Into<String>) -> Self {
        Self {
            name: default_component_name::<Self>(),
            transform: TransformParams::default(),
            visible: true,
            gltf_asset_path: gltf_asset_path.into(),
            scene_index: 0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl XrdsObject for XrdsGltfAsset {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsGltfAsset {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsGltfAsset {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl XrdsAssetComponent for XrdsGltfAsset {
    fn asset_path(&self) -> &str {
        &self.gltf_asset_path
    }

    fn scene_index(&self) -> usize {
        self.scene_index
    }
}
