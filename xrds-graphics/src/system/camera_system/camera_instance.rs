use uuid::Uuid;
use xrds_core::Transform;

use crate::{generate_halton_sequence, Constant, CopySwapchainProc, Framebuffer, GraphicsInstance};

use super::{CameraInfo, Fov, ViewParams};

#[derive(Debug, Clone)]
pub struct CameraInstance {
    pub(crate) graphics_instance: GraphicsInstance,
    pub(crate) camera_entity_id: Uuid,
    pub(crate) cameras: Vec<CameraInfo>,
    pub(crate) view_params: Vec<ViewParams>,
    pub(crate) transforms: Vec<Transform>,
    pub(crate) cam_uniform_buffer: wgpu::Buffer,
    pub(crate) cam_bind_group: wgpu::BindGroup,
    pub(crate) framebuffer: Framebuffer,
    pub(crate) copy_swapchain_proc: Option<CopySwapchainProc>,
    pub(crate) frame_index: u64,
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
        self.framebuffer.begin_frame();
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    pub fn framebuffer_mut(&mut self) -> &mut Framebuffer {
        &mut self.framebuffer
    }

    pub fn update_framebuffer(&mut self, framebuffer: Framebuffer) {
        self.framebuffer = framebuffer;
    }

    pub fn update_view_params(&mut self) {
        let jitter = get_jitter_offset(self.frame_index);

        let extent = self.framebuffer.extent();
        let viewport_width = extent.width;
        let viewport_height = extent.height;

        let mut next_view_params = Vec::new();

        for i in 0..self.cameras.len() {
            let camera_info = &self.cameras[i];
            let transform = &self.transforms[i];

            let base_params =
                camera_info.as_view_params(transform, jitter, viewport_width, viewport_height);

            let previous_params = if self.frame_index > 1 {
                &self.view_params[i]
            } else {
                &base_params
            };

            let view_params = ViewParams {
                prev_view_projection: previous_params.curr_view_projection,
                prev_jitter: previous_params.curr_jitter,
                curr_view_projection: base_params.curr_view_projection,
                curr_jitter: base_params.curr_jitter,
                world_position: base_params.world_position,
                projection: base_params.projection,
                inv_projection: base_params.inv_projection,
                view: base_params.view,
                inv_view: base_params.inv_view,
                inv_view_projection: base_params.inv_view_projection,
                _pad: 0,
            };

            next_view_params.push(view_params);
        }

        self.view_params = next_view_params;
        self.frame_index += 1;
    }

    pub fn update_uniform(&self) {
        self.graphics_instance.queue().write_buffer(
            &self.cam_uniform_buffer,
            0,
            bytemuck::cast_slice(&self.view_params),
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
        render_pass.set_bind_group(index, self.framebuffer().gbuffer_bind_group(), &[]);
    }
}

pub fn get_jitter_offset(frame_index: u64) -> glam::Vec2 {
    let sample_index: u64 = (frame_index % Constant::TAA_SAMPLE_COUNT) + 1;

    let jitter_x = generate_halton_sequence(sample_index, 2) - 0.5;
    let jitter_y = generate_halton_sequence(sample_index, 3) - 0.5;

    glam::Vec2::new(jitter_x, jitter_y)
}
