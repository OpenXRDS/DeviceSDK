//! `AHardwareBuffer` -> `VkImage`, sampled through a Ycbcr conversion.
//!
//! See the module docs in `mod.rs` for why this is necessary at all.

use anyhow::{anyhow, Context, Result};
use ash::vk;
use std::collections::HashMap;

/// An imported frame: a Vulkan image view over the decoder's buffer.
///
/// Sampling it is only legal through [`VideoImporter::sampler`], because the format
/// is external and the Ycbcr conversion is part of both the view and the sampler.
pub(super) struct ImportedFrame {
    pub image: vk::Image,
    pub view: vk::ImageView,
    memory: vk::DeviceMemory,
    /// The buffer this was imported from, with a refcount held.
    buffer: *mut vk::AHardwareBuffer,
}

/// Imports decoder buffers as Vulkan images, reusing the import for a buffer the
/// reader has recycled back around.
///
/// The conversion and sampler are created from the *first* buffer's properties and
/// then fixed: every frame from one decoder shares an external format, and a sampler
/// is only usable with images of the format it was built for.
pub(super) struct VideoImporter {
    device: ash::Device,
    ahb: ash::android::external_memory_android_hardware_buffer::Device,
    memory_properties: vk::PhysicalDeviceMemoryProperties,

    /// Created lazily: the external format is not known until a buffer arrives.
    conversion: Option<vk::SamplerYcbcrConversion>,
    sampler: Option<vk::Sampler>,
    external_format: u64,

    /// Keyed by buffer pointer. The reader cycles a pool of a handful of buffers, so
    /// this stays small and every import after the first few is a cache hit —
    /// importing per frame would allocate device memory per frame.
    frames: HashMap<usize, ImportedFrame>,
}

// The raw handles carry no `Send`, but nothing here is shared: every method takes
// `&mut self`, so concurrent use is not expressible. Same reasoning as
// `xrds-media`'s decoders, and for the same reason — this has to live in a system's
// state, which Bevy requires to be `Send`.
unsafe impl Send for VideoImporter {}

impl VideoImporter {
    /// Build an importer against the renderer's own Vulkan device.
    ///
    /// It must be the same device: the images produced here are sampled by the
    /// renderer, and a Vulkan image cannot cross devices.
    pub fn new(device: &wgpu::Device) -> Result<Self> {
        let hal = unsafe { device.as_hal::<wgpu_hal::api::Vulkan>() }
            .ok_or_else(|| anyhow!("video import needs a Vulkan wgpu backend"))?;

        let raw_device = hal.raw_device().clone();
        let raw_instance = hal.shared_instance().raw_instance();
        let physical_device = hal.raw_physical_device();

        let memory_properties =
            unsafe { raw_instance.get_physical_device_memory_properties(physical_device) };
        let ahb = ash::android::external_memory_android_hardware_buffer::Device::new(
            raw_instance,
            &raw_device,
        );

        Ok(Self {
            device: raw_device,
            ahb,
            memory_properties,
            conversion: None,
            sampler: None,
            external_format: 0,
            frames: HashMap::new(),
        })
    }

    /// The immutable sampler every imported frame must be sampled through.
    ///
    /// `None` until the first successful import, because it cannot be built before
    /// the external format is known.
    pub fn sampler(&self) -> Option<vk::Sampler> {
        self.sampler
    }

    /// Import `buffer`, or return the existing import if it has been seen before.
    pub fn import(
        &mut self,
        buffer: *mut vk::AHardwareBuffer,
        width: u32,
        height: u32,
    ) -> Result<&ImportedFrame> {
        let key = buffer as usize;

        let mut format_props = vk::AndroidHardwareBufferFormatPropertiesANDROID::default();
        let (allocation_size, memory_type_bits) = {
            let mut props =
                vk::AndroidHardwareBufferPropertiesANDROID::default().push_next(&mut format_props);
            unsafe { self.ahb.get_android_hardware_buffer_properties(buffer, &mut props) }
                .context("vkGetAndroidHardwareBufferPropertiesANDROID")?;
            (props.allocation_size, props.memory_type_bits)
        };

        if self.conversion.is_none() {
            self.create_conversion(&format_props)?;
        } else if self.external_format != format_props.external_format {
            // The sampler is built for one external format and cannot be reused for
            // another. This would mean the decoder changed layout mid-stream, which
            // should not happen — say so rather than sample through a wrong sampler.
            return Err(anyhow!(
                "external format changed mid-stream: {:#x} -> {:#x}",
                self.external_format,
                format_props.external_format
            ));
        }

        if !self.frames.contains_key(&key) {
            let frame = self.import_uncached(buffer, width, height, allocation_size, memory_type_bits)?;
            self.frames.insert(key, frame);
        }
        Ok(self.frames.get(&key).expect("just inserted"))
    }

    /// Build the Ycbcr conversion and the sampler that carries it.
    ///
    /// The `suggested_*` fields are the driver's own answer for this buffer — on a
    /// Quest 3 they come back as BT.709 / narrow range. Using them rather than
    /// hardcoding is what keeps this correct on a device that answers differently.
    fn create_conversion(
        &mut self,
        format_props: &vk::AndroidHardwareBufferFormatPropertiesANDROID,
    ) -> Result<()> {
        let mut external_format =
            vk::ExternalFormatANDROID::default().external_format(format_props.external_format);

        let conversion = unsafe {
            self.device.create_sampler_ycbcr_conversion(
                &vk::SamplerYcbcrConversionCreateInfo::default()
                    // UNDEFINED, with the real format supplied by the pNext chain:
                    // the layout is vendor-defined and has no VkFormat.
                    .format(vk::Format::UNDEFINED)
                    .ycbcr_model(format_props.suggested_ycbcr_model)
                    .ycbcr_range(format_props.suggested_ycbcr_range)
                    .components(format_props.sampler_ycbcr_conversion_components)
                    .x_chroma_offset(format_props.suggested_x_chroma_offset)
                    .y_chroma_offset(format_props.suggested_y_chroma_offset)
                    .chroma_filter(vk::Filter::NEAREST)
                    .force_explicit_reconstruction(false)
                    .push_next(&mut external_format),
                None,
            )
        }
        .context("vkCreateSamplerYcbcrConversion")?;

        let mut conversion_info = vk::SamplerYcbcrConversionInfo::default().conversion(conversion);
        let sampler = unsafe {
            self.device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    // Clamp: a video frame has edges, and wrapping them would smear
                    // the opposite side of the picture into view.
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .unnormalized_coordinates(false)
                    .push_next(&mut conversion_info),
                None,
            )
        }
        .context("vkCreateSampler")?;

        log::info!(
            "video import: external format {:#x}, ycbcr model {:?}, range {:?}",
            format_props.external_format,
            format_props.suggested_ycbcr_model,
            format_props.suggested_ycbcr_range,
        );

        self.conversion = Some(conversion);
        self.sampler = Some(sampler);
        self.external_format = format_props.external_format;
        Ok(())
    }

    fn import_uncached(
        &self,
        buffer: *mut vk::AHardwareBuffer,
        width: u32,
        height: u32,
        allocation_size: u64,
        memory_type_bits: u32,
    ) -> Result<ImportedFrame> {
        let conversion = self.conversion.expect("created before this is reached");

        // Hold a refcount for as long as the image exists. The decoder keeps recent
        // frames alive too, but that guarantee belongs to the decoder's pool policy
        // and this image outlives any single frame — relying on someone else's
        // keepalive would work until it silently did not.
        unsafe { ndk_sys::AHardwareBuffer_acquire(buffer.cast()) };

        let image = unsafe {
            let mut external_format =
                vk::ExternalFormatANDROID::default().external_format(self.external_format);
            let mut external_memory = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::ANDROID_HARDWARE_BUFFER_ANDROID);

            self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::UNDEFINED)
                    .extent(vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut external_memory)
                    .push_next(&mut external_format),
                None,
            )
        }
        .context("vkCreateImage for imported buffer")?;

        let memory_type_index = self
            .find_memory_type(memory_type_bits)
            .ok_or_else(|| anyhow!("no memory type for the imported AHardwareBuffer"))?;

        let memory = unsafe {
            // Dedicated allocation is required for an imported buffer, not merely an
            // optimisation: the memory *is* the buffer.
            let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
            let mut import =
                vk::ImportAndroidHardwareBufferInfoANDROID::default().buffer(buffer.cast());

            self.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(allocation_size)
                    .memory_type_index(memory_type_index)
                    .push_next(&mut dedicated)
                    .push_next(&mut import),
                None,
            )
        }
        .context("vkAllocateMemory importing AHardwareBuffer")?;

        unsafe { self.device.bind_image_memory(image, memory, 0) }
            .context("vkBindImageMemory")?;

        let view = unsafe {
            let mut conversion_info =
                vk::SamplerYcbcrConversionInfo::default().conversion(conversion);
            self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::UNDEFINED)
                    .components(vk::ComponentMapping::default())
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .push_next(&mut conversion_info),
                None,
            )
        }
        .context("vkCreateImageView for imported buffer")?;

        Ok(ImportedFrame {
            image,
            view,
            memory,
            buffer,
        })
    }

    fn find_memory_type(&self, type_bits: u32) -> Option<u32> {
        (0..self.memory_properties.memory_type_count)
            .find(|i| type_bits & (1 << i) != 0)
    }
}

impl Drop for VideoImporter {
    fn drop(&mut self) {
        unsafe {
            for (_, frame) in self.frames.drain() {
                self.device.destroy_image_view(frame.view, None);
                self.device.destroy_image(frame.image, None);
                self.device.free_memory(frame.memory, None);
                ndk_sys::AHardwareBuffer_release(frame.buffer.cast());
            }
            if let Some(sampler) = self.sampler.take() {
                self.device.destroy_sampler(sampler, None);
            }
            if let Some(conversion) = self.conversion.take() {
                // After the sampler, which holds a reference to it.
                self.device.destroy_sampler_ycbcr_conversion(conversion, None);
            }
        }
    }
}
