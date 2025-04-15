use wgpu::{AddressMode, Extent3d, FilterMode};

use crate::{GraphicsInstance, XrdsTexture};

use super::LightInstance;

#[derive(Debug, Clone, Copy)]
pub enum ShadowQuality {
    Low,
    Medium,
    High,
}

#[derive(Debug)]
pub struct ShadowmapPool {
    graphics_instance: GraphicsInstance,
    shadowmaps: Vec<XrdsTexture>,
    dummy_shadowmap: XrdsTexture,
    sampler: wgpu::Sampler,
    assigned_count: usize,
    shadow_extent: wgpu::Extent3d,
}

impl ShadowmapPool {
    const SHADOWMAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;
    const MAX_SHADOWMAPS: usize = 64;

    pub fn new(graphics_instance: &GraphicsInstance, quality: ShadowQuality) -> Self {
        let extent = match quality {
            ShadowQuality::Low => wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
            ShadowQuality::Medium => wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            ShadowQuality::High => wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
        };

        // Create dummy shadowmap with size 1x1x1 for empty array
        let dummy_shadowmap = Self::create_shadow_texture(graphics_instance, Extent3d::default());

        let sampler = graphics_instance
            .device()
            .create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Shadowmap Sampler"),
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                mag_filter: FilterMode::Nearest,
                min_filter: FilterMode::Nearest,
                ..Default::default()
            });
        Self {
            graphics_instance: graphics_instance.clone(),
            shadowmaps: Vec::new(),
            sampler,
            dummy_shadowmap,
            assigned_count: 0,
            shadow_extent: extent,
        }
    }

    /// Reset shadowmap index assignment of lights
    ///
    /// This method allow to select shadow castable light dynamically
    /// Currently do not support remove single light assignment
    pub fn reset(&mut self) {
        self.assigned_count = 0;
    }

    pub fn assign_index(&mut self, light_instance: &mut LightInstance) -> anyhow::Result<()> {
        let required_count = light_instance.light_type().shadowmap_count();
        if (self.assigned_count + required_count) > Self::MAX_SHADOWMAPS {
            return Err(anyhow::anyhow!("Shadowmap pool is full"));
        }

        if self.assigned_count + required_count > self.shadowmaps.len() {
            self.increase_pool(self.assigned_count + required_count)?;
        }

        let index = self.assigned_count as u32;
        light_instance.state_mut().set_shadow_map_index(index);

        self.assigned_count += required_count;

        Ok(())
    }

    pub fn get(&self, index: usize) -> anyhow::Result<&XrdsTexture> {
        if let Some(shadowmap) = self.shadowmaps.get(index) {
            Ok(shadowmap)
        } else {
            Err(anyhow::anyhow!("Shadowmap not found"))
        }
    }

    /// Increase interanl pool size (not maximum size)
    ///
    /// Ignore if request size ls smaller than current size
    pub fn increase_pool(&mut self, request_size: usize) -> anyhow::Result<()> {
        let current_size = self.shadowmaps.len();
        if request_size == current_size {
            return Ok(());
        }

        if request_size < current_size {
            return Ok(());
        }

        // Create new shadowmaps
        for _ in current_size..request_size {
            let texture = Self::create_shadow_texture(&self.graphics_instance, self.shadow_extent);
            self.shadowmaps.push(texture);
        }

        Ok(())
    }

    pub fn shadowmap_views(&self) -> Vec<&wgpu::TextureView> {
        let shadowmap_views: Vec<_> = self.shadowmaps.iter().map(|s| s.view()).collect();
        let dummy_views =
            vec![self.dummy_shadowmap.view(); Self::MAX_SHADOWMAPS - self.shadowmaps.len()];
        let views = [shadowmap_views, dummy_views].concat();

        views
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
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
                format: Self::SHADOWMAP_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
        let view = inner.create_view(&wgpu::TextureViewDescriptor::default());

        XrdsTexture::new(inner, Self::SHADOWMAP_FORMAT.into(), extent, view)
    }
}
