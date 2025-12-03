use bevy::prelude::*;

use openxr::{
    sys::{CompositionLayerProjection, SwapchainSubImage},
    CompositionLayerFlags, Fovf, Posef, StructureType,
};

use crate::openxr::{
    layers::{OpenXrCompositionLayer, OpenXrLayerBuilder},
    resources::{
        OpenXrPrimaryReferenceSpace, OpenXrSpace, OpenXrSwapchain, OpenXrSwapchainInfo, OpenXrViews,
    },
};

#[derive(Clone)]
pub struct OpenXrCompositionLayerProjectionView {
    inner: openxr::sys::CompositionLayerProjectionView,
}

#[derive(Clone)]
pub struct OpenXrCompositionLayerProjection {
    pub inner: openxr::sys::CompositionLayerProjection,
    pub views: Vec<openxr::sys::CompositionLayerProjectionView>,
}

#[derive(Clone)]
pub struct OpenXrCompositionLayerProjectionBuilder;

impl OpenXrCompositionLayer for OpenXrCompositionLayerProjection {
    fn as_raw(&self) -> &openxr::sys::CompositionLayerBaseHeader {
        unsafe {
            #[allow(clippy::missing_transmute_annotations)]
            std::mem::transmute(&self.inner)
        }
    }
}

impl OpenXrCompositionLayerProjection {
    pub fn new() -> Self {
        Self {
            inner: CompositionLayerProjection {
                ty: StructureType::COMPOSITION_LAYER_PROJECTION,
                ..unsafe { std::mem::zeroed() }
            },
            views: vec![],
        }
    }

    pub fn layer_flags(mut self, layer_flags: CompositionLayerFlags) -> Self {
        self.inner.layer_flags = layer_flags;
        self
    }

    pub fn space(mut self, space: &OpenXrSpace) -> Self {
        self.inner.space = openxr::sys::Space::from_raw(space.0);
        self
    }

    pub fn views(mut self, views: &[OpenXrCompositionLayerProjectionView]) -> Self {
        self.views = views.iter().map(|v| v.inner).collect();
        self.inner.view_count = self.views.len() as _;
        self.inner.views = self.views.as_ptr();
        self
    }
}

impl OpenXrCompositionLayerProjectionView {
    pub fn new() -> Self {
        Self {
            inner: openxr::sys::CompositionLayerProjectionView {
                ty: StructureType::COMPOSITION_LAYER_PROJECTION_VIEW,
                ..unsafe { std::mem::zeroed() }
            },
        }
    }

    pub fn pose(mut self, pose: Posef) -> Self {
        self.inner.pose = pose;
        self
    }

    pub fn fov(mut self, fov: Fovf) -> Self {
        self.inner.fov = fov;
        self
    }

    pub fn sub_image(mut self, sub_image: SwapchainSubImage) -> Self {
        self.inner.sub_image = sub_image;
        self
    }
}

impl OpenXrLayerBuilder for OpenXrCompositionLayerProjectionBuilder {
    fn build(&self, world: &bevy::ecs::world::World) -> Box<dyn OpenXrCompositionLayer> {
        let reference_space = world.resource::<OpenXrPrimaryReferenceSpace>();
        let views = world.resource::<OpenXrViews>();
        let swapchain = world.resource::<OpenXrSwapchain>();
        let swapchain_info = world.resource::<OpenXrSwapchainInfo>();
        let raw_swapchain = swapchain.as_raw();

        let rects: Vec<_> = (0..swapchain_info.size.depth_or_array_layers)
            .map(|_| openxr::Rect2Di {
                offset: openxr::Offset2Di { x: 0, y: 0 },
                extent: openxr::Extent2Di {
                    width: swapchain_info.size.width as i32,
                    height: swapchain_info.size.height as i32,
                },
            })
            .collect();
        trace!("layer rects={:?}", rects);

        let projection_views: Vec<_> = views
            .0
            .iter()
            .enumerate()
            .map(|(i, view)| {
                trace!("view#{} fov={:?}, pose={:?}", i, view.fov, view.pose);
                OpenXrCompositionLayerProjectionView::new()
                    .fov(view.fov)
                    .pose(view.pose)
                    .sub_image(SwapchainSubImage {
                        swapchain: raw_swapchain,
                        image_rect: rects[i],
                        image_array_index: i as u32,
                    })
            })
            .collect();

        Box::new(
            OpenXrCompositionLayerProjection::new()
                .layer_flags(CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
                .space(&reference_space.0)
                .views(&projection_views),
        )
    }
}
