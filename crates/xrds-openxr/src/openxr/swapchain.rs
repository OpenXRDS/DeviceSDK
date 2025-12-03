use crate::{
    backends::OpenXrGraphicsBackends,
    openxr::{
        graphics::{
            openxr_graphics, OpenXrGraphicsExtend, OpenXrGraphicsFamily, OpenXrGraphicsWrap,
        },
        resources::{
            OpenXrSwapchain, OpenXrSwapchainImages, OpenXrSwapchainInfo, OpenXrViewConfigurations,
        },
        schedule::{OpenXrRuntimeSystems, OpenXrSchedules},
        session::OpenXrSession,
    },
};
use bevy::prelude::*;
use openxr::{Duration, SwapchainCreateInfo};
use wgpu::{wgt::TextureViewDescriptor, Extent3d};

pub struct OpenXrSwapchainCreateInfo(pub OpenXrGraphicsWrap<Self>);
impl OpenXrGraphicsFamily for OpenXrSwapchainCreateInfo {
    type Inner<G: OpenXrGraphicsExtend> = SwapchainCreateInfo<G>;
}
impl OpenXrSwapchainCreateInfo {
    pub fn from_inner<G: OpenXrGraphicsExtend>(
        swapchain_create_info: SwapchainCreateInfo<G>,
    ) -> Self {
        Self(G::wrap(swapchain_create_info))
    }

    pub fn as_inner<G: OpenXrGraphicsExtend>(&self) -> &SwapchainCreateInfo<G> {
        openxr_graphics!(
            &self.0;
            inner => {
                let old: *const _ = inner;
                let ptr: *const SwapchainCreateInfo<G> = old.cast();
                unsafe { ptr.as_ref().unwrap() }
            }
        )
    }
}

impl OpenXrGraphicsFamily for OpenXrSwapchain {
    type Inner<G: OpenXrGraphicsExtend> = openxr::Swapchain<G>;
}

impl OpenXrSwapchain {
    pub fn from_inner<G: OpenXrGraphicsExtend>(session: openxr::Swapchain<G>) -> Self {
        Self(G::wrap(session))
    }

    pub fn wait_image(&mut self, timeout: Duration) -> openxr::Result<()> {
        openxr_graphics! {
            &mut self.0;
            inner => inner.wait_image(timeout)
        }
    }

    pub fn acquire_image(&mut self) -> openxr::Result<u32> {
        openxr_graphics! {
            &mut self.0;
            inner => inner.acquire_image()
        }
    }

    pub fn release_image(&mut self) -> openxr::Result<()> {
        openxr_graphics! {
            &mut self.0;
            inner => inner.release_image()
        }
    }

    pub fn as_raw(&self) -> openxr::sys::Swapchain {
        openxr_graphics! {
            &self.0;
            inner => inner.as_raw()
        }
    }
}

pub struct OpenXrSwapchainPlugin;

impl Plugin for OpenXrSwapchainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OpenXrSchedules::SessionCreate,
            create_swapchain.in_set(OpenXrRuntimeSystems::PostSessionCreate),
        );
    }
}

fn create_swapchain(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");

    // Must exists
    let session = world
        .get_resource::<OpenXrSession>()
        .expect("OpenXrSession resource not exists");
    let graphics_backends = world
        .get_resource::<OpenXrGraphicsBackends>()
        .expect("OpenXrGraphicsBackends resource not exists");
    let view_configurations = world
        .get_resource::<OpenXrViewConfigurations>()
        .expect("OpenXrViewConfigurations resource not exists");

    let view_configuration_view = view_configurations
        .view_configuration_views
        .first()
        .expect("View configuration views is empty");

    let size = Extent3d {
        width: view_configuration_view.recommended_image_rect_width,
        height: view_configuration_view.recommended_image_rect_height,
        depth_or_array_layers: 2, // TODO: Check stereo
    };

    let sample_count = view_configuration_view.recommended_swapchain_sample_count;

    let (swapchain, swapchain_images, swapchain_info) = openxr_graphics!(
        &session.0;
        session => {
            let swapchain_formats: Vec<_> = session.enumerate_swapchain_formats()
                .expect("Could not enumerate swapchain formats")
                .iter()
                .filter_map(|f| graphics_backends.format_from_raw::<Api>(f) )
                .collect();
            info!("Available swapchain formats: {:?}", swapchain_formats);

            let swapchain_format = wgpu::TextureFormat::Rgba8UnormSrgb;  // TODO: Select format from list. Prior Srgb
            info!("Selected swapchain format: {:?}", swapchain_format);
            let swapchain_create_info = graphics_backends.get_swapchain_create_info(swapchain_format, size, sample_count)
                .expect("Could not get swapchain create info");
            let swapchain = session.create_swapchain(swapchain_create_info.as_inner::<Api>())
                .expect("Could not create swapchain");

            let images = swapchain.enumerate_images()
                .expect("Could not enumerate swapchain images");
            let swapchain_images: Vec<_> = images
                .iter()
                .map(|image| {
                    let swapchain_image = graphics_backends.swapchain_image_to_wgpu::<Api>(image, swapchain_format, size, sample_count)
                        .expect("Could not create wgpu texture from swapchain image");

                    // Bevy not support Multiview feature in version 0.17. So we'll create texture view for each layer
                    let swapchain_image_views: Vec<_> = (0..size.depth_or_array_layers).map(|i| {
                        swapchain_image.create_view(&TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            array_layer_count: Some(1),
                            base_array_layer: i,
                            ..Default::default()
                        })
                    }).collect();
                    (swapchain_image, swapchain_image_views)
                }
                )
                .collect();

            let info = OpenXrSwapchainInfo {
                format: swapchain_format,
                size
            };

            (
                OpenXrSwapchain::from_inner(swapchain),
                OpenXrSwapchainImages(swapchain_images),
                info
            )
        }
    );

    world.insert_resource(swapchain);
    world.insert_resource(swapchain_images);
    world.insert_resource(swapchain_info);
    info!("OpenXR swapchain and swapchain images initialized");
}

const OPENXR_SWAPCHAIN_VIEW_INDEX_BASE: u32 = u32::from_le_bytes(*b"OPXR");

pub fn view_index(view_index: u32) -> u32 {
    OPENXR_SWAPCHAIN_VIEW_INDEX_BASE + view_index
}
