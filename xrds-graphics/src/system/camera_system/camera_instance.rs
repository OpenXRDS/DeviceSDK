use uuid::Uuid;
use xrds_core::Transform;

use crate::{CopySwapchainProc, Framebuffer, GraphicsInstance};

use super::{CameraInfo, Fov};

#[derive(Debug, Clone)]
pub struct CameraInstance {
    pub(crate) camera_entity_id: Uuid,
    pub(crate) cameras: Vec<CameraInfo>,
    pub(crate) transforms: Vec<Transform>,
    pub(crate) cam_uniform_buffer: wgpu::Buffer,
    pub(crate) cam_bind_group: wgpu::BindGroup,
    pub(crate) framebuffers: Vec<Framebuffer>,
    pub(crate) framebuffer_index: usize,
    pub(crate) copy_swapchain_proc: Option<CopySwapchainProc>,
}

impl CameraInstance {
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

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.cam_bind_group
    }

    pub fn bind_group_offset(&self) -> &[u32] {
        &[]
    }

    pub fn begin_frame(&mut self) {
        self.framebuffer_index = self.framebuffer_index + 1;
    }

    pub fn current_frame(&self) -> &Framebuffer {
        &self.framebuffers[self.framebuffer_index % self.framebuffers.len()]
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

    pub fn copy_swapchain_proc(&self) -> Option<&CopySwapchainProc> {
        self.copy_swapchain_proc.as_ref()
    }

    pub fn copy_swapchain_proc_mut(&mut self) -> Option<&mut CopySwapchainProc> {
        self.copy_swapchain_proc.as_mut()
    }

    pub fn encode_view_params(&self, render_pass: &mut wgpu::RenderPass<'_>, index: u32) {
        render_pass.set_bind_group(index, &self.cam_bind_group, &[]);
    }

    pub fn encode_framebuffers(&self, render_pass: &mut wgpu::RenderPass<'_>, index: u32) {
        render_pass.set_bind_group(index, self.current_frame().gbuffer_bind_group(), &[]);
    }
}
