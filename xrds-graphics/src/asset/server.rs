use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use uuid::Uuid;
use wgpu::{util::DeviceExt, TextureDescriptor};

use crate::{GraphicsInstance, TextureFormat, XrdsBuffer, XrdsMaterial, XrdsTexture};

use super::types::{AssetHandle, AssetId, AssetStrongHandle};

pub struct AssetServer {
    graphics_instance: Arc<GraphicsInstance>,
    resource_buffer: Arc<RwLock<ResourceBuffer>>,
}

#[derive(Default)]
pub struct ResourceBuffer {
    textures: HashMap<AssetId, AssetStrongHandle<XrdsTexture>>,
    buffers: HashMap<AssetId, AssetStrongHandle<XrdsBuffer>>,
    materials: HashMap<AssetId, AssetStrongHandle<XrdsMaterial>>,
}

impl AssetServer {
    pub fn new(graphics_instance: Arc<GraphicsInstance>) -> Arc<Self> {
        let resource_buffer = Arc::new(RwLock::new(ResourceBuffer::default()));

        Arc::new(Self {
            graphics_instance,
            resource_buffer,
        })
    }

    /// Register new texture to asset server.
    /// Return existing or new weak handle
    /// ```
    /// let id: AssetId = asset_server.generate_id();
    /// let handle: AssetHandle<XrdsTexture> = asset_server.register_texture(&id, data, width, height, depth_or_array);
    ///
    /// let texture: Option<XrdsTexture> = asset_server.get_texture(&handle);
    /// ```
    pub fn register_texture(
        &self,
        id: &AssetId,
        data: &[u8],
        width: u32,
        height: u32,
        depth_or_array: u32,
    ) -> AssetHandle<XrdsTexture> {
        {
            let lock = self.resource_buffer.read().unwrap();
            if let Some(handle) = lock.textures.get(id) {
                return handle.as_weak_handle();
            }
        }

        let label = match id {
            AssetId::Key(s) => s.clone(),
            AssetId::Uuid(u) => u.to_string(),
        };
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: depth_or_array,
        };
        let texture = self.graphics_instance.device().create_texture_with_data(
            self.graphics_instance.queue(),
            &TextureDescriptor {
                label: Some(&label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: if depth_or_array == 1 {
                    wgpu::TextureDimension::D2
                } else {
                    wgpu::TextureDimension::D3
                },
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let xrds_texture = XrdsTexture::from_init(
            texture,
            TextureFormat::from(wgpu::TextureFormat::Rgba8Unorm),
            size,
            Some(view),
        );
        let handle = AssetStrongHandle::new(id.clone(), xrds_texture);
        let weak_handle = handle.as_weak_handle();
        let mut lock = self.resource_buffer.write().unwrap();
        lock.textures.insert(id.clone(), handle);

        weak_handle
    }

    pub fn get_texture(&self, handle: &AssetHandle<XrdsTexture>) -> Option<XrdsTexture> {
        let lock = self.resource_buffer.read().unwrap();
        lock.textures.get(handle.id()).map(|h| h.asset().clone())
    }

    fn generate_id(&self) -> AssetId {
        let uuid = Uuid::new_v4();
        AssetId::Uuid(uuid)
    }
}
