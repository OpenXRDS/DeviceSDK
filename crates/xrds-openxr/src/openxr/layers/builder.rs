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

    /// Every layer that wants to be submitted this frame, in registration order.
    ///
    /// Order is the compositing order: index 0 is furthest back. Passthrough is
    /// inserted at 0 so it sits beneath the projection layer.
    pub fn build(&self, world: &World) -> Vec<Box<dyn OpenXrCompositionLayer>> {
        self.layers
            .iter()
            .filter_map(|layer| layer.build(world))
            .collect()
    }
}
