use uuid::Uuid;
use xrds_core::Transform;

use crate::{Constant, Framebuffer, GraphicsInstance, Postproc, XrdsTexture};

use super::{CameraInfo, Fov};

#[derive(Debug, Clone)]
pub struct CameraData {
    pub(crate) camera_entity_id: Uuid,
    pub(crate) cameras: Vec<CameraInfo>,
    pub(crate) transforms: Vec<Transform>,
    pub(crate) cam_uniform_buffer: wgpu::Buffer,
    pub(crate) cam_bind_group: wgpu::BindGroup,
    pub(crate) framebuffer: Framebuffer,
    pub(crate) copy_target: Option<XrdsTexture>,
    pub(crate) deferred_lighting: Postproc,
}

impl CameraData {
    pub fn cameras(&self) -> &[CameraInfo] {
        &self.cameras
    }

    pub fn transforms(&self) -> &[Transform] {
        &self.transforms
    }

    pub fn count(&self) -> u64 {
        self.cameras.len() as u64
    }

    pub fn id(&self) -> &Uuid {
        &self.camera_entity_id
    }

    pub fn get_camera_mut(&mut self, idx: usize) -> &mut CameraInfo {
        &mut self.cameras[idx]
    }

    pub fn get_transform_mut(&mut self, idx: usize) -> &mut Transform {
        &mut self.transforms[idx]
    }

    pub fn set_fovs(&mut self, fovs: &[Fov]) {
        // TODO: error handling
        fovs.iter().enumerate().for_each(|(idx, fov)| {
            self.cameras[idx].fov = fov.clone();
        });
    }

    pub fn set_transforms(&mut self, transforms: &[Transform]) {
        // TODO: error handling
        transforms.iter().enumerate().for_each(|(idx, transform)| {
            self.transforms[idx] = transform.clone();
        });
    }

    pub fn set_copy_target(&mut self, copy_target: Option<XrdsTexture>) {
        self.copy_target = copy_target;
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.cam_bind_group
    }

    pub fn bind_group_offset(&self) -> &[u32] {
        &[]
    }

    pub fn get_next_framebuffer(&self) -> &Framebuffer {
        // temporal code
        &self.framebuffer
    }

    pub fn update_uniform(&self, graphics_instance: &GraphicsInstance) {
        let view_params: Vec<_> = self
            .cameras
            .iter()
            .zip(self.transforms.iter())
            .map(|(camera, transform)| {
                let view_params = camera.as_view_params(transform);
                view_params
            })
            .collect();

        graphics_instance.queue().write_buffer(
            &self.cam_uniform_buffer,
            0,
            bytemuck::cast_slice(&view_params),
        );
    }

    pub fn get_copy_from(&self) -> wgpu::TexelCopyTextureInfo {
        // TODO: error handling
        wgpu::TexelCopyTextureInfo {
            texture: self.framebuffer.final_color().texture().wgpu_texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        }
    }

    pub fn get_copy_to(&self) -> wgpu::TexelCopyTextureInfo {
        // TODO: error handling
        wgpu::TexelCopyTextureInfo {
            texture: self.copy_target.as_ref().unwrap().wgpu_texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        }
    }

    pub fn get_copy_size(&self) -> wgpu::Extent3d {
        // TODO: error handling
        *self.framebuffer.final_color().texture().size()
    }

    pub fn encode_view_params(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_bind_group(
            Constant::BIND_GROUP_ID_VIEW_PARAMS,
            &self.cam_bind_group,
            &[],
        );
    }

    pub fn encode_framebuffers(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        let framebuffer = self.get_next_framebuffer();
        render_pass.set_bind_group(
            Constant::BIND_GROUP_ID_TEXTURE_INPUT,
            framebuffer.gbuffer_bind_group(),
            &[],
        );
    }
}
