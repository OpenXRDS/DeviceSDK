use std::num::{NonZeroU32, NonZeroU64};

use glam::Mat4;
use wgpu::{BindingResource, BufferBinding};

use crate::RenderPass;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewParams {
    pub view_projection: glam::Mat4,
    pub inv_view_projection: glam::Mat4,
    pub view: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub projection: glam::Mat4,
    pub inv_projection: glam::Mat4,
    pub world_position: glam::Vec3,
    pub _pad: u32,
}

#[derive(Debug, Clone, Default, Copy)]
pub struct Fov {
    pub left: f32,
    pub right: f32,
    pub up: f32,
    pub down: f32,
}

/// Single view camera with basic view parameters
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    position: glam::Vec3,
    orientation: glam::Quat,
    fov: Fov,
    near: f32,
    far: f32,
}

/// Multiple camera binding
#[derive(Debug, Clone)]
pub struct CameraBinding {
    cameras: [Camera; 2],
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Camera {
    pub fn new(
        position: glam::Vec3,
        orientation: glam::Quat,
        fov: Fov,
        near: f32,
        far: f32,
    ) -> Self {
        Camera {
            position,
            orientation,
            fov,
            near,
            far,
        }
    }

    pub fn as_view_params(&self) -> ViewParams {
        let view_mat = Mat4::look_at_rh(
            self.position,
            self.position + self.orientation * glam::Vec3::Z,
            self.orientation * glam::Vec3::Y,
        );
        let inv_view_mat = view_mat.inverse();
        let proj_mat = {
            let [tan_left, tan_right, tan_down, tan_up] =
                [self.fov.left, self.fov.right, self.fov.down, self.fov.up].map(f32::tan);
            let tan_width = tan_right - tan_left;
            let tan_height = tan_up - tan_down;

            let a11 = 2.0 / tan_width;
            let a22 = 2.0 / tan_height;

            let a31 = (tan_right + tan_left) / tan_width;
            let a32 = (tan_up + tan_down) / tan_height;
            let a33 = -self.far / (self.far - self.near);

            let a43 = -(self.far * self.near) / (self.far - self.near);

            glam::Mat4::from_cols_array(&[
                a11, 0.0, 0.0, 0.0, //
                0.0, a22, 0.0, 0.0, //
                a31, a32, a33, -1.0, //
                0.0, 0.0, a43, 0.0, //
            ])
        };
        let inv_proj_mat = proj_mat.inverse();
        let view_proj_mat = proj_mat * view_mat;
        let inv_view_proj_mat = view_proj_mat.inverse();

        ViewParams {
            view_projection: view_proj_mat,
            inv_view_projection: inv_view_proj_mat,
            view: view_mat,
            inv_view: inv_view_mat,
            projection: proj_mat,
            inv_projection: inv_proj_mat,
            world_position: self.position,
            _pad: 0,
        }
    }

    pub fn set_position(&mut self, position: glam::Vec3) {
        self.position = position;
    }

    pub fn set_orientation(&mut self, orientation: glam::Quat) {
        self.orientation = orientation;
    }

    pub fn set_fov(&mut self, fov: Fov) {
        self.fov = fov;
    }

    pub fn set_near(&mut self, near: f32) {
        self.near = near;
    }

    pub fn set_far(&mut self, far: f32) {
        self.far = far;
    }

    pub fn get_position(&self) -> glam::Vec3 {
        self.position
    }

    pub fn get_orientation(&self) -> glam::Quat {
        self.orientation
    }

    pub fn get_fov(&self) -> Fov {
        self.fov
    }

    pub fn get_near(&self) -> f32 {
        self.near
    }

    pub fn get_far(&self) -> f32 {
        self.far
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            position: glam::Vec3::ZERO,
            orientation: glam::Quat::IDENTITY,
            fov: Fov {
                left: 45.0f32.to_radians(),
                right: 45.0f32.to_radians(),
                down: 45.0f32.to_radians(),
                up: 45.0f32.to_radians(),
            },
            near: 0.05,
            far: 10000.0,
        }
    }
}

impl CameraBinding {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CameraBindings"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: NonZeroU32::new(2),
            }],
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<ViewParams>() * 2) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: NonZeroU64::new((std::mem::size_of::<ViewParams>() * 2) as u64),
                }),
            }],
        });

        CameraBinding {
            cameras: [Camera::default(), Camera::default()],
            uniform_buffer,
            bind_group,
        }
    }

    pub fn get_camera(&self, index: usize) -> &Camera {
        &self.cameras[index]
    }

    pub fn get_camera_mut(&mut self, index: usize) -> &mut Camera {
        &mut self.cameras[index]
    }

    pub fn update_uniform(&self, queue: &wgpu::Queue) {
        let view_params = self
            .cameras
            .iter()
            .map(|camera| camera.as_view_params())
            .collect::<Vec<_>>();
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&view_params));
    }

    pub fn encode(&self, render_pass: &mut RenderPass) {
        render_pass.set_bind_group(0, &self.bind_group, &[]);
    }
}
