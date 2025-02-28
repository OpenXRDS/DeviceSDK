use std::{
    collections::HashSet,
    ffi::{CStr, CString},
    os::raw::c_void,
    sync::Arc,
};

use anyhow::Context;
use ash::vk::{self, Handle};
use log::{debug, warn};
use openxr::{
    AnyGraphics, FrameStream, FrameWaiter, Session, SwapchainCreateFlags, SwapchainCreateInfo,
    SwapchainUsageFlags, Vulkan,
};
use wgpu::{BackendOptions, Extent3d, InstanceDescriptor};
use xrds_graphics::{GraphicsInstance, Size2Di, TextureFormat, XrdsTexture};

use crate::{OpenXrError, ViewConfiguration};

use super::{get_projection_views, OpenXrContextApi, OpenXrContextCreateResult};

pub struct OpenXrVulkanContext {
    session: openxr::Session<Vulkan>,
    frame_stream: openxr::FrameStream<Vulkan>,
    swapchain: Option<openxr::Swapchain<Vulkan>>,
    swapchain_format: Option<TextureFormat>,
    swapchain_extent: Option<wgpu::Extent3d>,
}

impl OpenXrVulkanContext {
    /// Return values are not used but openxr need to query vulkan instance extensions and graphics requirements before instantiating
    fn query_xr_instance_extensions(
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
    ) -> anyhow::Result<()> {
        let _ = instance.vulkan_legacy_device_extensions(system_id)?;
        let _ = instance.graphics_requirements::<Vulkan>(system_id)?;
        Ok(())
    }

    fn wgpu_instance() -> wgpu::Instance {
        wgpu::Instance::new(&InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: wgpu::InstanceFlags::empty(),
            backend_options: BackendOptions::from_env_or_default(),
        })
    }

    fn wgpu_hal_instance(wgpu_instance: &wgpu::Instance) -> &wgpu_hal::vulkan::Instance {
        unsafe {
            wgpu_instance
                .as_hal::<wgpu_hal::api::Vulkan>()
                .expect("Could not get wgpu hal instance with Vulkan api")
        }
    }

    fn wgpu_hal_exposed_adapter(
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
        wgpu_hal_instance: &wgpu_hal::vulkan::Instance,
    ) -> anyhow::Result<wgpu_hal::ExposedAdapter<wgpu_hal::api::Vulkan>> {
        let raw_vk_instance = wgpu_hal_instance
            .shared_instance()
            .raw_instance()
            .handle()
            .as_raw() as *const c_void;
        let raw_physical_device =
            unsafe { instance.vulkan_graphics_device(system_id, raw_vk_instance) }?;
        let physical_device = vk::PhysicalDevice::from_raw(raw_physical_device as u64);
        let wgpu_hal_exposed_adapter = wgpu_hal_instance
            .expose_adapter(physical_device)
            .expect("Could not expose adapter");

        Ok(wgpu_hal_exposed_adapter)
    }

    fn required_device_extensions(
        wgpu_hal_exposed_adapter: &wgpu_hal::ExposedAdapter<wgpu_hal::api::Vulkan>,
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
        wgpu_features: wgpu::Features,
    ) -> anyhow::Result<Vec<CString>> {
        let mut extension_set = HashSet::new();

        instance
            .vulkan_legacy_device_extensions(system_id)?
            .split(' ')
            .for_each(|s| {
                extension_set.insert(s.to_owned());
            });

        wgpu_hal_exposed_adapter
            .adapter
            .required_device_extensions(wgpu_features)
            .into_iter()
            .for_each(|cstr| {
                extension_set.insert(String::from(cstr.to_string_lossy()));
            });
        debug!("Required device extensions = {:?}", extension_set);

        let extension_set: Vec<_> = extension_set
            .iter()
            .map(|s| CString::new(s.clone()).unwrap())
            .filter_map(|ext| {
                if wgpu_hal_exposed_adapter
                    .adapter
                    .physical_device_capabilities()
                    .supports_extension(&ext)
                {
                    Some(ext.clone())
                } else {
                    warn!(
                        "Extension {} is not supported",
                        ext.as_c_str().to_string_lossy()
                    );
                    None
                }
            })
            .collect();

        Ok(extension_set
            .into_iter()
            .map(|s| CString::new(s).unwrap())
            .collect())
    }

    fn wgpu_device(
        vk_entry: &ash::Entry,
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
        wgpu_hal_instance: &wgpu_hal::vulkan::Instance,
        wgpu_hal_exposed_adapter: &wgpu_hal::ExposedAdapter<wgpu_hal::api::Vulkan>,
        wgpu_features: wgpu::Features,
        wgpu_memory_hints: &wgpu::MemoryHints,
        device_extensions: &[CString],
    ) -> anyhow::Result<wgpu_hal::OpenDevice<wgpu_hal::api::Vulkan>> {
        let device_extensions_cchar: Vec<_> =
            device_extensions.iter().map(|s| s.as_ptr()).collect();
        let device_extensions_cstr: Vec<_> = unsafe {
            device_extensions_cchar
                .iter()
                .map(|ptr| CStr::from_ptr(*ptr))
                .collect()
        };

        let mut enabled_physical_device_features = wgpu_hal_exposed_adapter
            .adapter
            .physical_device_features(&device_extensions_cstr, wgpu_features);

        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(0)
            .queue_priorities(&[1.0]);
        let queue_create_infos = [queue_create_info];
        let mut physical_device_multiview_features =
            vk::PhysicalDeviceMultiviewFeatures::default().multiview(true);
        let device_create_info = enabled_physical_device_features.add_to_device_create(
            vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&device_extensions_cchar)
                .push_next(&mut physical_device_multiview_features),
        );
        let raw_physical_device = wgpu_hal_exposed_adapter
            .adapter
            .raw_physical_device()
            .as_raw() as *mut c_void;
        let raw_device_ptr = unsafe {
            instance.create_vulkan_device(
                system_id,
                std::mem::transmute::<
                    unsafe extern "system" fn(
                        ash::vk::Instance,
                        *const i8,
                    )
                        -> std::option::Option<unsafe extern "system" fn()>,
                    unsafe extern "system" fn(
                        *const std::ffi::c_void,
                        *const i8,
                    )
                        -> std::option::Option<unsafe extern "system" fn()>,
                >(vk_entry.static_fn().get_instance_proc_addr),
                raw_physical_device,
                &device_create_info as *const _ as *const c_void,
            )
        }?
        .map_err(vk::Result::from_raw)?;

        let raw_device = unsafe {
            ash::Device::load(
                wgpu_hal_instance.shared_instance().raw_instance().fp_v1_0(),
                vk::Device::from_raw(raw_device_ptr as u64),
            )
        };

        let wgpu_hal_open_device = unsafe {
            wgpu_hal_exposed_adapter.adapter.device_from_raw(
                raw_device,
                None,
                &device_extensions_cstr,
                wgpu_features,
                wgpu_memory_hints,
                0,
                0,
            )
        }?;

        Ok(wgpu_hal_open_device)
    }

    fn openxr_session(
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
        wgpu_hal_instance: &wgpu_hal::vulkan::Instance,
        wgpu_hal_exposed_adapter: &wgpu_hal::ExposedAdapter<wgpu_hal::api::Vulkan>,
        wgpu_hal_open_device: &wgpu_hal::OpenDevice<wgpu_hal::api::Vulkan>,
    ) -> anyhow::Result<(Session<Vulkan>, FrameWaiter, FrameStream<Vulkan>)> {
        let (session, frame_watier, frame_stream) = unsafe {
            instance.create_session::<Vulkan>(
                system_id,
                &openxr::vulkan::SessionCreateInfo {
                    instance: wgpu_hal_instance
                        .shared_instance()
                        .raw_instance()
                        .handle()
                        .as_raw() as *const c_void,
                    device: wgpu_hal_open_device.device.raw_device().handle().as_raw()
                        as *const c_void,
                    physical_device: wgpu_hal_exposed_adapter
                        .adapter
                        .raw_physical_device()
                        .as_raw() as *const c_void,
                    queue_family_index: wgpu_hal_open_device.device.queue_family_index(),
                    queue_index: wgpu_hal_open_device.device.queue_index(),
                },
            )
        }
        .context("openxr_session()")?;

        Ok((session, frame_watier, frame_stream))
    }

    fn wrap_wgpu(
        wgpu_instance: &wgpu::Instance,
        wgpu_hal_exposed_adapter: wgpu_hal::ExposedAdapter<wgpu_hal::api::Vulkan>,
        wgpu_hal_open_device: wgpu_hal::OpenDevice<wgpu_hal::api::Vulkan>,
        wgpu_features: wgpu::Features,
        wgpu_limits: wgpu::Limits,
        wgpu_memory_hints: wgpu::MemoryHints,
    ) -> anyhow::Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
        let wgpu_adapter =
            unsafe { wgpu_instance.create_adapter_from_hal(wgpu_hal_exposed_adapter) };
        let (wgpu_device, wgpu_queue) = unsafe {
            wgpu_adapter.create_device_from_hal(
                wgpu_hal_open_device,
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu_features,
                    required_limits: wgpu_limits,
                    memory_hints: wgpu_memory_hints,
                },
                None,
            )
        }?;

        Ok((wgpu_adapter, wgpu_device, wgpu_queue))
    }
}

impl OpenXrContextApi for OpenXrVulkanContext {
    fn create(
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
    ) -> anyhow::Result<OpenXrContextCreateResult<Self>> {
        let vk_entry = unsafe { ash::Entry::load().expect("Could not load vulkan library entry") };

        debug!("OpenXr vulkan context initializing");
        Self::query_xr_instance_extensions(instance, system_id)?;

        let wgpu_instance = Self::wgpu_instance();
        let wgpu_hal_instance = Self::wgpu_hal_instance(&wgpu_instance);
        let wgpu_hal_exposed_adapter =
            Self::wgpu_hal_exposed_adapter(instance, system_id, wgpu_hal_instance)?;
        let wgpu_features = xrds_graphics::required_wgpu_features();
        let wgpu_memory_hints = xrds_graphics::required_wgpu_memory_hints();
        let wgpu_limits = xrds_graphics::required_wgpu_limits();

        let device_extensions = Self::required_device_extensions(
            &wgpu_hal_exposed_adapter,
            instance,
            system_id,
            wgpu_features,
        )?;

        let wgpu_hal_open_device = Self::wgpu_device(
            &vk_entry,
            instance,
            system_id,
            wgpu_hal_instance,
            &wgpu_hal_exposed_adapter,
            wgpu_features,
            &wgpu_memory_hints,
            &device_extensions,
        )?;
        let (session, frame_waiter, frame_stream) = Self::openxr_session(
            instance,
            system_id,
            wgpu_hal_instance,
            &wgpu_hal_exposed_adapter,
            &wgpu_hal_open_device,
        )?;

        let (wgpu_adapter, wgpu_device, wgpu_queue) = Self::wrap_wgpu(
            &wgpu_instance,
            wgpu_hal_exposed_adapter,
            wgpu_hal_open_device,
            wgpu_features,
            wgpu_limits,
            wgpu_memory_hints,
        )?;
        let graphics_instance =
            GraphicsInstance::from_init(wgpu_instance, wgpu_adapter, wgpu_device, wgpu_queue);

        Ok(OpenXrContextCreateResult {
            context: Box::new(Self {
                session,
                frame_stream,
                swapchain: None,
                swapchain_format: None,
                swapchain_extent: None,
            }),
            frame_waiter,
            graphics_instance,
            _phantom: std::marker::PhantomData,
        })
    }

    fn create_swapchain(
        &mut self,
        view_configuration: &ViewConfiguration,
        graphics_instance: Arc<GraphicsInstance>,
    ) -> anyhow::Result<Vec<XrdsTexture>> {
        let swapchain_formats = self.session.enumerate_swapchain_formats()?;
        let swapchain_formats: Vec<TextureFormat> = swapchain_formats
            .iter()
            .map(|f| vk::Format::from_raw(*f as _))
            .filter_map(|vk_format| xrds_graphics::TextureFormat::try_from(vk_format).ok())
            .collect();

        // we prefer Rgba8UnormSrgb
        let swapchain_format =
            if swapchain_formats.contains(&wgpu::TextureFormat::Rgba8UnormSrgb.into()) {
                wgpu::TextureFormat::Rgba8UnormSrgb.into()
            } else {
                swapchain_formats[0]
            };

        let width = view_configuration.views[0].recommended_image_size.width;
        let height = view_configuration.views[0].recommended_image_size.height;
        let sample_count = view_configuration.views[0].recommended_swapchain_sample_count;
        let swapchain = self
            .session
            .create_swapchain(&SwapchainCreateInfo::<Vulkan> {
                create_flags: SwapchainCreateFlags::EMPTY,
                usage_flags: SwapchainUsageFlags::TRANSFER_DST | SwapchainUsageFlags::SAMPLED,
                format: swapchain_format.as_vk().as_raw() as _,
                sample_count,
                width,
                height,
                face_count: 1,
                array_size: view_configuration.views.len() as _,
                mip_count: 1,
            })
            .context("create_swapchain()")?;

        let swapchain_size = Extent3d {
            width,
            height,
            depth_or_array_layers: view_configuration.views.len() as _,
        };

        let mut swapchain_textures = Vec::new();
        for image in swapchain.enumerate_images()?.iter() {
            let raw_image = vk::Image::from_raw(*image);
            let wgpu_texture = graphics_instance.create_texture_from_vk(
                raw_image,
                &wgpu_hal::TextureDescriptor {
                    label: None,
                    size: swapchain_size,
                    mip_level_count: 1,
                    sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: swapchain_format.as_wgpu(),
                    usage: wgpu_hal::TextureUses::COPY_DST | wgpu_hal::TextureUses::RESOURCE,
                    memory_flags: wgpu_hal::MemoryFlags::empty(),
                    view_formats: vec![swapchain_format.as_wgpu()],
                },
            )?;
            let view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor {
                label: None,
                format: Some(swapchain_format.as_wgpu()),
                dimension: Some(if swapchain_size.depth_or_array_layers > 1 {
                    wgpu::TextureViewDimension::D2Array
                } else {
                    wgpu::TextureViewDimension::D2
                }),
                array_layer_count: Some(swapchain_size.depth_or_array_layers),
                mip_level_count: Some(1),
                base_mip_level: 0,
                base_array_layer: 0,
                ..Default::default()
            });
            let swapchain_texture = xrds_graphics::XrdsTexture::new(
                wgpu_texture,
                swapchain_format,
                swapchain_size,
                Some(view),
            );
            swapchain_textures.push(swapchain_texture);
        }

        self.swapchain = Some(swapchain);
        self.swapchain_format = Some(swapchain_format);
        self.swapchain_extent = Some(swapchain_size);
        Ok(swapchain_textures)
    }

    fn stream_begin(&mut self) -> anyhow::Result<()> {
        self.frame_stream.begin()?;
        Ok(())
    }

    fn swapchain_wait(&mut self) -> anyhow::Result<u32> {
        let swapchain = self
            .swapchain
            .as_mut()
            .ok_or(OpenXrError::SwapchainNotInitialized)?;
        let image = swapchain.acquire_image()?;
        swapchain.wait_image(openxr::Duration::INFINITE)?;

        Ok(image)
    }

    fn swapchain_release_image(&mut self) -> anyhow::Result<()> {
        let swapchain = self
            .swapchain
            .as_mut()
            .ok_or(OpenXrError::SwapchainNotInitialized)?;
        swapchain.release_image()?;
        Ok(())
    }

    fn stream_end(
        &mut self,
        display_time: openxr::Time,
        environment_blend_mode: openxr::EnvironmentBlendMode,
        rect: openxr::Rect2Di,
        space: Arc<openxr::Space>,
        views: &[openxr::View],
    ) -> anyhow::Result<()> {
        if views.len() > 0 {
            let projection_views: Vec<_> = {
                let swapchain = self
                    .swapchain
                    .as_ref()
                    .ok_or(OpenXrError::SwapchainNotInitialized)?;

                get_projection_views(rect, views, swapchain)
            };

            self.frame_stream.end(
                display_time,
                environment_blend_mode,
                &[&openxr::CompositionLayerProjection::new()
                    .space(&space)
                    .views(&projection_views)],
            )?;
        } else {
            self.frame_stream
                .end(display_time, environment_blend_mode, &[])?;
        }

        Ok(())
    }

    fn session(&self) -> openxr::Session<AnyGraphics> {
        self.session.clone().into_any_graphics()
    }

    fn swapchain_format(&self) -> anyhow::Result<TextureFormat> {
        Ok(self
            .swapchain_format
            .ok_or(OpenXrError::SwapchainNotInitialized)?)
    }

    fn swapchain_size(&self) -> anyhow::Result<Size2Di> {
        let extent = self.swapchain_extent()?;
        Ok(Size2Di {
            width: extent.width,
            height: extent.height,
        })
    }

    fn swapchain_extent(&self) -> anyhow::Result<wgpu::Extent3d> {
        Ok(self
            .swapchain_extent
            .ok_or(OpenXrError::SwapchainNotInitialized)?)
    }
}
