use std::sync::Arc;

use ash::vk;
use wgpu::{DeviceDescriptor, InstanceDescriptor, RequestAdapterOptions, TextureUsages};

use crate::required_wgpu_features;

#[derive(Debug, Clone)]
pub struct GraphicsInstance {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline_cache: Option<wgpu::PipelineCache>,
}

#[derive(Debug, PartialEq)]
pub enum GraphicsApi {
    Vulkan,
    D3d12,
    OpenGles,
}

impl GraphicsInstance {
    pub async fn new() -> Arc<Self> {
        let instance = wgpu::Instance::new(&InstanceDescriptor::from_env_or_default());
        let adapter = instance
            .request_adapter(&RequestAdapterOptions::default())
            .await
            .unwrap();
        log::debug!("Adapter graphics limits: {:?}", adapter.limits());
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    required_features: required_wgpu_features(),
                    required_limits: adapter.limits(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        log::debug!("Device graphics limits: {:?}", device.limits());
        Arc::new(Self {
            instance,
            adapter,
            device,
            queue,
            pipeline_cache: None,
        })
    }

    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn pipeline_cache(&self) -> Option<&wgpu::PipelineCache> {
        self.pipeline_cache.as_ref()
    }

    pub fn from_init(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Arc<Self> {
        Arc::new(Self {
            instance,
            adapter,
            device,
            queue,
            pipeline_cache: None,
        })
    }
}

impl GraphicsInstance {
    pub fn create_texture_from_vk<'a>(
        &self,
        raw_image: vk::Image,
        texture_descriptor: &'a wgpu::hal::TextureDescriptor<'a>,
    ) -> anyhow::Result<wgpu::Texture> {
        let wgpu_texture = unsafe {
            let wgpu_hal_texture =
                wgpu_hal::vulkan::Device::texture_from_raw(raw_image, texture_descriptor, None);

            self.device
                .create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                    wgpu_hal_texture,
                    &Self::texture_descriptor_from_hal(texture_descriptor),
                )
        };

        Ok(wgpu_texture)
    }

    fn texture_descriptor_from_hal<'a>(
        desc: &'a wgpu::hal::TextureDescriptor<'a>,
    ) -> wgpu::TextureDescriptor<'a> {
        wgpu::TextureDescriptor {
            label: desc.label,
            size: desc.size,
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
            dimension: desc.dimension,
            format: desc.format,
            usage: Self::texture_usage_from_hal(desc.usage),
            view_formats: &desc.view_formats,
        }
    }

    fn texture_usage_from_hal(usages: wgpu::hal::TextureUses) -> wgpu::TextureUsages {
        let mut res = TextureUsages::empty();
        if usages.contains(wgpu::hal::TextureUses::COPY_SRC) {
            res |= wgpu::TextureUsages::COPY_SRC;
        }
        if usages.contains(wgpu::hal::TextureUses::COPY_DST) {
            res |= wgpu::TextureUsages::COPY_DST;
        }
        if usages.intersects(
            wgpu::hal::TextureUses::RESOURCE | wgpu::hal::TextureUses::DEPTH_STENCIL_READ,
        ) {
            res |= wgpu::TextureUsages::TEXTURE_BINDING;
        }
        if usages.intersects(
            wgpu::hal::TextureUses::STORAGE_READ_ONLY
                | wgpu::hal::TextureUses::STORAGE_WRITE_ONLY
                | wgpu::hal::TextureUses::STORAGE_READ_WRITE,
        ) {
            res |= wgpu::TextureUsages::STORAGE_BINDING;
        }
        if usages.intersects(
            wgpu::hal::TextureUses::COLOR_TARGET | wgpu::hal::TextureUses::DEPTH_STENCIL_WRITE,
        ) {
            res |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        }
        if usages.contains(wgpu::hal::TextureUses::STORAGE_ATOMIC) {
            res |= wgpu::TextureUsages::STORAGE_ATOMIC
        }

        res
    }
}
