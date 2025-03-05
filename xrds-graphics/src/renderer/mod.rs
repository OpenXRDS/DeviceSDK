mod command_encoder;
mod framebuffer;
mod render_pass;

pub use command_encoder::*;
pub use framebuffer::*;
pub use render_pass::*;
use wgpu::{
    Color, CommandEncoderDescriptor, LoadOp, Operations, Origin3d, StoreOp, TexelCopyTextureInfo,
    TextureDescriptor, TextureViewDescriptor,
};

use std::sync::Arc;

use crate::{
    GraphicsInstance, RenderTargetOps, RenderTargetTexture, TextureFormat, XrdsScene, XrdsTexture,
};

#[derive(Debug, Clone)]
pub struct Renderer {
    graphics_instance: Arc<GraphicsInstance>,
    framebuffers: Vec<Framebuffer>,
    framebuffer_index: usize,
}

impl Renderer {
    pub fn new(
        graphics_instance: Arc<GraphicsInstance>,
        color_format: TextureFormat,
        extent: wgpu::Extent3d,
        framebuffer_count: u32,
    ) -> Self {
        let framebuffers: Vec<_> = (0..framebuffer_count)
            .map(|_| {
                let wgpu_color_texture =
                    graphics_instance
                        .device()
                        .create_texture(&TextureDescriptor {
                            label: None,
                            size: extent,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: color_format.as_wgpu(),
                            usage: wgpu::TextureUsages::COPY_SRC
                                | wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        });
                let wgpu_color_view =
                    wgpu_color_texture.create_view(&TextureViewDescriptor::default());
                let color_texture =
                    XrdsTexture::new(wgpu_color_texture, color_format, extent, wgpu_color_view);
                let wgpu_depth_stencil_texture =
                    graphics_instance
                        .device()
                        .create_texture(&TextureDescriptor {
                            label: None,
                            size: extent,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Depth24PlusStencil8,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                            view_formats: &[],
                        });
                let wgpu_depth_stencil_view =
                    wgpu_depth_stencil_texture.create_view(&TextureViewDescriptor::default());
                let depth_stencil_texture = XrdsTexture::new(
                    wgpu_depth_stencil_texture,
                    wgpu::TextureFormat::Depth24PlusStencil8.into(),
                    extent,
                    wgpu_depth_stencil_view,
                );
                Framebuffer::new(
                    &[RenderTargetTexture::new(
                        color_texture,
                        RenderTargetOps::ColorAttachment(Operations {
                            load: LoadOp::Clear(Color::GREEN),
                            store: StoreOp::Store,
                        }),
                    )],
                    Some(RenderTargetTexture::new(
                        depth_stencil_texture,
                        RenderTargetOps::DepthStencilAttachment {
                            depth_ops: Some(Operations {
                                load: LoadOp::Clear(0.0),
                                store: StoreOp::Store,
                            }),
                            stencil_ops: Some(Operations {
                                load: LoadOp::Clear(0),
                                store: StoreOp::Store,
                            }),
                        },
                    )),
                )
            })
            .collect();
        Self {
            graphics_instance,
            framebuffers,
            framebuffer_index: 0,
        }
    }

    pub fn on_pre_render(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn on_post_render(&mut self) -> anyhow::Result<()> {
        let new_index = self.framebuffer_index + 1;
        self.framebuffer_index = new_index % self.framebuffers.len();

        Ok(())
    }

    pub fn create_command_encoder(&mut self) -> anyhow::Result<CommandEncoder> {
        let command_encoder = self
            .graphics_instance
            .device()
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        Ok(CommandEncoder::new(command_encoder))
    }

    pub fn create_gbuffer_pass<'encoder>(
        &mut self,
        encoder: &'encoder mut CommandEncoder,
    ) -> anyhow::Result<RenderPass<'encoder>> {
        let framebuffer = self.get_current_framebuffer();

        let render_pass = encoder
            .encoder_mut()
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &framebuffer.color_attachments()?,
                depth_stencil_attachment: framebuffer.depth_stencil_attachment()?,
                ..Default::default()
            });

        Ok(RenderPass::new(render_pass))
    }

    pub fn copy_render_result(
        &self,
        command_encoder: &mut CommandEncoder,
        target_texture: &XrdsTexture,
    ) -> anyhow::Result<()> {
        let framebuffer = self.get_current_framebuffer();
        let encoder = command_encoder.encoder_mut();

        // todo!()
        let final_color = framebuffer.color_textures()[0];
        let final_color_texture: &XrdsTexture = final_color.texture();

        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: final_color_texture.texture(),
                origin: Origin3d::ZERO,
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
            },
            TexelCopyTextureInfo {
                texture: target_texture.texture(),
                origin: Origin3d::ZERO,
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
            },
            *target_texture.size(),
        );

        Ok(())
    }

    pub fn summit(&self, command_encoder: CommandEncoder) -> anyhow::Result<()> {
        let command_buffer = command_encoder.end();
        self.graphics_instance.queue().submit([command_buffer]);

        Ok(())
    }

    pub fn load_scene(&mut self) -> anyhow::Result<XrdsScene> {
        Ok(XrdsScene {})
    }

    fn get_current_framebuffer(&self) -> &Framebuffer {
        let framebuffer = self.framebuffers.get(self.framebuffer_index).unwrap();

        framebuffer
    }
}
