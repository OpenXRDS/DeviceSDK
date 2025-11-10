use std::marker::PhantomData;

use bevy::{
    ecs::resource::Resource,
    math::Mat4,
    render::settings::{RenderResources, WgpuSettings},
};
use openxr::{Graphics, SwapchainCreateInfo};
use wgpu::Extent3d;

use crate::openxr::{
    graphics::{openxr_graphics, OpenXrGraphicsExtend, OpenXrGraphicsFamily, OpenXrGraphicsWrap},
    session::OpenXrSessionCreateInfo,
    swapchain::OpenXrSwapchainCreateInfo,
};

#[cfg(target_os = "windows")]
pub mod d3d12;
pub mod opengl;
pub mod vulkan;

pub trait OpenXrGraphicsBackend<G: Graphics> {
    /// Initialize graphics backend from OpenXR
    fn initialize(
        openxr_instance: &openxr::Instance,
        system_id: openxr::SystemId,
        openxr_appinfo: &openxr::ApplicationInfo,
        wgpu_settings: WgpuSettings,
    ) -> anyhow::Result<OpenXrGraphicsBackends>;

    /// Get bevy RenderResources from graphics backend
    fn get_render_resource(&self) -> anyhow::Result<RenderResources>;

    /// Get OpenXrSessionCreateInfo from graphics backend
    fn get_session_create_info(&self) -> anyhow::Result<OpenXrSessionCreateInfo>;

    /// Get SwapchainCreateInfo from graphics backend
    fn get_swapchain_create_info(
        &self,
        format: wgpu::TextureFormat,
        size: Extent3d,
        sample_count: u32,
    ) -> anyhow::Result<SwapchainCreateInfo<G>>;

    /// Convert graphics backend swapchain image to wgpu Texture
    fn swapchain_image_to_wgpu(
        &self,
        swapchain_image: &G::SwapchainImage,
        format: wgpu::TextureFormat,
        size: Extent3d,
        sample_count: u32,
    ) -> anyhow::Result<wgpu::Texture>;

    fn format_from_raw(&self, format: &G::Format) -> Option<wgpu::TextureFormat>;

    fn calculate_projection_matrix(&self, near: f32, fov: openxr::Fovf) -> Mat4;
}

#[derive(Clone)]
pub struct GraphicsInner<G: Graphics> {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    adapter_info: wgpu::AdapterInfo,
    device: wgpu::Device,
    queue: wgpu::Queue,
    _phantom: PhantomData<G>,
}

#[derive(Resource)]
pub struct OpenXrGraphicsBackends(pub OpenXrGraphicsWrap<Self>);
impl OpenXrGraphicsFamily for OpenXrGraphicsBackends {
    type Inner<G: OpenXrGraphicsExtend> = GraphicsInner<G>;
}
impl OpenXrGraphicsBackends {
    pub fn from_inner<G: OpenXrGraphicsExtend>(inner: GraphicsInner<G>) -> Self {
        Self(G::wrap(inner))
    }
}

impl OpenXrGraphicsBackends {
    pub fn get_render_resources(&self) -> anyhow::Result<RenderResources> {
        openxr_graphics!(
            &self.0;
            inner => inner.get_render_resource()
        )
    }

    pub fn get_session_create_info(&self) -> anyhow::Result<OpenXrSessionCreateInfo> {
        openxr_graphics!(
            &self.0;
            inner => inner.get_session_create_info()
        )
    }

    pub fn get_swapchain_create_info(
        &self,
        swapchain_format: wgpu::TextureFormat,
        size: Extent3d,
        sample_count: u32,
    ) -> anyhow::Result<OpenXrSwapchainCreateInfo> {
        let swapchain_create_info = openxr_graphics!(
            &self.0;
            inner => {
                OpenXrSwapchainCreateInfo::from_inner(inner.get_swapchain_create_info(swapchain_format, size, sample_count)?)
            }
        );

        Ok(swapchain_create_info)
    }

    pub fn swapchain_image_to_wgpu<G: openxr::Graphics>(
        &self,
        swapchain_image: &G::SwapchainImage,
        swapchain_format: wgpu::TextureFormat,
        size: Extent3d,
        sample_count: u32,
    ) -> anyhow::Result<wgpu::Texture> {
        openxr_graphics!(
            &self.0;
            inner => {
                let swapchain_image_ptr: *const _ = swapchain_image;
                let swapchain_casted_ptr: *const <Api as openxr::Graphics>::SwapchainImage = swapchain_image_ptr.cast();
                let swapchain_image = unsafe { swapchain_casted_ptr.as_ref().unwrap() };
                inner.swapchain_image_to_wgpu(swapchain_image, swapchain_format, size, sample_count)
            }
        )
    }

    pub fn format_from_raw<G: openxr::Graphics>(
        &self,
        format: &G::Format,
    ) -> Option<wgpu::TextureFormat> {
        openxr_graphics!(
            &self.0;
            inner => {
                let format_ptr: *const _ = format;
                let casted_ptr: *const <Api as openxr::Graphics>::Format = format_ptr.cast();
                let format_ref = unsafe { casted_ptr.as_ref().unwrap() };
                inner.format_from_raw(format_ref)
            }
        )
    }

    pub fn calculate_projection_matrix(&self, near: f32, fov: openxr::Fovf) -> Mat4 {
        openxr_graphics!(
            &self.0;
            inner => inner.calculate_projection_matrix(near, fov)
        )
    }
}
