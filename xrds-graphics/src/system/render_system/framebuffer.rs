use wgpu::{BindGroupDescriptor, Extent3d};

use crate::{
    BindGroupLayoutHelper, Constant, GraphicsInstance, RenderTargetOps, RenderTargetTexture,
    TextureFormat, XrdsTexture,
};

use super::gbuffer::GBuffer;

#[derive(Debug, Clone)]
struct RenderBuffer {
    render_target: RenderTargetTexture,
    bind_group: wgpu::BindGroup,
}

#[derive(Debug, Clone)]
pub struct Framebuffer {
    prev_index: usize,
    curr_index: usize,
    gbuffer: GBuffer,
    sampler: wgpu::Sampler,
    gbuffer_bind_group: wgpu::BindGroup,
    extent: Extent3d,
    render_buffers: [RenderBuffer; 3],

    motion_vector_bind_group: wgpu::BindGroup, // Only for TAA
    // Maybe optional in future implementation: depends on global graphics option like enable bloom
    bloom_downsample_buffers: Vec<RenderBuffer>,
    bloom_blur_buffers: Vec<RenderBuffer>,
}

impl Framebuffer {
    const MIN_BLOOM_DIM: u32 = 128;
    const MAX_BLOOM_LEVEL: usize = 3;

    pub fn new(
        graphics_instance: &GraphicsInstance,
        size: Extent3d,
        output_format: TextureFormat,
    ) -> Self {
        let gbuffer = GBuffer::new(
            graphics_instance,
            size,
            Constant::INTERMEDIATE_RENDER_FORMAT,
        );
        let device = graphics_instance.device();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            label: None,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 1,
            ..Default::default()
        });
        let bind_group_layout = BindGroupLayoutHelper::create_intermediate(device);
        let render_buffers = [
            Self::create_render_buffer(
                device,
                size,
                output_format,
                &bind_group_layout,
                &sampler,
                Some("framebuffer0"),
            ),
            Self::create_render_buffer(
                device,
                size,
                output_format,
                &bind_group_layout,
                &sampler,
                Some("framebuffer1"),
            ),
            Self::create_render_buffer(
                device,
                size,
                output_format,
                &bind_group_layout,
                &sampler,
                Some("framebuffer2"),
            ),
        ];

        let gbuffer_bind_group_layout = BindGroupLayoutHelper::create_gbuffer_params(device);
        let gbuffer_bind_group =
            Self::create_gbuffer_bind_group(device, &gbuffer_bind_group_layout, &gbuffer, &sampler);

        let motion_vector_bind_group_layout = BindGroupLayoutHelper::create_intermediate(device);
        let motion_vector_bind_group = Self::create_motion_vector_bind_group(
            device,
            &motion_vector_bind_group_layout,
            &gbuffer,
            &sampler,
        );

        let bloom_levels = Self::calculate_bloom_levels(size.width, size.height);
        let mut curr_width = size.width;
        let mut curr_height = size.height;
        let mut downsample_buffers = Vec::new();
        let mut blur_buffers = Vec::new();
        for _i in 0..bloom_levels {
            curr_width = (curr_width / 2).max(1);
            curr_height = (curr_height / 2).max(1);
            let level_size = Extent3d {
                width: curr_width,
                height: curr_height,
                depth_or_array_layers: size.depth_or_array_layers,
            };
            let downsample = Self::create_render_buffer(
                device,
                level_size,
                output_format,
                &bind_group_layout,
                &sampler,
                Some(format!("bloom_downsample_{}", _i).as_str()),
            );
            let blur = Self::create_render_buffer(
                device,
                level_size,
                output_format,
                &bind_group_layout,
                &sampler,
                Some(format!("bloom_blur_{}", _i).as_str()),
            );
            downsample_buffers.push(downsample);
            blur_buffers.push(blur);
        }

        Self {
            prev_index: 0,
            curr_index: 1,
            gbuffer,
            sampler,
            extent: size,
            gbuffer_bind_group,
            motion_vector_bind_group,
            render_buffers,
            bloom_downsample_buffers: downsample_buffers,
            bloom_blur_buffers: blur_buffers,
        }
    }

    fn calculate_bloom_levels(width: u32, height: u32) -> usize {
        let min_dim = width.min(height);
        let mut levels = 0;
        let mut current_dim = min_dim;
        while current_dim / 2 >= Self::MIN_BLOOM_DIM && levels <= Self::MAX_BLOOM_LEVEL
        /* Make dynamic for device spec */
        {
            current_dim /= 2;
            levels += 1;
        }

        levels.max(1)
    }

    pub fn position_metallic(&self) -> &RenderTargetTexture {
        self.gbuffer.position_metallic()
    }

    pub fn normal_roughness(&self) -> &RenderTargetTexture {
        self.gbuffer.normal_roughness()
    }

    pub fn albedo_occlusion(&self) -> &RenderTargetTexture {
        self.gbuffer.albedo_occlusion()
    }

    pub fn emissive(&self) -> &RenderTargetTexture {
        self.gbuffer.emissive()
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn gbuffer(&self) -> &GBuffer {
        &self.gbuffer
    }

    pub fn gbuffer_bind_group(&self) -> &wgpu::BindGroup {
        &self.gbuffer_bind_group
    }

    pub fn extent(&self) -> &wgpu::Extent3d {
        &self.extent
    }

    pub fn bloom_downsample_target(&self, index: usize) -> &RenderTargetTexture {
        &self.bloom_downsample_buffers[index].render_target
    }

    pub fn bloom_blur_target(&self, index: usize) -> &RenderTargetTexture {
        &self.bloom_blur_buffers[index].render_target
    }

    pub fn downsample_level(&self) -> usize {
        self.bloom_downsample_buffers.len()
    }

    fn create_render_buffer(
        device: &wgpu::Device,
        size: Extent3d,
        output_format: TextureFormat,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        label: Option<&str>,
    ) -> RenderBuffer {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: output_format.as_wgpu(),
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: if size.depth_or_array_layers > 1 {
                Some(wgpu::TextureViewDimension::D2Array)
            } else {
                Some(wgpu::TextureViewDimension::D2)
            },
            ..Default::default()
        });
        let render_target = RenderTargetTexture::new(
            XrdsTexture::new(texture, output_format, size, view),
            RenderTargetOps::ColorAttachment(wgpu::Operations {
                // load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
        );

        let bind_group = Self::create_render_buffer_bind_group(
            device,
            bind_group_layout,
            render_target.texture(),
            sampler,
        );

        RenderBuffer {
            render_target,
            bind_group,
        }
    }

    fn create_gbuffer_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        gbuffer: &GBuffer,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Position-Metallic-BindGroup"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.position_metallic().texture().view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.normal_roughness().texture().view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.albedo_occlusion().texture().view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.emissive().texture().view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.motion_vector().texture().view(),
                    ),
                },
            ],
        })
    }

    fn create_motion_vector_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        gbuffer: &GBuffer,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("MotionVector-BindGroup"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.motion_vector().texture().view(),
                    ),
                },
            ],
        })
    }

    fn create_render_buffer_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        texture: &XrdsTexture,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("RenderBufferBindGroup"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture.view()),
                },
            ],
        })
    }

    /// Begin frame and initialize internal index
    pub fn begin_frame(&mut self) {
        // Self previous frame index to last output index
        self.prev_index = self.curr_index;
        // Set current frame index to next from previous frame
        self.curr_index = (self.curr_index + 1) % self.render_buffers.len();
    }

    /// Swap input output frame for post processing
    pub fn swap_frame(&mut self) {
        self.curr_index = self.get_next_index();
    }

    fn get_next_index(&self) -> usize {
        self.render_buffers.len() - self.prev_index - self.curr_index
    }

    /// pre-defined color attachment. use ```output_target()``` for customizing attachment ops
    pub fn output_attachments(
        &self,
    ) -> anyhow::Result<Vec<Option<wgpu::RenderPassColorAttachment>>> {
        let output_index = self.get_next_index();
        let render_target = &self.render_buffers[output_index].render_target;

        let attachments = vec![Some(wgpu::RenderPassColorAttachment {
            view: render_target.texture().view(),
            ops: render_target.as_color_operation()?,
            resolve_target: None,
        })];

        Ok(attachments)
    }

    pub fn output_target(&self) -> &RenderTargetTexture {
        let output_index = self.get_next_index();
        &self.render_buffers[output_index].render_target
    }

    pub fn encode_input(&self, render_pass: &mut wgpu::RenderPass<'_>, index: u32) {
        let bind_group = &self.render_buffers[self.curr_index].bind_group;
        render_pass.set_bind_group(index, bind_group, &[]);
    }

    pub fn encode_previous_final_color(&self, render_pass: &mut wgpu::RenderPass<'_>, index: u32) {
        let bind_group = &self.render_buffers[self.prev_index].bind_group;
        render_pass.set_bind_group(index, bind_group, &[]);
    }

    pub fn encode_motion_vector(&self, render_pass: &mut wgpu::RenderPass<'_>, index: u32) {
        render_pass.set_bind_group(index, &self.motion_vector_bind_group, &[]);
    }

    pub fn encode_gbuffer_params(&self, render_pass: &mut wgpu::RenderPass<'_>, index: u32) {
        render_pass.set_bind_group(index, &self.gbuffer_bind_group, &[]);
    }

    pub fn encode_bloom_downsample(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        buffer_index: usize,
        index: u32,
    ) {
        render_pass.set_bind_group(
            index,
            &self.bloom_downsample_buffers[buffer_index].bind_group,
            &[],
        );
    }

    pub fn encode_bloom_blur(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        buffer_index: usize,
        index: u32,
    ) {
        render_pass.set_bind_group(
            index,
            &self.bloom_blur_buffers[buffer_index].bind_group,
            &[],
        );
    }
}
