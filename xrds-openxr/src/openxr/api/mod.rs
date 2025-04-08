use std::sync::Arc;

use openxr::SwapchainSubImage;
use xrds_graphics::{GraphicsInstance, TextureFormat, XrdsTexture};

pub mod vulkan;

use super::ViewConfiguration;

pub(crate) struct OpenXrContextCreateResult<T: OpenXrContextApi> {
    pub(crate) context: Box<dyn OpenXrContextApi>,
    pub(crate) frame_waiter: openxr::FrameWaiter,
    pub(crate) graphics_instance: GraphicsInstance,
    pub(crate) _phantom: std::marker::PhantomData<T>,
}

pub(crate) trait OpenXrContextApi {
    fn create(
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
    ) -> anyhow::Result<OpenXrContextCreateResult<Self>>
    where
        Self: Sized;
    fn create_swapchain(
        &mut self,
        view_configuration: &ViewConfiguration,
        graphics_instance: &GraphicsInstance,
    ) -> anyhow::Result<Vec<XrdsTexture>>;
    fn swapchain_wait(&mut self) -> anyhow::Result<u32>;
    fn swapchain_release_image(&mut self) -> anyhow::Result<()>;
    fn stream_begin(&mut self) -> anyhow::Result<()>;
    fn stream_end(
        &mut self,
        display_time: openxr::Time,
        environment_blend_mode: openxr::EnvironmentBlendMode,
        rect: openxr::Rect2Di,
        space: Arc<openxr::Space>,
        views: &[openxr::View],
    ) -> anyhow::Result<()>;

    fn session(&self) -> openxr::Session<openxr::AnyGraphics>;

    fn swapchain_format(&self) -> anyhow::Result<TextureFormat>;
    fn swapchain_extent(&self) -> anyhow::Result<wgpu::Extent3d>;
}

pub(crate) fn get_projection_views<'a, G: openxr::Graphics>(
    rect: openxr::Rect2Di,
    views: &[openxr::View],
    swapchain: &'a openxr::Swapchain<G>,
) -> Vec<openxr::CompositionLayerProjectionView<'a, G>> {
    views
        .iter()
        .enumerate()
        .map(|(i, v)| {
            openxr::CompositionLayerProjectionView::new()
                .pose(v.pose)
                .fov(v.fov)
                .sub_image(
                    SwapchainSubImage::new()
                        .swapchain(swapchain)
                        .image_array_index(i as _)
                        .image_rect(rect),
                )
        })
        .collect()
}
