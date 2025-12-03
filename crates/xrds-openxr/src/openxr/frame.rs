use bevy::prelude::*;
use openxr::{CompositionLayerBase, EnvironmentBlendMode};

use crate::openxr::{
    graphics::{openxr_graphics, OpenXrGraphicsExtend, OpenXrGraphicsFamily},
    layers::OpenXrCompositionLayer,
    resources::OpenXrFrameStream,
};

impl OpenXrGraphicsFamily for OpenXrFrameStream {
    type Inner<G: OpenXrGraphicsExtend> = openxr::FrameStream<G>;
}

impl OpenXrFrameStream {
    pub fn from_inner<G: OpenXrGraphicsExtend>(frame_stream: openxr::FrameStream<G>) -> Self {
        Self(G::wrap(frame_stream))
    }

    #[inline]
    pub fn begin(&mut self) -> openxr::Result<()> {
        openxr_graphics!(
            &mut self.0;
            inner => inner.begin()
        )
    }

    #[inline]
    pub fn end(
        &mut self,
        display_time: openxr::Time,
        environment_blend_mode: EnvironmentBlendMode,
        layers: &[&dyn OpenXrCompositionLayer],
    ) -> openxr::Result<()> {
        openxr_graphics!(
            &mut self.0;
            inner => {
                let openxr_layers: Vec<&CompositionLayerBase<'_, Api>> = layers
                    .iter()
                    .map(|l|
                        unsafe {
                            #[allow(clippy::missing_transmute_annotations)]
                            std::mem::transmute(l.as_raw())
                        }
                    )
                    .collect();
                inner.end(display_time, environment_blend_mode, openxr_layers.as_slice())
            }
        )
    }
}

#[derive(Resource)]
pub struct OpenXrFrameWaiter(pub openxr::FrameWaiter);

impl OpenXrFrameWaiter {
    pub fn from_inner(frame_waiter: openxr::FrameWaiter) -> Self {
        Self(frame_waiter)
    }

    #[inline]
    pub fn wait(&mut self) -> openxr::Result<openxr::FrameState> {
        self.0.wait()
    }

    #[inline]
    #[allow(dead_code)]
    pub fn wait_secondary(
        &mut self,
    ) -> openxr::Result<(openxr::FrameState, openxr::SecondaryViewState)> {
        self.0.wait_secondary()
    }

    #[inline]
    #[allow(dead_code)]
    pub fn wait_secondary_multiple(
        &mut self,
        count: u32,
    ) -> openxr::Result<(openxr::FrameState, Vec<openxr::SecondaryViewState>)> {
        self.0.wait_secondary_multiple(count)
    }
}
