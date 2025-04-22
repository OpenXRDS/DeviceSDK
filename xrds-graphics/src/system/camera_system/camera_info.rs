use glam::{Mat4, Vec2};
use xrds_core::Transform;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewParams {
    pub curr_view_projection: glam::Mat4,
    pub prev_view_projection: glam::Mat4,
    pub curr_jitter: glam::Vec2,
    pub prev_jitter: glam::Vec2,
    pub inv_view_projection: glam::Mat4,
    pub view: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub projection: glam::Mat4,
    pub inv_projection: glam::Mat4,
    pub world_position: glam::Vec3,
    pub _pad: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum ProjectionType {
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone, Default, Copy)]
pub struct Fov {
    pub left: f32,
    pub right: f32,
    pub up: f32,
    pub down: f32,
}

/// Single view camera with basic view parameters
#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub fov: Fov,
    pub projection_type: ProjectionType,
    pub far: f32,
    pub near: f32,
}

impl Default for CameraInfo {
    fn default() -> Self {
        CameraInfo {
            fov: Fov {
                left: 45.0f32.to_radians(),
                right: 45.0f32.to_radians(),
                down: 45.0f32.to_radians(),
                up: 45.0f32.to_radians(),
            },
            projection_type: ProjectionType::Perspective,
            far: 10000.0,
            near: 0.05,
        }
    }
}

impl CameraInfo {
    pub fn new(fov: Fov, projection_type: ProjectionType, far: f32, near: f32) -> Self {
        CameraInfo {
            fov,
            projection_type,
            far,
            near,
        }
    }

    pub fn as_view_params(
        &self,
        transform: &Transform,
        jitter: Vec2,
        viewport_width: u32,
        viewport_height: u32,
    ) -> ViewParams {
        let position = transform.get_translation();
        let orientation = transform.get_rotation();

        // Calculate unjittered view matrix
        let view_mat = Mat4::look_at_rh(
            position,
            position + orientation * glam::Vec3::Z,
            orientation * glam::Vec3::Y,
        );
        let inv_view_mat = view_mat.inverse();

        // Calculate unjittered projection matrix
        let proj_mat = match self.projection_type {
            ProjectionType::Perspective => self.as_perspective_projection(),
            ProjectionType::Orthographic => self.as_orthogonal_projection(),
        };

        // Apply jitter to projection
        let mut jittered_proj_mat = proj_mat;
        let mut jitter_ndc = Vec2::default();
        if viewport_width > 0 && viewport_height > 0 {
            jitter_ndc = Vec2::new(
                jitter.x * 2.0 / viewport_width as f32,
                jitter.y * 2.0 / viewport_height as f32,
            );

            let mut jitter_mat = Mat4::IDENTITY;
            jitter_mat.col_mut(3)[0] = jitter_ndc.x;
            jitter_mat.col_mut(3)[1] = jitter_ndc.y;

            jittered_proj_mat = jitter_mat.mul_mat4(&proj_mat);

            // // Add jitter to translation component
            // jittered_proj_mat.col_mut(2)[0] += offset_ndc_x;
            // jittered_proj_mat.col_mut(2)[1] += offset_ndc_y;
        }

        let curr_view_proj_mat = jittered_proj_mat.mul_mat4(&view_mat);
        let inv_proj_mat = jittered_proj_mat.inverse();
        let inv_view_proj_mat = curr_view_proj_mat.inverse();

        ViewParams {
            curr_view_projection: curr_view_proj_mat,
            prev_view_projection: Mat4::IDENTITY, // fill with unit matrix
            curr_jitter: jitter_ndc,
            prev_jitter: Vec2::ZERO,
            inv_view_projection: inv_view_proj_mat,
            view: view_mat,
            inv_view: inv_view_mat,
            projection: jittered_proj_mat,
            inv_projection: inv_proj_mat,
            world_position: position,
            _pad: 0,
        }
    }

    fn as_perspective_projection(&self) -> glam::Mat4 {
        let tan_left = self.fov.left.tan();
        let tan_right = self.fov.right.tan();
        let tan_down = self.fov.down.tan();
        let tan_up = self.fov.up.tan();

        let tan_width = tan_right - tan_left;
        let tan_height = tan_up - tan_down;

        if tan_width <= 0.0 || tan_height <= 0.0 {
            log::error!("Invalid fov for perspective projection: {:?}", self.fov);
            return Mat4::IDENTITY;
        }

        let a00 = 2.0 / tan_width;
        let a11 = 2.0 / tan_height;

        let a20 = (tan_right + tan_left) / tan_width;
        let a21 = (tan_up + tan_down) / tan_height;
        let a22 = -self.far / (self.far - self.near);

        let a32 = -(self.far * self.near) / (self.far - self.near);

        glam::Mat4::from_cols_array(&[
            a00, 0.0, 0.0, 0.0, //
            0.0, a11, 0.0, 0.0, //
            a20, a21, a22, -1.0, //
            0.0, 0.0, a32, 0.0, //
        ])
    }

    fn as_orthogonal_projection(&self) -> glam::Mat4 {
        let tan_left = self.fov.left.tan();
        let tan_right = self.fov.right.tan();
        let tan_bottom = self.fov.down.tan();
        let tan_top = self.fov.up.tan();

        if tan_right <= tan_left || tan_top <= tan_bottom {
            log::error!(
                "Invalid boundaries for orthographic projection: l={}, r={}, b={}, t={}",
                tan_left,
                tan_right,
                tan_bottom,
                tan_top
            );
            return glam::Mat4::IDENTITY;
        }

        let a00 = 2.0 / (tan_right - tan_left);
        let a11 = 2.0 / (tan_top - tan_bottom);
        let a22 = -2.0 / (self.far - self.near);

        let a30 = -(tan_right + tan_left) / (tan_right - tan_left);
        let a31 = -(tan_top + tan_bottom) / (tan_top - tan_bottom);
        let a32 = -(self.far + self.near) / (self.far - self.near);

        glam::Mat4::from_cols_array(&[
            a00, 0.0, 0.0, 0.0, //
            0.0, a11, 0.0, 0.0, //
            0.0, 0.0, a22, 0.0, //
            a30, a31, a32, 1.0, //
        ])
    }

    pub fn set_fov(&mut self, fov: Fov) {
        self.fov = fov;
    }

    pub fn get_fov(&self) -> Fov {
        self.fov
    }
}
