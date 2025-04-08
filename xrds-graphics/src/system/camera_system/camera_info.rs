use glam::Mat4;
use xrds_core::Transform;

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

    pub fn as_view_params(&self, transform: &Transform) -> ViewParams {
        let position = transform.get_translation();
        let orientation = transform.get_rotation();

        let view_mat = Mat4::look_at_rh(
            position,
            position + orientation * glam::Vec3::Z,
            orientation * glam::Vec3::Y,
        );

        let proj_mat = match self.projection_type {
            ProjectionType::Perspective => self.as_perspective_projections(),
            ProjectionType::Orthographic => self.as_orthogonal_projections(),
        };
        let inv_view_mat = view_mat.inverse();
        let view_proj_mat = proj_mat.mul_mat4(&view_mat);
        let inv_proj_mat = proj_mat.inverse();
        let inv_view_proj_mat = view_proj_mat.inverse();

        ViewParams {
            view_projection: view_proj_mat,
            inv_view_projection: inv_view_proj_mat,
            view: view_mat,
            inv_view: inv_view_mat,
            projection: proj_mat,
            inv_projection: inv_proj_mat,
            world_position: position,
            _pad: 0,
        }
    }

    fn as_perspective_projections(&self) -> glam::Mat4 {
        let tan_left = self.fov.left.tan();
        let tan_right = self.fov.right.tan();
        let tan_down = self.fov.down.tan();
        let tan_up = self.fov.up.tan();

        let tan_width = tan_right - tan_left;
        let tan_height = tan_up - tan_down;

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

    fn as_orthogonal_projections(&self) -> glam::Mat4 {
        let tan_left = self.fov.left.tan();
        let tan_right = self.fov.right.tan();
        let tan_down = self.fov.down.tan();
        let tan_up = self.fov.up.tan();

        let a00 = 2.0 / (tan_right - tan_left);
        let a11 = 2.0 / (tan_up - tan_down);
        let a22 = -2.0 / (self.far - self.near);

        let a30 = -(tan_right + tan_left) / (tan_right - tan_left);
        let a31 = -(tan_up + tan_down) / (tan_up - tan_down);
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
