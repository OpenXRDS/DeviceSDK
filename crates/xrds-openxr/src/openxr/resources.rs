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
    /// Whether `XR_FB_passthrough` was both requested and granted.
    ///
    /// Recorded at instance creation because that is the only point where the
    /// negotiated `ExtensionSet` exists. Passthrough entry points are absent
    /// without it, so `create_passthrough` would fail rather than degrade.
    pub passthrough_supported: bool,
}

/// The live passthrough feature and layer, kept alive for the session.
///
/// Main world only: neither handle is `Clone`, and the render world needs just the
/// layer handle (see [`OpenXrPassthroughLayerHandle`]). Dropping this stops
/// passthrough, so it must outlive every frame that submits the layer.
#[derive(Resource)]
pub struct OpenXrPassthrough {
    #[allow(unused)]
    pub feature: openxr::Passthrough,
    pub layer: openxr::PassthroughLayer,
}

/// The passthrough layer's raw handle, for the render world.
///
/// `PassthroughLayer` is not `Clone`, but its handle is a plain `Copy` id, and the
/// composition layer only needs the id. The owning [`OpenXrPassthrough`] stays in
/// the main world and keeps it valid.
#[derive(Resource, ExtractResource, Clone, Copy)]
pub struct OpenXrPassthroughLayerHandle(pub openxr::sys::PassthroughLayerFB);

/// Whether the scene wants passthrough composited beneath it.
///
/// Driven by `XrdsSceneMetadata::xr_blend_mode`. Separate from
/// [`OpenXrPassthrough`]'s existence because the feature is created once per
/// session when the device supports it, while a scene may switch the effect on
/// and off — and `xrEndFrame` takes its layer list fresh every frame.
///
/// **Not** the same thing as `EnvironmentBlendMode::ALPHA_BLEND`. That enum is a
/// global frame-blend parameter and is deliberately left `OPAQUE`; see
/// `session.rs`'s blend-mode selection.
#[derive(Resource, ExtractResource, Clone, Copy, Default)]
pub struct OpenXrPassthroughEnabled(pub bool);

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
pub struct OpenXrReferenceSpaces(pub Vec<OpenXrReferenceSpace>);

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
