use bevy::{prelude::*, render::extract_resource::ExtractResource};

use crate::openxr::{graphics::OpenXrGraphicsWrap, layers::builder::OpenXrCompositionLayerBuilder};

#[derive(Resource, ExtractResource, Default, Clone)]
pub struct OpenXrViews(pub Vec<openxr::View>);

#[derive(ExtractResource, Resource, Clone)]
pub struct OpenXrFrameState(pub openxr::FrameState);

#[derive(Resource)]
pub struct OpenXrFrameStream(pub OpenXrGraphicsWrap<Self>);

#[derive(Resource, Clone)]
pub struct OpenXrInstance {
    pub instance: openxr::Instance,
    pub system_id: openxr::SystemId,
}

#[derive(Resource, ExtractResource, Clone)]
pub struct OpenXrViewConfigurations {
    pub view_configuration_type: openxr::ViewConfigurationType,
    pub view_configuration_views: Vec<openxr::ViewConfigurationView>,
}

#[derive(Resource, ExtractResource, Clone)]
pub struct OpenXrEnvironmentBlendModes {
    #[allow(unused)]
    pub blend_modes: Vec<openxr::EnvironmentBlendMode>,
    pub current_blend_mode: openxr::EnvironmentBlendMode,
}

#[derive(Clone)]
pub struct OpenXrSpace(pub u64);

#[allow(unused)]
#[derive(Resource, ExtractResource, Clone)]
pub struct OpenXrReferenceSpace(pub OpenXrSpace);

#[derive(Resource, ExtractResource, Clone)]
pub struct OpenXrPrimaryReferenceSpace(pub OpenXrSpace);

#[allow(unused)]
#[derive(Resource)]
pub struct OpenXrRefrerenceSpaces(pub Vec<OpenXrReferenceSpace>);

#[derive(Resource)]
pub struct OpenXrRenderResources {
    pub frame_stream: OpenXrFrameStream,
    pub swapchain: OpenXrSwapchain,
    pub layer_builder: OpenXrCompositionLayerBuilder,
}

#[derive(Resource)]
pub struct OpenXrSwapchain(pub OpenXrGraphicsWrap<Self>);

#[derive(ExtractResource, Resource, Clone)]
pub struct OpenXrSwapchainImages(pub Vec<(wgpu::Texture, Vec<wgpu::TextureView>)>);

#[derive(ExtractResource, Resource, Clone)]
pub struct OpenXrSwapchainInfo {
    pub format: wgpu::TextureFormat,
    pub size: wgpu::Extent3d,
}
