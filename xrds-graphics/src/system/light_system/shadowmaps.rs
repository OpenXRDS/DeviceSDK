use std::num::NonZeroU32;

use wgpu::{BindGroupDescriptor, BindGroupEntry};

use crate::{GraphicsInstance, XrdsTexture};

#[derive(Debug, Clone, Copy)]
pub enum ShadowQuality {
    Low,
    Medium,
    High,
}

#[derive(Debug)]
pub struct ShadowmapPool {
    shadowmaps: Vec<XrdsTexture>,
    quality: ShadowQuality,
    write_bind_group_layout: wgpu::BindGroupLayout,
    write_bind_group: wgpu::BindGroup,
    read_bind_group_layout: wgpu::BindGroupLayout,
    read_bind_group: wgpu::BindGroup,
}

struct BindGroupCreationResult {
    write_bind_group_layout: wgpu::BindGroupLayout,
    write_bind_group: wgpu::BindGroup,
    read_bind_group_layout: wgpu::BindGroupLayout,
    read_bind_group: wgpu::BindGroup,
}

impl ShadowmapPool {
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;

    pub fn new(
        graphics_instance: &GraphicsInstance,
        quality: ShadowQuality,
        initial_size: usize,
    ) -> Self {
        let depth_or_array_layers = graphics_instance
            .multiview()
            .or(NonZeroU32::new(1))
            .unwrap()
            .get();
        let extent = match quality {
            ShadowQuality::Low => wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers,
            },
            ShadowQuality::Medium => wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers,
            },
            ShadowQuality::High => wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers,
            },
        };
        let shadowmaps: Vec<_> = (0..initial_size)
            .into_iter()
            .map(|_| Self::create_shadow_texture(graphics_instance, extent))
            .collect();

        let res = Self::create_bind_group_and_layout(graphics_instance, &shadowmaps);

        Self {
            shadowmaps,
            quality,
            write_bind_group_layout: res.write_bind_group_layout,
            write_bind_group: res.write_bind_group,
            read_bind_group_layout: res.read_bind_group_layout,
            read_bind_group: res.read_bind_group,
        }
    }

    pub fn get_shadowmap(&self, index: usize) -> anyhow::Result<XrdsTexture> {
        if let Some(shadowmap) = self.shadowmaps.get(index) {
            Ok(shadowmap.clone())
        } else {
            Err(anyhow::anyhow!("Shadowmap not found"))
        }
    }

    pub fn increase_pool(
        &mut self,
        graphics_instance: &GraphicsInstance,
        size: usize,
    ) -> anyhow::Result<()> {
        let depth_or_array_layers = graphics_instance
            .multiview()
            .or(NonZeroU32::new(1))
            .unwrap()
            .get();
        let extent = match self.quality {
            ShadowQuality::Low => wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers,
            },
            ShadowQuality::Medium => wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers,
            },
            ShadowQuality::High => wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers,
            },
        };
        for _ in 0..size {
            let texture = Self::create_shadow_texture(graphics_instance, extent);
            self.shadowmaps.push(texture);
        }

        // Recreate bind groups
        let res = Self::create_bind_group_and_layout(graphics_instance, &self.shadowmaps);
        self.write_bind_group_layout = res.write_bind_group_layout;
        self.write_bind_group = res.write_bind_group;
        self.read_bind_group_layout = res.read_bind_group_layout;
        self.read_bind_group = res.read_bind_group;

        Ok(())
    }

    fn create_shadow_texture(
        graphics_instance: &GraphicsInstance,
        extent: wgpu::Extent3d,
    ) -> XrdsTexture {
        let inner = graphics_instance
            .device()
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
        let view = inner.create_view(&wgpu::TextureViewDescriptor::default());

        XrdsTexture::new(inner, Self::FORMAT.into(), extent, view)
    }

    fn create_bind_group_and_layout(
        graphics_instance: &GraphicsInstance,
        shadowmaps: &[XrdsTexture],
    ) -> BindGroupCreationResult {
        let views: Vec<_> = shadowmaps.iter().map(|s| s.view()).collect();

        let write_bind_group_layout =
            graphics_instance
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: Self::FORMAT,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                        },
                        count: NonZeroU32::new(shadowmaps.len() as _),
                    }],
                });
        let write_bind_group = graphics_instance
            .device()
            .create_bind_group(&BindGroupDescriptor {
                label: Some("Shadowmap-write"),
                layout: &write_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(&views),
                }],
            });

        let read_bind_group_layout =
            graphics_instance
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: NonZeroU32::new(shadowmaps.len() as _),
                    }],
                });

        let read_bind_group = graphics_instance
            .device()
            .create_bind_group(&BindGroupDescriptor {
                label: Some("Shadowmap-read"),
                layout: &read_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(&views),
                }],
            });

        BindGroupCreationResult {
            write_bind_group_layout,
            write_bind_group,
            read_bind_group_layout,
            read_bind_group,
        }
    }
}
