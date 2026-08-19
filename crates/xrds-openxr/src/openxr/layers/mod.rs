use bevy::prelude::*;

pub mod builder;
pub mod fb;
pub mod khr;
pub mod projection;

pub trait OpenXrLayerBuilder {
    /// Build this layer for the current frame, or `None` to omit it.
    ///
    /// `None` exists for layers that are registered once but only submitted some
    /// frames — passthrough is registered when the device supports it and
    /// submitted only while a scene asks for it. `xrEndFrame` takes a fresh layer
    /// list every frame, so omitting is the correct way to switch one off;
    /// there is no "disabled layer" to hand the runtime.
    fn build(&self, world: &World) -> Option<Box<dyn OpenXrCompositionLayer>>;
}

pub trait OpenXrCompositionLayer {
    fn as_raw(&self) -> &openxr::sys::CompositionLayerBaseHeader;
}
