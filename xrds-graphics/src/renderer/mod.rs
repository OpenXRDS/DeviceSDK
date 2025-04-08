mod command_encoder;
mod framebuffer;
mod gbuffer;
mod render_pass;
mod shadowmap_pool;

pub use command_encoder::*;
pub use framebuffer::*;
use log::debug;
pub use render_pass::*;
pub use shadowmap_pool::*;

use wgpu::{CommandEncoderDescriptor, Origin3d, TexelCopyTextureInfo};

use crate::{Constant, GraphicsInstance, TextureFormat, XrdsTexture};

#[derive(Debug, Clone)]
pub struct Renderer {
    graphics_instance: GraphicsInstance,
    framebuffers: Vec<Framebuffer>,
    framebuffer_index: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ViewInfo {
    pub view_projection: glam::Mat4,
    pub cam_position: glam::Vec3,
}

impl Renderer {
    pub fn new(
        graphics_instance: GraphicsInstance,
        output_format: TextureFormat,
        extent: wgpu::Extent3d,
        framebuffer_count: u32,
    ) -> anyhow::Result<Self> {
        let framebuffers: Vec<_> = (0..framebuffer_count)
            .map(|_| Framebuffer::new(&graphics_instance, extent, output_format))
            .collect();
        debug!(
            "framebuffers created. view_count: {}",
            extent.depth_or_array_layers
        );

        Ok(Self {
            graphics_instance,
            framebuffers,
            framebuffer_index: 0,
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

        Ok(RenderPass::new(render_pass, RenderPassType::PbrGbuffer))
    }

    pub fn create_shadow_pass<'encoder>(
        &mut self,
        encoder: &'encoder mut CommandEncoder,
    ) -> anyhow::Result<RenderPass<'encoder>> {
        let framebuffer = self.get_current_framebuffer();
        let wgpu_render_pass =
            encoder
                .encoder_mut()
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &framebuffer.final_color_attachment()?,
                    // color_attachments: &framebuffer.shadowmap_attachments()?,
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

        Ok(RenderPass::new(wgpu_render_pass, RenderPassType::PbrShadow))
    }

    pub fn create_lighting_pass<'encoder>(
        &mut self,
        encoder: &'encoder mut CommandEncoder,
    ) -> anyhow::Result<RenderPass<'encoder>> {
        let framebuffer = self.get_current_framebuffer();

        let wgpu_render_pass =
            encoder
                .encoder_mut()
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &framebuffer.final_color_attachment()?,
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

        Ok(RenderPass::new(wgpu_render_pass, RenderPassType::PbrLight))
    }

    pub fn do_deferred_lighting(&mut self, render_pass: &mut RenderPass<'_>) -> anyhow::Result<()> {
        let framebuffer = self.get_current_framebuffer();
        render_pass.set_bind_group(
            Constant::BIND_GROUP_ID_GBUFFER,
            framebuffer.gbuffer_bind_group(),
            &[],
        );
        // self.deferred_lighting_proc.encode(render_pass);
        Ok(())
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

    pub fn get_current_framebuffer(&self) -> &Framebuffer {
        let framebuffer = self.framebuffers.get(self.framebuffer_index).unwrap();

        framebuffer
    }
}
