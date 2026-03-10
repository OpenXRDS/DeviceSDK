use bevy::prelude::*;

use crate::openxr::layers::{OpenXrCompositionLayer, OpenXrLayerBuilder};

#[derive(Resource)]
pub struct OpenXrCompositionLayerBuilder {
    layers: Vec<Box<dyn OpenXrLayerBuilder + Send + Sync>>,
}

impl OpenXrCompositionLayerBuilder {
    pub fn new() -> Self {
        Self { layers: vec![] }
    }

    pub fn insert_layer(&mut self, index: usize, layer: Box<dyn OpenXrLayerBuilder + Send + Sync>) {
        self.layers.insert(index, layer);
    }

    pub fn build(&self, world: &World) -> Vec<Box<dyn OpenXrCompositionLayer>> {
        self.layers.iter().map(|layer| layer.build(world)).collect()
    }
}
