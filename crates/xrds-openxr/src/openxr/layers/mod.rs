use bevy::prelude::*;

pub mod builder;
pub mod fb;
pub mod khr;
pub mod projection;

pub trait OpenXrLayerBuilder {
    fn build(&self, world: &World) -> Box<dyn OpenXrCompositionLayer>;
}

pub trait OpenXrCompositionLayer {
    fn as_raw(&self) -> &openxr::sys::CompositionLayerBaseHeader;
}
