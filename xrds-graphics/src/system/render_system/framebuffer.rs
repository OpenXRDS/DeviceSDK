use wgpu::{
    BindGroupDescriptor, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Extent3d,
    TextureSampleType,
};

use crate::{
    Constant, GraphicsInstance, RenderTargetOps, RenderTargetTexture, TextureFormat, XrdsTexture,
};

use super::gbuffer::GBuffer;

#[derive(Debug, Clone)]
pub struct Framebuffer {
    final_color: RenderTargetTexture,
    gbuffer: GBuffer,
    gbuffer_sampler: wgpu::Sampler,
    gbuffer_bind_group_layout: wgpu::BindGroupLayout,
    gbuffer_bind_group: wgpu::BindGroup,
}

impl Framebuffer {
    pub fn new(
        graphics_instance: &GraphicsInstance,
        size: Extent3d,
        output_format: TextureFormat,
    ) -> Self {
        let gbuffer = GBuffer::new(graphics_instance, size, wgpu::TextureFormat::Rgba32Float);
        let device = graphics_instance.device();

        let final_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
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
        let final_color_view = final_color_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: if size.depth_or_array_layers > 1 {
                Some(wgpu::TextureViewDimension::D2Array)
            } else {
                Some(wgpu::TextureViewDimension::D2)
            },
            ..Default::default()
        });
        let final_color = RenderTargetTexture::new(
            XrdsTexture::new(final_color_texture, output_format, size, final_color_view),
            RenderTargetOps::ColorAttachment(wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            }),
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            label: None,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            anisotropy_clamp: 1,
            ..Default::default()
        });

        let gbuffer_bind_group_layout = Self::create_bind_group_layout(device);
        let gbuffer_bind_group =
            Self::create_bind_group(device, &gbuffer_bind_group_layout, &gbuffer, &sampler);

        Self {
            final_color,
            gbuffer,
            gbuffer_sampler: sampler,
            gbuffer_bind_group_layout,
            gbuffer_bind_group,
        }
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

    pub fn final_color(&self) -> &RenderTargetTexture {
        &self.final_color
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.gbuffer_sampler
    }

    pub fn final_color_attachments(
        &self,
    ) -> anyhow::Result<Vec<Option<wgpu::RenderPassColorAttachment>>> {
        let attachments = vec![Some(wgpu::RenderPassColorAttachment {
            view: self.final_color.texture().view(),
            ops: self.final_color.as_color_operation()?,
            resolve_target: None,
        })];

        Ok(attachments)
    }

    pub fn gbuffer(&self) -> &GBuffer {
        &self.gbuffer
    }

    pub fn gbuffer_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.gbuffer_bind_group_layout
    }

    pub fn gbuffer_bind_group(&self) -> &wgpu::BindGroup {
        &self.gbuffer_bind_group
    }

    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("GBufferBindings"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        gbuffer: &GBuffer,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Position-Metallic-BindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.position_metallic().texture().view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.normal_roughness().texture().view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.albedo_occlusion().texture().view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.emissive().texture().view(),
                    ),
                },
            ],
        })
    }

    pub fn encode(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_bind_group(
            Constant::BIND_GROUP_ID_TEXTURE_INPUT,
            &self.gbuffer_bind_group,
            &[],
        );
    }
}
