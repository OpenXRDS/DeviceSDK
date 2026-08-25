//! The conversion pass: sample an imported frame, write ordinary RGBA.
//!
//! One fullscreen triangle. The interesting work happens in the sampler, not the
//! shader — see `video_convert.frag`.

use anyhow::{anyhow, Context, Result};
use ash::vk;

/// Renders imported video frames into a wgpu-ownable RGBA image.
///
/// The output is `R8G8B8A8_SRGB` so the render pass does the sRGB encode on write
/// and wgpu can sample it as `Rgba8UnormSrgb`. Doing the encode in the shader
/// instead would mean storing display-referred values in a linear texture, which
/// bands visibly in the darks — and video is mostly darks.
pub(super) struct VideoConverter {
    device: ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,

    render_pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    sampler: vk::Sampler,

    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,

    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,

    width: u32,
    height: u32,
}

// Raw handles carry no `Send`; nothing here is shared and every method takes
// `&mut self`. Same reasoning as the importer.
unsafe impl Send for VideoConverter {}

const OUTPUT_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

impl VideoConverter {
    /// Build the pass and the image it renders into.
    ///
    /// `sampler` must be the importer's immutable Ycbcr sampler: it is baked into
    /// the descriptor set layout, so a converter is only usable with frames from the
    /// importer that produced it.
    pub(super) fn new(
        wgpu_device: &wgpu::Device,
        sampler: vk::Sampler,
        width: u32,
        height: u32,
    ) -> Result<(Self, wgpu::Texture)> {
        let hal = unsafe { wgpu_device.as_hal::<wgpu_hal::api::Vulkan>() }
            .ok_or_else(|| anyhow!("video conversion needs a Vulkan wgpu backend"))?;
        let device = hal.raw_device().clone();
        let physical_device = hal.raw_physical_device();
        let instance = hal.shared_instance().raw_instance();
        let queue_family_index = hal.queue_family_index();
        let queue = unsafe { device.get_device_queue(queue_family_index, hal.queue_index()) };

        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        unsafe {
            let image = device
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(OUTPUT_FORMAT)
                        .extent(vk::Extent3D {
                            width,
                            height,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(
                            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                        )
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED),
                    None,
                )
                .context("vkCreateImage for the conversion target")?;

            let requirements = device.get_image_memory_requirements(image);
            let memory_type_index = (0..memory_properties.memory_type_count)
                .find(|i| {
                    requirements.memory_type_bits & (1 << i) != 0
                        && memory_properties.memory_types[*i as usize]
                            .property_flags
                            .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
                })
                .ok_or_else(|| anyhow!("no device-local memory for the conversion target"))?;

            let memory = device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(requirements.size)
                        .memory_type_index(memory_type_index),
                    None,
                )
                .context("vkAllocateMemory for the conversion target")?;
            device
                .bind_image_memory(image, memory, 0)
                .context("vkBindImageMemory for the conversion target")?;

            let view = device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(OUTPUT_FORMAT)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
                .context("vkCreateImageView for the conversion target")?;

            // finalLayout is SHADER_READ_ONLY_OPTIMAL because wgpu samples this
            // next, and wgpu will not know to transition it — it never sees the
            // pass that wrote it.
            let attachment = vk::AttachmentDescription::default()
                .format(OUTPUT_FORMAT)
                .samples(vk::SampleCountFlags::TYPE_1)
                // The pass writes every pixel, so there is nothing to preserve.
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let color_ref = vk::AttachmentReference::default()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
            let color_refs = [color_ref];
            let subpass = vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color_refs);
            let attachments = [attachment];
            let subpasses = [subpass];
            let render_pass = device
                .create_render_pass(
                    &vk::RenderPassCreateInfo::default()
                        .attachments(&attachments)
                        .subpasses(&subpasses),
                    None,
                )
                .context("vkCreateRenderPass")?;

            let views = [view];
            let framebuffer = device
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(render_pass)
                        .attachments(&views)
                        .width(width)
                        .height(height)
                        .layers(1),
                    None,
                )
                .context("vkCreateFramebuffer")?;

            // The immutable sampler is the whole point: a Ycbcr-converting sampler
            // cannot be bound dynamically, it must be part of the layout.
            let immutable = [sampler];
            let binding = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                .immutable_samplers(&immutable);
            let bindings = [binding];
            let set_layout = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .context("vkCreateDescriptorSetLayout")?;

            let set_layouts = [set_layout];
            let pipeline_layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts),
                    None,
                )
                .context("vkCreatePipelineLayout")?;

            let vert = create_shader(&device, include_bytes!("../shaders/video_convert.vert.spv"))?;
            let frag = create_shader(&device, include_bytes!("../shaders/video_convert.frag.spv"))?;

            let entry = c"main";
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(vert)
                    .name(entry),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(frag)
                    .name(entry),
            ];
            // No vertex input at all: the vertex shader generates its three
            // positions from gl_VertexIndex.
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let viewports = [vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }];
            let scissors = [vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D { width, height },
            }];
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewports(&viewports)
                .scissors(&scissors);
            let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::NONE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .line_width(1.0);
            let multisample = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .blend_enable(false)];
            let color_blend = vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(&blend_attachments);

            let pipeline = device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[vk::GraphicsPipelineCreateInfo::default()
                        .stages(&stages)
                        .vertex_input_state(&vertex_input)
                        .input_assembly_state(&input_assembly)
                        .viewport_state(&viewport_state)
                        .rasterization_state(&rasterization)
                        .multisample_state(&multisample)
                        .color_blend_state(&color_blend)
                        .layout(pipeline_layout)
                        .render_pass(render_pass)
                        .subpass(0)],
                    None,
                )
                .map_err(|(_, e)| anyhow!("vkCreateGraphicsPipelines: {e:?}"))?[0];

            device.destroy_shader_module(vert, None);
            device.destroy_shader_module(frag, None);

            let pool_sizes = [vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)];
            let descriptor_pool = device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(1)
                        .pool_sizes(&pool_sizes),
                    None,
                )
                .context("vkCreateDescriptorPool")?;
            let descriptor_set = device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(descriptor_pool)
                        .set_layouts(&set_layouts),
                )
                .context("vkAllocateDescriptorSets")?[0];

            let command_pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(queue_family_index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .context("vkCreateCommandPool")?;
            let command_buffer = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(command_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .context("vkAllocateCommandBuffers")?[0];
            // Signalled, so the first frame's wait returns immediately rather than
            // hanging on a submission that never happened.
            let fence = device
                .create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )
                .context("vkCreateFence")?;

            // Hand the image to wgpu without transferring ownership: `None` for the
            // drop callback means wgpu will not destroy it, exactly as the OpenXR
            // swapchain images are wrapped.
            let hal_texture = hal.texture_from_raw(
                image,
                &wgpu_hal::TextureDescriptor {
                    label: Some("XrdsVideoFrame"),
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    usage: wgpu::TextureUses::RESOURCE,
                    memory_flags: wgpu_hal::MemoryFlags::empty(),
                    view_formats: vec![],
                },
                None,
            );
            let texture = wgpu_device.create_texture_from_hal::<wgpu_hal::api::Vulkan>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("XrdsVideoFrame"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            );

            Ok((
                Self {
                    device,
                    queue,
                    queue_family_index,
                    render_pass,
                    set_layout,
                    pipeline_layout,
                    pipeline,
                    descriptor_pool,
                    descriptor_set,
                    sampler,
                    command_pool,
                    command_buffer,
                    fence,
                    image,
                    memory,
                    view,
                    framebuffer,
                    width,
                    height,
                },
                texture,
            ))
        }
    }

    /// Draw one imported frame into the output image.
    ///
    /// # Synchronisation
    ///
    /// The fence is waited on at the *top* of the next call, not after submitting.
    /// It exists to know when the command buffer is safe to re-record, and a whole
    /// frame has passed by then, so in practice the wait returns immediately.
    ///
    /// Measured on a Quest 3 at 1920x800, both ways through this same code path:
    /// **0.62 ms/frame waiting immediately after submit, 0.27 ms/frame waiting at
    /// the top of the next call.** (A 3.53 ms figure appears in earlier notes for
    /// this pass; that was measured from a main-world system, where blocking stalls
    /// the main thread against a busy render thread. Running here, in the render
    /// schedule, was worth more than this change was.)
    ///
    /// **What is still not guaranteed.** This submission is not ordered against
    /// wgpu's. wgpu deliberately does not rely on same-queue submission order —
    /// it chains its own submits with relay semaphores
    /// (`wgpu-hal/src/vulkan/mod.rs`: "In order for submissions to be strictly
    /// ordered, we encode a dependency between each submission using a pair of
    /// semaphores") — and our raw submit is not part of that chain. So two
    /// hazards remain, both of which produce a stale or torn frame rather than
    /// corruption or a crash:
    ///
    /// - our write for frame N may not be complete when wgpu samples it, and
    /// - we may overwrite the image while wgpu is still reading frame N-1.
    ///
    /// Both windows are a full frame wide, which is why this looks fine and why
    /// that is not an argument. A real fix needs semaphores in both directions;
    /// wgpu-hal currently exposes only `Queue::add_signal_semaphore`, so the wait
    /// direction has no hook and closing this properly needs something upstream.
    /// Tracked as S6 in `OVERALL_PROGRESS.md`.
    pub(super) fn convert(&mut self, frame_view: vk::ImageView, frame_image: vk::Image) -> Result<()> {
        unsafe {
            self.device
                .wait_for_fences(&[self.fence], true, u64::MAX)
                .context("vkWaitForFences")?;
            self.device.reset_fences(&[self.fence]).context("vkResetFences")?;

            // The sampler here is ignored — it is immutable in the layout — but the
            // view is not, and it changes as the reader cycles its buffer pool.
            let image_info = [vk::DescriptorImageInfo {
                sampler: self.sampler,
                image_view: frame_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            self.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(self.descriptor_set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&image_info)],
                &[],
            );

            self.device
                .begin_command_buffer(
                    self.command_buffer,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .context("vkBeginCommandBuffer")?;

            // Acquire the frame from the decoder. The producer is outside Vulkan
            // entirely, which is what QUEUE_FAMILY_FOREIGN_EXT expresses; without
            // this the contents are formally undefined.
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .dst_queue_family_index(self.queue_family_index)
                    .image(frame_image)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .level_count(1)
                            .layer_count(1),
                    )],
            );

            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.render_pass)
                    .framebuffer(self.framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: vk::Extent2D {
                            width: self.width,
                            height: self.height,
                        },
                    }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.descriptor_set],
                &[],
            );
            self.device.cmd_draw(self.command_buffer, 3, 1, 0, 0);
            self.device.cmd_end_render_pass(self.command_buffer);
            self.device
                .end_command_buffer(self.command_buffer)
                .context("vkEndCommandBuffer")?;

            let command_buffers = [self.command_buffer];
            // Submit and return. The fence is not waited on here — the next call
            // does that, by which time the work is a frame old and done.
            self.device
                .queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default().command_buffers(&command_buffers)],
                    self.fence,
                )
                .context("vkQueueSubmit")?;
        }
        Ok(())
    }
}

fn create_shader(device: &ash::Device, bytes: &[u8]) -> Result<vk::ShaderModule> {
    let code = ash::util::read_spv(&mut std::io::Cursor::new(bytes)).context("read_spv")?;
    unsafe {
        device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)
    }
    .context("vkCreateShaderModule")
}

impl Drop for VideoConverter {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_descriptor_set_layout(self.set_layout, None);
            self.device.destroy_framebuffer(self.framebuffer, None);
            self.device.destroy_render_pass(self.render_pass, None);
            self.device.destroy_image_view(self.view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
