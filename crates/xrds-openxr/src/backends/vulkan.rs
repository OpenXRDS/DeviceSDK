use std::{
    ffi::{c_void, CString},
    marker::PhantomData,
    sync::Arc,
};

use anyhow::anyhow;
use ash::vk::{DeviceQueueCreateInfo, Handle};
use bevy::prelude::*;
use bevy::render::{
    renderer::{
        RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue, WgpuWrapper,
    },
    settings::{RenderResources, WgpuSettings},
};
use openxr::{Instance, SystemId};
use wgpu::{
    AstcBlock, AstcChannel, DeviceType, Extent3d, Features, Limits, MemoryBudgetThresholds,
};
use wgpu_hal::MemoryFlags;

use crate::{
    backends::{GraphicsInner, OpenXrGraphicsBackend, OpenXrGraphicsBackends},
    openxr::{
        graphics::{OpenXrGraphicsExtend, OpenXrGraphicsFamily, OpenXrGraphicsWrap},
        session::OpenXrSessionCreateInfo,
    },
};

unsafe impl OpenXrGraphicsExtend for openxr::Vulkan {
    fn wrap<G: OpenXrGraphicsFamily>(inner: G::Inner<Self>) -> OpenXrGraphicsWrap<G> {
        OpenXrGraphicsWrap::Vulkan(inner)
    }
}

impl GraphicsInner<openxr::Vulkan> {}

impl OpenXrGraphicsBackend<openxr::Vulkan> for GraphicsInner<openxr::Vulkan> {
    fn initialize(
        openxr_instance: &Instance,
        system_id: SystemId,
        openxr_appinfo: &openxr::ApplicationInfo,
        wgpu_settings: WgpuSettings,
    ) -> anyhow::Result<OpenXrGraphicsBackends> {
        let _span = debug_span!("xrds-openxr::vulkan::initialize");
        let vk_entry = unsafe { ash::Entry::load()? };

        let api_version = get_api_version(openxr_instance, system_id)?;
        let instance_extensions = wgpu_hal::vulkan::Instance::desired_extensions(
            &vk_entry,
            api_version,
            wgpu_settings.instance_flags,
        )?;
        let device_extensions = [
            ash::khr::swapchain::NAME,
            ash::khr::draw_indirect_count::NAME,
            ash::khr::timeline_semaphore::NAME,
            ash::khr::imageless_framebuffer::NAME,
            ash::khr::image_format_list::NAME,
            #[cfg(target_os = "macos")]
            ash::khr::portability_subset::NAME,
            #[cfg(target_os = "macos")]
            ash::ext::metal_objects::NAME,
        ];

        let instance_extensions_cchar: Vec<_> =
            instance_extensions.iter().map(|s| s.as_ptr()).collect();

        let app_name = CString::new(openxr_appinfo.application_name.to_owned())?;
        let engine_name = CString::new(openxr_appinfo.engine_name.to_owned())?;
        let application_info = ash::vk::ApplicationInfo::default()
            .api_version(api_version)
            .application_name(&app_name)
            .application_version(openxr_appinfo.application_version)
            .engine_name(&engine_name)
            .engine_version(openxr_appinfo.engine_version);

        // Create vulkan instance from OpenXR instance
        let raw_instance = unsafe {
            openxr_instance.create_vulkan_instance(
                system_id,
                #[allow(clippy::missing_transmute_annotations)]
                std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                &ash::vk::InstanceCreateInfo::default()
                    .application_info(&application_info)
                    .enabled_extension_names(&instance_extensions_cchar) as *const _
                    as *const c_void,
            )
        }?
        .map_err(ash::vk::Result::from_raw)?;
        // Create ash instance from raw vulkan instance
        let vk_instance = unsafe {
            ash::Instance::load(
                vk_entry.static_fn(),
                ash::vk::Instance::from_raw(raw_instance as u64),
            )
        };
        // Get vulkan physical device from OpenXR instance
        let vk_physical_device = unsafe {
            ash::vk::PhysicalDevice::from_raw(
                openxr_instance
                    .vulkan_graphics_device(system_id, vk_instance.handle().as_raw() as _)?
                    as _,
            )
        };

        let android_sdk_version = get_android_sdk_version();
        let has_nv_optimus = get_has_nv_optimus(&vk_entry)?;

        // Create ash physical device from raw vulkan physical device
        let phyiscal_device_properties =
            unsafe { vk_instance.get_physical_device_properties(vk_physical_device) };
        info!(
            "OpenXR runtime device: {}, api_version: {}",
            phyiscal_device_properties
                .device_name_as_c_str()?
                .to_str()?,
            phyiscal_device_properties.api_version
        );
        // Create wgpu hal instance from ash instance
        let wgpu_hal_instance = unsafe {
            wgpu_hal::vulkan::Instance::from_raw(
                vk_entry.clone(),
                vk_instance.clone(),
                api_version,
                android_sdk_version,
                None,
                instance_extensions,
                wgpu_settings.instance_flags,
                MemoryBudgetThresholds::default(),
                has_nv_optimus,
                None,
            )
        }?;

        // Create exposed wgpu adapter from ash physical device
        let wgpu_exposed_adapter = wgpu_hal_instance
            .expose_adapter(vk_physical_device)
            .expect("Could not expose raw physical device");

        let (limits, features) =
            get_limits_and_features_from_adapter(&wgpu_exposed_adapter, &wgpu_settings)?;
        trace!("Limits: {:?}", limits);
        trace!("Features: {:?}", features);

        let enabled_device_extensions = wgpu_exposed_adapter
            .adapter
            .required_device_extensions(wgpu_exposed_adapter.features);
        debug!(
            "required device extensions: {:?}",
            enabled_device_extensions
        );
        let device_extensions_cchar: Vec<_> =
            device_extensions.iter().map(|s| s.as_ptr()).collect();
        let mut enabled_physical_device_features = wgpu_exposed_adapter
            .adapter
            .physical_device_features(&enabled_device_extensions, features);

        let queue_family_index = 0;

        let device_queue_create_info = DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&[1.0]);
        let queue_create_infos = [device_queue_create_info];
        let mut physical_device_multiview_features =
            ash::vk::PhysicalDeviceMultiviewFeatures::default().multiview(true);

        let device_create_info = enabled_physical_device_features
            .add_to_device_create(
                ash::vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queue_create_infos)
                    .push_next(&mut physical_device_multiview_features),
            )
            .enabled_extension_names(&device_extensions_cchar);

        let raw_device = unsafe {
            openxr_instance.create_vulkan_device(
                system_id,
                #[allow(clippy::missing_transmute_annotations)]
                std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                vk_physical_device.as_raw() as _,
                &device_create_info as *const _ as *const _,
            )
        }?
        .map_err(ash::vk::Result::from_raw)?;
        let vk_device = unsafe {
            ash::Device::load(
                vk_instance.fp_v1_0(),
                ash::vk::Device::from_raw(raw_device as u64),
            )
        };
        let wgpu_open_device = unsafe {
            wgpu_exposed_adapter.adapter.device_from_raw(
                vk_device,
                None,
                &enabled_device_extensions,
                features,
                &wgpu_settings.memory_hints,
                queue_family_index,
                0,
            )
        }?;

        let wgpu_instance =
            unsafe { wgpu::Instance::from_hal::<wgpu_hal::api::Vulkan>(wgpu_hal_instance) };
        let wgpu_adapter = unsafe { wgpu_instance.create_adapter_from_hal(wgpu_exposed_adapter) };
        let limits = wgpu_adapter.limits();

        let (wgpu_device, wgpu_queue) = unsafe {
            wgpu_adapter.create_device_from_hal(
                wgpu_open_device,
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: features,
                    required_limits: limits,
                    memory_hints: wgpu_settings.memory_hints,
                    trace: wgpu::Trace::Off,
                },
            )
        }?;
        let wgpu_adapter_info = wgpu_adapter.get_info();

        let inner = GraphicsInner::<openxr::Vulkan> {
            device: wgpu_device,
            queue: wgpu_queue,
            adapter: wgpu_adapter,
            adapter_info: wgpu_adapter_info,
            instance: wgpu_instance,
            _phantom: PhantomData,
        };

        Ok(OpenXrGraphicsBackends::from_inner(inner))
    }

    fn get_render_resource(&self) -> anyhow::Result<RenderResources> {
        Ok(RenderResources(
            RenderDevice::new(WgpuWrapper::new(self.device.clone())),
            RenderQueue(Arc::new(WgpuWrapper::new(self.queue.clone()))),
            RenderAdapterInfo(WgpuWrapper::new(self.adapter_info.clone())),
            RenderAdapter(Arc::new(WgpuWrapper::new(self.adapter.clone()))),
            RenderInstance(Arc::new(WgpuWrapper::new(self.instance.clone()))),
        ))
    }

    fn get_session_create_info(&self) -> anyhow::Result<OpenXrSessionCreateInfo> {
        let hal_instance = unsafe { self.instance.as_hal::<wgpu_hal::api::Vulkan>() }.unwrap();
        let raw_instance = hal_instance
            .shared_instance()
            .raw_instance()
            .handle()
            .as_raw() as _;
        let hal_adapter = unsafe { self.adapter.as_hal::<wgpu_hal::api::Vulkan>() }.unwrap();
        let raw_physical_device = hal_adapter.raw_physical_device().as_raw() as _;
        let hal_device = unsafe { self.device.as_hal::<wgpu_hal::api::Vulkan>() }.unwrap();
        let raw_device = hal_device.raw_device().handle().as_raw() as _;

        Ok(OpenXrSessionCreateInfo::from_inner::<openxr::Vulkan>(
            openxr::vulkan::SessionCreateInfo {
                instance: raw_instance,
                physical_device: raw_physical_device,
                device: raw_device,
                queue_family_index: hal_device.queue_family_index(),
                queue_index: hal_device.queue_index(),
            },
        ))
    }

    fn get_swapchain_create_info(
        &self,
        format: wgpu::TextureFormat,
        size: Extent3d,
        sample_count: u32,
    ) -> anyhow::Result<openxr::SwapchainCreateInfo<openxr::Vulkan>> {
        Ok(openxr::SwapchainCreateInfo::<openxr::Vulkan> {
            create_flags: openxr::SwapchainCreateFlags::EMPTY,
            usage_flags: openxr::SwapchainUsageFlags::COLOR_ATTACHMENT
                | openxr::SwapchainUsageFlags::SAMPLED,
            sample_count,
            width: size.width,
            height: size.height,
            format: format_wgpu_to_vk(format)
                .expect("Invalid texture format")
                .as_raw() as _,
            face_count: 1,
            array_size: 2,
            mip_count: 1,
        })
    }

    fn swapchain_image_to_wgpu(
        &self,
        swapchain_image: &<openxr::Vulkan as openxr::Graphics>::SwapchainImage,
        format: wgpu::TextureFormat,
        size: Extent3d,
        sample_count: u32,
    ) -> anyhow::Result<wgpu::Texture> {
        debug_span!("swapchain_image_to_wgpu");
        let vk_image = ash::vk::Image::from_raw(*swapchain_image);

        let texture = unsafe {
            let hal_device = self
                .device
                .as_hal::<wgpu_hal::api::Vulkan>()
                .expect("Could not get hal device");
            let hal_texture = hal_device.texture_from_raw(
                vk_image,
                &wgpu_hal::TextureDescriptor {
                    label: Some("OpenXrSwapchain"),
                    format,
                    size,
                    mip_level_count: 1,
                    sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    usage: wgpu::TextureUses::COLOR_TARGET | wgpu::TextureUses::COPY_DST,
                    memory_flags: MemoryFlags::empty(),
                    view_formats: vec![],
                },
                None,
            );
            self.device
                .create_texture_from_hal::<wgpu_hal::api::Vulkan>(
                    hal_texture,
                    &wgpu::TextureDescriptor {
                        label: Some("OpenXrSwapchain"),
                        format,
                        size,
                        dimension: wgpu::TextureDimension::D2,
                        mip_level_count: 1,
                        sample_count,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    },
                )
        };

        Ok(texture)
    }

    fn format_from_raw(
        &self,
        format: &<openxr::Vulkan as openxr::Graphics>::Format,
    ) -> Option<wgpu::TextureFormat> {
        let vk_format = ash::vk::Format::from_raw(*format as i32);
        format_vk_to_wgpu(vk_format)
    }

    fn calculate_projection_matrix(&self, near: f32, fov: openxr::Fovf) -> bevy::math::Mat4 {
        let far = -1.0; //   use infinite projection

        let tan_angle_left = fov.angle_left.tan();
        let tan_angle_right = fov.angle_right.tan();

        let tan_angle_down = fov.angle_down.tan();
        let tan_angle_up = fov.angle_up.tan();

        let tan_angle_width: f32 = tan_angle_right - tan_angle_left;
        let tan_angle_height = tan_angle_up - tan_angle_down;

        let offset_z = 0.0;

        let mut cols: [f32; 16] = [0.0; 16];

        if far <= near {
            // place the far plane at infinity
            cols[0] = 2.0 / tan_angle_width;
            cols[4] = 0.0;
            cols[8] = (tan_angle_right + tan_angle_left) / tan_angle_width;
            cols[12] = 0.0;

            cols[1] = 0.0;
            cols[5] = 2.0 / tan_angle_height;
            cols[9] = (tan_angle_up + tan_angle_down) / tan_angle_height;
            cols[13] = 0.0;

            cols[2] = 0.0;
            cols[6] = 0.0;
            cols[10] = -1.0;
            cols[14] = -(near + offset_z);

            cols[3] = 0.0;
            cols[7] = 0.0;
            cols[11] = -1.0;
            cols[15] = 0.0;

            //  bevy uses the _reverse_ infinite projection
            //  https://dev.theomader.com/depth-precision/
            let z_reversal = Mat4::from_cols_array_2d(&[
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0, 1.0],
            ]);

            z_reversal * Mat4::from_cols_array(&cols)
        } else {
            // normal projection
            cols[0] = 2.0 / tan_angle_width;
            cols[4] = 0.0;
            cols[8] = (tan_angle_right + tan_angle_left) / tan_angle_width;
            cols[12] = 0.0;

            cols[1] = 0.0;
            cols[5] = 2.0 / tan_angle_height;
            cols[9] = (tan_angle_up + tan_angle_down) / tan_angle_height;
            cols[13] = 0.0;

            cols[2] = 0.0;
            cols[6] = 0.0;
            cols[10] = -(far + offset_z) / (far - near);
            cols[14] = -(far * (near + offset_z)) / (far - near);

            cols[3] = 0.0;
            cols[7] = 0.0;
            cols[11] = -1.0;
            cols[15] = 0.0;

            Mat4::from_cols_array(&cols)
        }
    }
}

fn get_api_version(openxr_instance: &Instance, system_id: SystemId) -> anyhow::Result<u32> {
    let graphics_requirements =
        openxr_instance.graphics_requirements::<openxr::Vulkan>(system_id)?;
    #[cfg(target_os = "android")]
    let target_version = openxr::Version::new(1, 1, 0);
    #[cfg(not(target_os = "android"))]
    let target_version = openxr::Version::new(1, 2, 0);

    if target_version < graphics_requirements.min_api_version_supported {
        error!(
            "OpenXR runtime requires Vulkan API version >= {}.{}.{}, but target version is {}.{}.{}",
            graphics_requirements.min_api_version_supported.major(),
            graphics_requirements.min_api_version_supported.minor(),
            graphics_requirements.min_api_version_supported.patch(),
            target_version.major(),
            target_version.minor(),
            target_version.patch()
        );
        return Err(anyhow!("Vulkan API version not supported"));
    } else if target_version.major() > graphics_requirements.max_api_version_supported.major() {
        error!(
            "OpenXR runtime requires Vulkan API version < {}, but target version is {}",
            graphics_requirements.max_api_version_supported.major(),
            target_version.major(),
        );
        return Err(anyhow!("Vulkan API version not supported"));
    }
    Ok(ash::vk::make_api_version(
        0,
        target_version.major() as u32,
        target_version.minor() as u32,
        target_version.patch(),
    ))
}

fn get_android_sdk_version() -> u32 {
    // the android_sdk_version stuff is copied from wgpu
    #[cfg(target_os = "android")]
    {
        let properties = android_system_properties::AndroidSystemProperties::new();
        // See: https://developer.android.com/reference/android/os/Build.VERSION_CODES
        if let Some(val) = properties.get("ro.build.version.sdk") {
            match val.parse::<u32>() {
                Ok(sdk_ver) => sdk_ver,
                Err(err) => {
                    error!(
                        concat!(
                            "Couldn't parse Android's ",
                            "ro.build.version.sdk system property ({}): {}",
                        ),
                        val, err,
                    );
                    0
                }
            }
        } else {
            error!("Couldn't read Android's ro.build.version.sdk system property");
            0
        }
    }
    #[cfg(not(target_os = "android"))]
    0
}

fn get_has_nv_optimus(vk_entry: &ash::Entry) -> anyhow::Result<bool> {
    let has_nv_optimus = unsafe {
        vk_entry
            .enumerate_instance_layer_properties()?
            .iter()
            .any(|prop| {
                prop.layer_name_as_c_str()
                    .is_ok_and(|cstr| cstr == c"VK_LAYER_NV_optimus")
            })
    };
    Ok(has_nv_optimus)
}

fn get_limits_and_features_from_adapter(
    adapter: &wgpu_hal::ExposedAdapter<wgpu_hal::api::Vulkan>,
    wgpu_settings: &WgpuSettings,
) -> anyhow::Result<(Limits, Features)> {
    // Modify limits and features.
    // Code snippets from bevy renderer initialization
    let mut limits = adapter.capabilities.limits.clone();
    let mut features = adapter.features;
    if adapter.info.device_type == DeviceType::DiscreteGpu {
        features.remove(Features::MAPPABLE_PRIMARY_BUFFERS);
    }

    if let Some(disabled_features) = wgpu_settings.disabled_features {
        features.remove(disabled_features);
    }
    features |= wgpu_settings.features;
    features |= Features::MULTIVIEW;

    if let Some(constrained_limits) = wgpu_settings.constrained_limits.as_ref() {
        // NOTE: Respect the configured limits as an 'upper bound'. This means for 'max' limits, we
        // take the minimum of the calculated limits according to the adapter/backend and the
        // specified max_limits. For 'min' limits, take the maximum instead. This is intended to
        // err on the side of being conservative. We can't claim 'higher' limits that are supported
        // but we can constrain to 'lower' limits.
        limits = wgpu::Limits {
            max_texture_dimension_1d: limits
                .max_texture_dimension_1d
                .min(constrained_limits.max_texture_dimension_1d),
            max_texture_dimension_2d: limits
                .max_texture_dimension_2d
                .min(constrained_limits.max_texture_dimension_2d),
            max_texture_dimension_3d: limits
                .max_texture_dimension_3d
                .min(constrained_limits.max_texture_dimension_3d),
            max_texture_array_layers: limits
                .max_texture_array_layers
                .min(constrained_limits.max_texture_array_layers),
            max_bind_groups: limits
                .max_bind_groups
                .min(constrained_limits.max_bind_groups),
            max_dynamic_uniform_buffers_per_pipeline_layout: limits
                .max_dynamic_uniform_buffers_per_pipeline_layout
                .min(constrained_limits.max_dynamic_uniform_buffers_per_pipeline_layout),
            max_dynamic_storage_buffers_per_pipeline_layout: limits
                .max_dynamic_storage_buffers_per_pipeline_layout
                .min(constrained_limits.max_dynamic_storage_buffers_per_pipeline_layout),
            max_sampled_textures_per_shader_stage: limits
                .max_sampled_textures_per_shader_stage
                .min(constrained_limits.max_sampled_textures_per_shader_stage),
            max_samplers_per_shader_stage: limits
                .max_samplers_per_shader_stage
                .min(constrained_limits.max_samplers_per_shader_stage),
            max_storage_buffers_per_shader_stage: limits
                .max_storage_buffers_per_shader_stage
                .min(constrained_limits.max_storage_buffers_per_shader_stage),
            max_storage_textures_per_shader_stage: limits
                .max_storage_textures_per_shader_stage
                .min(constrained_limits.max_storage_textures_per_shader_stage),
            max_uniform_buffers_per_shader_stage: limits
                .max_uniform_buffers_per_shader_stage
                .min(constrained_limits.max_uniform_buffers_per_shader_stage),
            max_binding_array_elements_per_shader_stage: limits
                .max_binding_array_elements_per_shader_stage
                .min(constrained_limits.max_binding_array_elements_per_shader_stage),
            max_binding_array_sampler_elements_per_shader_stage: limits
                .max_binding_array_sampler_elements_per_shader_stage
                .min(constrained_limits.max_binding_array_sampler_elements_per_shader_stage),
            max_uniform_buffer_binding_size: limits
                .max_uniform_buffer_binding_size
                .min(constrained_limits.max_uniform_buffer_binding_size),
            max_storage_buffer_binding_size: limits
                .max_storage_buffer_binding_size
                .min(constrained_limits.max_storage_buffer_binding_size),
            max_vertex_buffers: limits
                .max_vertex_buffers
                .min(constrained_limits.max_vertex_buffers),
            max_vertex_attributes: limits
                .max_vertex_attributes
                .min(constrained_limits.max_vertex_attributes),
            max_vertex_buffer_array_stride: limits
                .max_vertex_buffer_array_stride
                .min(constrained_limits.max_vertex_buffer_array_stride),
            max_push_constant_size: limits
                .max_push_constant_size
                .min(constrained_limits.max_push_constant_size),
            min_uniform_buffer_offset_alignment: limits
                .min_uniform_buffer_offset_alignment
                .max(constrained_limits.min_uniform_buffer_offset_alignment),
            min_storage_buffer_offset_alignment: limits
                .min_storage_buffer_offset_alignment
                .max(constrained_limits.min_storage_buffer_offset_alignment),
            max_inter_stage_shader_components: limits
                .max_inter_stage_shader_components
                .min(constrained_limits.max_inter_stage_shader_components),
            max_compute_workgroup_storage_size: limits
                .max_compute_workgroup_storage_size
                .min(constrained_limits.max_compute_workgroup_storage_size),
            max_compute_invocations_per_workgroup: limits
                .max_compute_invocations_per_workgroup
                .min(constrained_limits.max_compute_invocations_per_workgroup),
            max_compute_workgroup_size_x: limits
                .max_compute_workgroup_size_x
                .min(constrained_limits.max_compute_workgroup_size_x),
            max_compute_workgroup_size_y: limits
                .max_compute_workgroup_size_y
                .min(constrained_limits.max_compute_workgroup_size_y),
            max_compute_workgroup_size_z: limits
                .max_compute_workgroup_size_z
                .min(constrained_limits.max_compute_workgroup_size_z),
            max_compute_workgroups_per_dimension: limits
                .max_compute_workgroups_per_dimension
                .min(constrained_limits.max_compute_workgroups_per_dimension),
            max_buffer_size: limits
                .max_buffer_size
                .min(constrained_limits.max_buffer_size),
            max_bindings_per_bind_group: limits
                .max_bindings_per_bind_group
                .min(constrained_limits.max_bindings_per_bind_group),
            max_non_sampler_bindings: limits
                .max_non_sampler_bindings
                .min(constrained_limits.max_non_sampler_bindings),
            max_blas_primitive_count: limits
                .max_blas_primitive_count
                .min(constrained_limits.max_blas_primitive_count),
            max_blas_geometry_count: limits
                .max_blas_geometry_count
                .min(constrained_limits.max_blas_geometry_count),
            max_tlas_instance_count: limits
                .max_tlas_instance_count
                .min(constrained_limits.max_tlas_instance_count),
            max_color_attachments: limits
                .max_color_attachments
                .min(constrained_limits.max_color_attachments),
            max_color_attachment_bytes_per_sample: limits
                .max_color_attachment_bytes_per_sample
                .min(constrained_limits.max_color_attachment_bytes_per_sample),
            min_subgroup_size: limits
                .min_subgroup_size
                .max(constrained_limits.min_subgroup_size),
            max_subgroup_size: limits
                .max_subgroup_size
                .min(constrained_limits.max_subgroup_size),
            max_acceleration_structures_per_shader_stage: 0,
        };
    }

    Ok((limits, features))
}

fn format_vk_to_wgpu(format: ash::vk::Format) -> Option<wgpu::TextureFormat> {
    use ash::vk;

    let conv = match format {
        vk::Format::R8_UNORM => wgpu::TextureFormat::R8Unorm,
        vk::Format::R8_SNORM => wgpu::TextureFormat::R8Snorm,
        vk::Format::R8_UINT => wgpu::TextureFormat::R8Uint,
        vk::Format::R8_SINT => wgpu::TextureFormat::R8Sint,
        vk::Format::R16_UINT => wgpu::TextureFormat::R16Uint,
        vk::Format::R16_SINT => wgpu::TextureFormat::R16Sint,
        vk::Format::R16_UNORM => wgpu::TextureFormat::R16Unorm,
        vk::Format::R16_SNORM => wgpu::TextureFormat::R16Snorm,
        vk::Format::R16_SFLOAT => wgpu::TextureFormat::R16Float,
        vk::Format::R8G8_UNORM => wgpu::TextureFormat::Rg8Unorm,
        vk::Format::R8G8_SNORM => wgpu::TextureFormat::Rg8Snorm,
        vk::Format::R8G8_UINT => wgpu::TextureFormat::Rg8Uint,
        vk::Format::R8G8_SINT => wgpu::TextureFormat::Rg8Sint,
        vk::Format::R16G16_UNORM => wgpu::TextureFormat::Rg16Unorm,
        vk::Format::R16G16_SNORM => wgpu::TextureFormat::Rg16Snorm,
        vk::Format::R32_UINT => wgpu::TextureFormat::R32Uint,
        vk::Format::R32_SINT => wgpu::TextureFormat::R32Sint,
        vk::Format::R32_SFLOAT => wgpu::TextureFormat::R32Float,
        vk::Format::R16G16_UINT => wgpu::TextureFormat::Rg16Uint,
        vk::Format::R16G16_SINT => wgpu::TextureFormat::Rg16Sint,
        vk::Format::R16G16_SFLOAT => wgpu::TextureFormat::Rg16Float,
        vk::Format::R8G8B8A8_UNORM => wgpu::TextureFormat::Rgba8Unorm,
        vk::Format::R8G8B8A8_SRGB => wgpu::TextureFormat::Rgba8UnormSrgb,
        vk::Format::B8G8R8A8_SRGB => wgpu::TextureFormat::Bgra8UnormSrgb,
        vk::Format::R8G8B8A8_SNORM => wgpu::TextureFormat::Rgba8Snorm,
        vk::Format::B8G8R8A8_UNORM => wgpu::TextureFormat::Bgra8Unorm,
        vk::Format::R8G8B8A8_UINT => wgpu::TextureFormat::Rgba8Uint,
        vk::Format::R8G8B8A8_SINT => wgpu::TextureFormat::Rgba8Sint,
        vk::Format::A2B10G10R10_UINT_PACK32 => wgpu::TextureFormat::Rgb10a2Uint,
        vk::Format::A2B10G10R10_UNORM_PACK32 => wgpu::TextureFormat::Rgb10a2Unorm,
        vk::Format::B10G11R11_UFLOAT_PACK32 => wgpu::TextureFormat::Rg11b10Ufloat,
        vk::Format::R32G32_UINT => wgpu::TextureFormat::Rg32Uint,
        vk::Format::R32G32_SINT => wgpu::TextureFormat::Rg32Sint,
        vk::Format::R32G32_SFLOAT => wgpu::TextureFormat::Rg32Float,
        vk::Format::R16G16B16A16_UINT => wgpu::TextureFormat::Rgba16Uint,
        vk::Format::R16G16B16A16_SINT => wgpu::TextureFormat::Rgba16Sint,
        vk::Format::R16G16B16A16_UNORM => wgpu::TextureFormat::Rgba16Unorm,
        vk::Format::R16G16B16A16_SNORM => wgpu::TextureFormat::Rgba16Snorm,
        vk::Format::R16G16B16A16_SFLOAT => wgpu::TextureFormat::Rgba16Float,
        vk::Format::R32G32B32A32_UINT => wgpu::TextureFormat::Rgba32Uint,
        vk::Format::R32G32B32A32_SINT => wgpu::TextureFormat::Rgba32Sint,
        vk::Format::R32G32B32A32_SFLOAT => wgpu::TextureFormat::Rgba32Float,
        vk::Format::D32_SFLOAT => wgpu::TextureFormat::Depth32Float,
        vk::Format::D32_SFLOAT_S8_UINT => wgpu::TextureFormat::Depth32FloatStencil8,
        vk::Format::D16_UNORM => wgpu::TextureFormat::Depth16Unorm,
        vk::Format::G8_B8R8_2PLANE_420_UNORM => wgpu::TextureFormat::NV12,
        vk::Format::E5B9G9R9_UFLOAT_PACK32 => wgpu::TextureFormat::Rgb9e5Ufloat,
        vk::Format::BC1_RGBA_UNORM_BLOCK => wgpu::TextureFormat::Bc1RgbaUnorm,
        vk::Format::BC1_RGBA_SRGB_BLOCK => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        vk::Format::BC2_UNORM_BLOCK => wgpu::TextureFormat::Bc2RgbaUnorm,
        vk::Format::BC2_SRGB_BLOCK => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
        vk::Format::BC3_UNORM_BLOCK => wgpu::TextureFormat::Bc3RgbaUnorm,
        vk::Format::BC3_SRGB_BLOCK => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        vk::Format::BC4_UNORM_BLOCK => wgpu::TextureFormat::Bc4RUnorm,
        vk::Format::BC4_SNORM_BLOCK => wgpu::TextureFormat::Bc4RSnorm,
        vk::Format::BC5_UNORM_BLOCK => wgpu::TextureFormat::Bc5RgUnorm,
        vk::Format::BC5_SNORM_BLOCK => wgpu::TextureFormat::Bc5RgSnorm,
        vk::Format::BC6H_UFLOAT_BLOCK => wgpu::TextureFormat::Bc6hRgbUfloat,
        vk::Format::BC6H_SFLOAT_BLOCK => wgpu::TextureFormat::Bc6hRgbFloat,
        vk::Format::BC7_UNORM_BLOCK => wgpu::TextureFormat::Bc7RgbaUnorm,
        vk::Format::BC7_SRGB_BLOCK => wgpu::TextureFormat::Bc7RgbaUnormSrgb,
        vk::Format::ETC2_R8G8B8_UNORM_BLOCK => wgpu::TextureFormat::Etc2Rgb8Unorm,
        vk::Format::ETC2_R8G8B8_SRGB_BLOCK => wgpu::TextureFormat::Etc2Rgb8UnormSrgb,
        vk::Format::ETC2_R8G8B8A1_UNORM_BLOCK => wgpu::TextureFormat::Etc2Rgb8A1Unorm,
        vk::Format::ETC2_R8G8B8A1_SRGB_BLOCK => wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb,
        vk::Format::ETC2_R8G8B8A8_UNORM_BLOCK => wgpu::TextureFormat::Etc2Rgba8Unorm,
        vk::Format::ETC2_R8G8B8A8_SRGB_BLOCK => wgpu::TextureFormat::Etc2Rgba8UnormSrgb,
        vk::Format::EAC_R11_UNORM_BLOCK => wgpu::TextureFormat::EacR11Unorm,
        vk::Format::EAC_R11_SNORM_BLOCK => wgpu::TextureFormat::EacR11Snorm,
        vk::Format::EAC_R11G11_UNORM_BLOCK => wgpu::TextureFormat::EacRg11Unorm,
        vk::Format::EAC_R11G11_SNORM_BLOCK => wgpu::TextureFormat::EacRg11Snorm,
        vk::Format::ASTC_4X4_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_5X4_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_5X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_6X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_6X6_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_8X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_8X6_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_8X8_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_10X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_10X6_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_10X8_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_10X10_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_12X10_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_12X12_UNORM_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Unorm,
        },
        vk::Format::ASTC_4X4_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_5X4_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_5X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_6X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_6X6_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_8X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_8X6_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_8X8_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_10X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_10X6_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_10X8_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_10X10_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_12X10_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_12X12_SRGB_BLOCK => wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::UnormSrgb,
        },
        vk::Format::ASTC_4X4_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_5X4_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_5X5_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_6X5_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_6X6_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_8X5_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_8X6_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_8X8_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_10X5_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_10X6_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_10X8_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_10X10_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_12X10_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Hdr,
        },
        vk::Format::ASTC_12X12_SFLOAT_BLOCK_EXT => wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Hdr,
        },
        _ => return None,
    };

    Some(conv)
}

fn format_wgpu_to_vk(format: wgpu::TextureFormat) -> Option<ash::vk::Format> {
    use ash::vk;

    let conv = match format {
        wgpu::TextureFormat::R8Unorm => vk::Format::R8_UNORM,
        wgpu::TextureFormat::R8Snorm => vk::Format::R8_SNORM,
        wgpu::TextureFormat::R8Uint => vk::Format::R8_UINT,
        wgpu::TextureFormat::R8Sint => vk::Format::R8_SINT,
        wgpu::TextureFormat::R16Uint => vk::Format::R16_UINT,
        wgpu::TextureFormat::R16Sint => vk::Format::R16_SINT,
        wgpu::TextureFormat::R16Unorm => vk::Format::R16_UNORM,
        wgpu::TextureFormat::R16Snorm => vk::Format::R16_SNORM,
        wgpu::TextureFormat::R16Float => vk::Format::R16_SFLOAT,
        wgpu::TextureFormat::Rg8Unorm => vk::Format::R8G8_UNORM,
        wgpu::TextureFormat::Rg8Snorm => vk::Format::R8G8_SNORM,
        wgpu::TextureFormat::Rg8Uint => vk::Format::R8G8_UINT,
        wgpu::TextureFormat::Rg8Sint => vk::Format::R8G8_SINT,
        wgpu::TextureFormat::Rg16Unorm => vk::Format::R16G16_UNORM,
        wgpu::TextureFormat::Rg16Snorm => vk::Format::R16G16_SNORM,
        wgpu::TextureFormat::R32Uint => vk::Format::R32_UINT,
        wgpu::TextureFormat::R32Sint => vk::Format::R32_SINT,
        wgpu::TextureFormat::R32Float => vk::Format::R32_SFLOAT,
        wgpu::TextureFormat::Rg16Uint => vk::Format::R16G16_UINT,
        wgpu::TextureFormat::Rg16Sint => vk::Format::R16G16_SINT,
        wgpu::TextureFormat::Rg16Float => vk::Format::R16G16_SFLOAT,
        wgpu::TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
        wgpu::TextureFormat::Rgba8UnormSrgb => vk::Format::R8G8B8A8_SRGB,
        wgpu::TextureFormat::Bgra8UnormSrgb => vk::Format::B8G8R8A8_SRGB,
        wgpu::TextureFormat::Rgba8Snorm => vk::Format::R8G8B8A8_SNORM,
        wgpu::TextureFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
        wgpu::TextureFormat::Rgba8Uint => vk::Format::R8G8B8A8_UINT,
        wgpu::TextureFormat::Rgba8Sint => vk::Format::R8G8B8A8_SINT,
        wgpu::TextureFormat::Rgb10a2Uint => vk::Format::A2B10G10R10_UINT_PACK32,
        wgpu::TextureFormat::Rgb10a2Unorm => vk::Format::A2B10G10R10_UNORM_PACK32,
        wgpu::TextureFormat::Rg11b10Ufloat => vk::Format::B10G11R11_UFLOAT_PACK32,
        wgpu::TextureFormat::Rg32Uint => vk::Format::R32G32_UINT,
        wgpu::TextureFormat::Rg32Sint => vk::Format::R32G32_SINT,
        wgpu::TextureFormat::Rg32Float => vk::Format::R32G32_SFLOAT,
        wgpu::TextureFormat::Rgba16Uint => vk::Format::R16G16B16A16_UINT,
        wgpu::TextureFormat::Rgba16Sint => vk::Format::R16G16B16A16_SINT,
        wgpu::TextureFormat::Rgba16Unorm => vk::Format::R16G16B16A16_UNORM,
        wgpu::TextureFormat::Rgba16Snorm => vk::Format::R16G16B16A16_SNORM,
        wgpu::TextureFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
        wgpu::TextureFormat::Rgba32Uint => vk::Format::R32G32B32A32_UINT,
        wgpu::TextureFormat::Rgba32Sint => vk::Format::R32G32B32A32_SINT,
        wgpu::TextureFormat::Rgba32Float => vk::Format::R32G32B32A32_SFLOAT,
        wgpu::TextureFormat::Depth32Float => vk::Format::D32_SFLOAT,
        wgpu::TextureFormat::Depth32FloatStencil8 => vk::Format::D32_SFLOAT_S8_UINT,
        wgpu::TextureFormat::Depth16Unorm => vk::Format::D16_UNORM,
        wgpu::TextureFormat::NV12 => vk::Format::G8_B8R8_2PLANE_420_UNORM,
        wgpu::TextureFormat::Rgb9e5Ufloat => vk::Format::E5B9G9R9_UFLOAT_PACK32,
        wgpu::TextureFormat::Bc1RgbaUnorm => vk::Format::BC1_RGBA_UNORM_BLOCK,
        wgpu::TextureFormat::Bc1RgbaUnormSrgb => vk::Format::BC1_RGBA_SRGB_BLOCK,
        wgpu::TextureFormat::Bc2RgbaUnorm => vk::Format::BC2_UNORM_BLOCK,
        wgpu::TextureFormat::Bc2RgbaUnormSrgb => vk::Format::BC2_SRGB_BLOCK,
        wgpu::TextureFormat::Bc3RgbaUnorm => vk::Format::BC3_UNORM_BLOCK,
        wgpu::TextureFormat::Bc3RgbaUnormSrgb => vk::Format::BC3_SRGB_BLOCK,
        wgpu::TextureFormat::Bc4RUnorm => vk::Format::BC4_UNORM_BLOCK,
        wgpu::TextureFormat::Bc4RSnorm => vk::Format::BC4_SNORM_BLOCK,
        wgpu::TextureFormat::Bc5RgUnorm => vk::Format::BC5_UNORM_BLOCK,
        wgpu::TextureFormat::Bc5RgSnorm => vk::Format::BC5_SNORM_BLOCK,
        wgpu::TextureFormat::Bc6hRgbUfloat => vk::Format::BC6H_UFLOAT_BLOCK,
        wgpu::TextureFormat::Bc6hRgbFloat => vk::Format::BC6H_SFLOAT_BLOCK,
        wgpu::TextureFormat::Bc7RgbaUnorm => vk::Format::BC7_UNORM_BLOCK,
        wgpu::TextureFormat::Bc7RgbaUnormSrgb => vk::Format::BC7_SRGB_BLOCK,
        wgpu::TextureFormat::Etc2Rgb8Unorm => vk::Format::ETC2_R8G8B8_UNORM_BLOCK,
        wgpu::TextureFormat::Etc2Rgb8UnormSrgb => vk::Format::ETC2_R8G8B8_SRGB_BLOCK,
        wgpu::TextureFormat::Etc2Rgb8A1Unorm => vk::Format::ETC2_R8G8B8A1_UNORM_BLOCK,
        wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb => vk::Format::ETC2_R8G8B8A1_SRGB_BLOCK,
        wgpu::TextureFormat::Etc2Rgba8Unorm => vk::Format::ETC2_R8G8B8A8_UNORM_BLOCK,
        wgpu::TextureFormat::Etc2Rgba8UnormSrgb => vk::Format::ETC2_R8G8B8A8_SRGB_BLOCK,
        wgpu::TextureFormat::EacR11Unorm => vk::Format::EAC_R11_UNORM_BLOCK,
        wgpu::TextureFormat::EacR11Snorm => vk::Format::EAC_R11_SNORM_BLOCK,
        wgpu::TextureFormat::EacRg11Unorm => vk::Format::EAC_R11G11_UNORM_BLOCK,
        wgpu::TextureFormat::EacRg11Snorm => vk::Format::EAC_R11G11_SNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_4X4_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_5X4_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_5X5_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_6X5_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_6X6_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_8X5_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_8X6_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_8X8_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_10X5_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_10X6_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_10X8_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_10X10_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_12X10_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Unorm,
        } => vk::Format::ASTC_12X12_UNORM_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_4X4_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_5X4_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_5X5_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_6X5_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_6X6_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_8X5_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_8X6_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_8X8_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_10X5_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_10X6_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_10X8_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_10X10_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_12X10_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::UnormSrgb,
        } => vk::Format::ASTC_12X12_SRGB_BLOCK,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_4X4_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_5X4_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_5X5_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_6X5_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_6X6_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_8X5_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_8X6_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_8X8_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_10X5_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_10X6_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_10X8_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_10X10_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_12X10_SFLOAT_BLOCK_EXT,
        wgpu::TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Hdr,
        } => vk::Format::ASTC_12X12_SFLOAT_BLOCK_EXT,
        _ => return None,
    };

    Some(conv)
}
