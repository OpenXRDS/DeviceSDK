mod command_encoder;
mod framebuffer;
mod gbuffer;
mod render_pass;

pub use command_encoder::*;
pub use framebuffer::*;
use log::debug;
pub use render_pass::*;

use wgpu::{CommandEncoderDescriptor, Origin3d, TexelCopyTextureInfo};

use std::sync::Arc;

use crate::{
    create_deferred_lighting_proc, GraphicsInstance, Postproc, TextureFormat, XrdsScene,
    XrdsTexture,
};

#[derive(Debug, Clone)]
pub struct Renderer {
    graphics_instance: Arc<GraphicsInstance>,
    framebuffers: Vec<Framebuffer>,
    framebuffer_index: usize,
    deferred_lighting_proc: Postproc,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ViewInfo {
    pub view_projection: glam::Mat4,
    pub cam_position: glam::Vec3,
}

impl Renderer {
    pub fn new(
        graphics_instance: Arc<GraphicsInstance>,
        output_format: TextureFormat,
        extent: wgpu::Extent3d,
        framebuffer_count: u32,
    ) -> anyhow::Result<Self> {
        let framebuffers: Vec<_> = (0..framebuffer_count)
            .map(|_| Framebuffer::new(graphics_instance.clone(), extent, output_format))
            .collect();
        debug!("framebuffers created");

        let deferred_lighting_proc = create_deferred_lighting_proc(
            graphics_instance.clone(),
            extent.depth_or_array_layers,
            framebuffers[0].gbuffer_bind_group_layout(),
            framebuffers[0].final_color(),
        )?;

        Ok(Self {
            graphics_instance,
            framebuffers,
            framebuffer_index: 0,
            deferred_lighting_proc,
        })
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
                color_attachments: &framebuffer.gbuffer().as_color_attachments()?,
                depth_stencil_attachment: framebuffer.gbuffer().as_depth_stencil_attachment()?,
                ..Default::default()
            });

        Ok(RenderPass::new(render_pass))
    }

    pub fn create_lighting_pass<'encoder>(
        &mut self,
        encoder: &'encoder mut CommandEncoder,
    ) -> anyhow::Result<RenderPass<'encoder>> {
        let framebuffer = self.get_current_framebuffer();

        let mut wgpu_render_pass =
            encoder
                .encoder_mut()
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &framebuffer.final_color_attachment()?,
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

        self.deferred_lighting_proc
            .encode(&mut wgpu_render_pass, framebuffer.gbuffer_bind_group());

        Ok(RenderPass::new(wgpu_render_pass))
    }

    pub fn copy_render_result(
        &self,
        command_encoder: &mut CommandEncoder,
        target_texture: &XrdsTexture,
    ) -> anyhow::Result<()> {
        let framebuffer = self.get_current_framebuffer();
        let encoder = command_encoder.encoder_mut();

        let final_color = framebuffer.final_color();
        let final_color_texture: &XrdsTexture = final_color.texture();

        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: final_color_texture.wgpu_texture(),
                origin: Origin3d::ZERO,
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
            },
            TexelCopyTextureInfo {
                texture: target_texture.wgpu_texture(),
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
