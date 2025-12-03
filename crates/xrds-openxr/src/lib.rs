use bevy::{
    app::PluginGroupBuilder,
    prelude::*,
    render::{pipelined_rendering::PipelinedRenderingPlugin, RenderPlugin},
    state::app::StatesPlugin,
};

pub(crate) mod backends;
pub(crate) mod openxr;
#[cfg(target_os = "windows")]
mod windows;

pub use openxr::OpenXrCamera;

use crate::openxr::{
    camera::OpenXrCameraPlugin, init::OpenXrInitPlugin,
    reference_space::OpenXrReferenceSpacePlugin, render::OpenXrRenderPlugin,
    session::OpenXrSessionPlugin, swapchain::OpenXrSwapchainPlugin,
};

pub fn add_plugins<PG: PluginGroup>(base_plugins: PG, app_name: String) -> PluginGroupBuilder {
    let plugin_builder = base_plugins
        .build()
        .disable::<RenderPlugin>()
        .disable::<StatesPlugin>()
        .disable::<PipelinedRenderingPlugin>()
        .add_before::<RenderPlugin>(StatesPlugin)
        .add_before::<RenderPlugin>(OpenXrInitPlugin {
            app_name,
            ..Default::default()
        })
        .add(OpenXrSessionPlugin)
        .add(OpenXrReferenceSpacePlugin)
        .add(OpenXrSwapchainPlugin)
        .add(OpenXrCameraPlugin)
        .add(OpenXrRenderPlugin);

    #[cfg(feature = "preview_window")]
    let plugin_builder = {
        plugin_builder.set(WindowPlugin {
            primary_window: Some(Window {
                transparent: true,
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                ..Default::default()
            }),
            ..Default::default()
        })
    };

    plugin_builder
}
