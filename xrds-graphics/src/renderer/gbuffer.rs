use std::sync::Arc;

use log::debug;
use wgpu::{Extent3d, Operations};

use crate::{GraphicsInstance, RenderTargetOps, RenderTargetTexture, XrdsTexture};

#[derive(Debug, Clone)]
pub struct GBuffer {
    position_metallic: RenderTargetTexture,
    normal_roughness: RenderTargetTexture,
    albedo_occlusion: RenderTargetTexture,
    emissive: RenderTargetTexture,
    depth_stencil: RenderTargetTexture,
}

impl GBuffer {
    pub fn new(
        graphics_instance: Arc<GraphicsInstance>,
        size: Extent3d,
        format: wgpu::TextureFormat,
    ) -> Self {
        let device = graphics_instance.device();
        let position_metallic =
            Self::create_texture(device, size, format, "GBuffer-position-metallic");
        let normal_roughness =
            Self::create_texture(device, size, format, "GBuffer-normal-roughness");
        let albedo_occlusion =
            Self::create_texture(device, size, format, "GBuffer-albedo-occlusion");
        let emissive = Self::create_texture(device, size, format, "GBuffer-emissive");
        let depth_stencil = Self::create_texture(
            device,
            size,
            wgpu::TextureFormat::Depth24PlusStencil8,
            "GBuffer-depth-stencil",
        );

        Self {
            position_metallic,
            normal_roughness,
            albedo_occlusion,
            emissive,
            depth_stencil,
        }
    }

    fn create_texture<'a>(
        device: &wgpu::Device,
        size: Extent3d,
        format: wgpu::TextureFormat,
        label: &'a str,
    ) -> RenderTargetTexture {
        debug!(
            "create_texture() label={:?}, size={:?}, format={:?}",
            label, size, format
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: if size.depth_or_array_layers > 1 {
                Some(wgpu::TextureViewDimension::D2Array)
            } else {
                Some(wgpu::TextureViewDimension::D2)
            },
            array_layer_count: Some(size.depth_or_array_layers),
            ..Default::default()
        });

        let is_depth = (format == wgpu::TextureFormat::Depth24PlusStencil8)
            || (format == wgpu::TextureFormat::Depth32Float)
            || (format == wgpu::TextureFormat::Depth24Plus);

        let target = RenderTargetTexture::new(
            XrdsTexture::new(texture, format.into(), size, view),
            if is_depth {
                RenderTargetOps::DepthStencilAttachment {
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }
            } else {
                RenderTargetOps::ColorAttachment(Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                })
            },
        );
        debug!("Create texture={:?}", target);
        target
    }

    pub fn depth_stencil(&self) -> &RenderTargetTexture {
        &self.depth_stencil
    }

    pub fn position_metallic(&self) -> &RenderTargetTexture {
        &self.position_metallic
    }

    pub fn normal_roughness(&self) -> &RenderTargetTexture {
        &self.normal_roughness
    }

    pub fn albedo_occlusion(&self) -> &RenderTargetTexture {
        &self.albedo_occlusion
    }

    pub fn emissive(&self) -> &RenderTargetTexture {
        &self.emissive
    }

    pub fn as_color_attachments(
        &self,
    ) -> anyhow::Result<Vec<Option<wgpu::RenderPassColorAttachment>>> {
        let mut attachments = Vec::new();
        attachments.push(Some(wgpu::RenderPassColorAttachment {
            view: self.position_metallic.texture().view(),
            ops: self.position_metallic.as_color_operation()?,
            resolve_target: None,
        }));
        attachments.push(Some(wgpu::RenderPassColorAttachment {
            view: self.normal_roughness.texture().view(),
            ops: self.normal_roughness.as_color_operation()?,
            resolve_target: None,
        }));
        attachments.push(Some(wgpu::RenderPassColorAttachment {
            view: self.albedo_occlusion.texture().view(),
            ops: self.albedo_occlusion.as_color_operation()?,
            resolve_target: None,
        }));
        attachments.push(Some(wgpu::RenderPassColorAttachment {
            view: self.emissive.texture().view(),
            ops: self.emissive.as_color_operation()?,
            resolve_target: None,
        }));

        Ok(attachments)
    }

    pub fn as_depth_stencil_attachment(
        &self,
    ) -> anyhow::Result<Option<wgpu::RenderPassDepthStencilAttachment>> {
        Ok(Some(wgpu::RenderPassDepthStencilAttachment {
            view: self.depth_stencil.texture().view(),
            depth_ops: self.depth_stencil.as_depth_operation()?,
            stencil_ops: self.depth_stencil.as_stencil_operation()?,
        }))
    }
}
