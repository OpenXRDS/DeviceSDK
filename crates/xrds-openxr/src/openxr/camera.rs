use bevy::{
    camera::{CameraProjection, SubCameraView},
    prelude::*,
    render::extract_component::{ExtractComponent, ExtractComponentPlugin},
};

#[derive(Debug, Clone)]
pub struct OpenXrViewProjection {
    pub projection_matrix: Mat4,
    pub near: f32,
}

impl Default for OpenXrViewProjection {
    fn default() -> Self {
        Self {
            projection_matrix: Mat4::IDENTITY,
            near: 0.1,
        }
    }
}

/// Component for camera transform following HMD view
#[derive(Component, ExtractComponent, Clone, Copy, Debug, Default)]
#[require(Camera3d, Transform)]
pub struct OpenXrCamera;

#[derive(Component, ExtractComponent, Clone, Copy, Debug, Default)]
#[require(Camera3d)]
pub struct OpenXrCameraIndex(pub u32);

#[derive(Debug, Default)]
pub struct OpenXrCameraPlugin;

impl Plugin for OpenXrCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<OpenXrCameraIndex>::default());
    }
}

// CameraProjection implementation from bevy_openxr camera.rs
impl CameraProjection for OpenXrViewProjection {
    fn update(&mut self, _width: f32, _height: f32) {}

    fn far(&self) -> f32 {
        self.projection_matrix.to_cols_array()[14]
            / (self.projection_matrix.to_cols_array()[10] + 1.0)
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        fn normalized_corner(inverse_matrix: &Mat4, near: f32, ndc_x: f32, ndc_y: f32) -> Vec3A {
            let clip_pos = Vec4::new(ndc_x * near, ndc_y * near, near, near);
            // I don't know why multiplying the Z axis by -1 is necessary.
            // As far as I can tell from (likely my incorrect understanding of the code),
            // PerspectiveProjection::get_frustum_corners() has the Z axis inverted??
            Vec3A::from_vec4(inverse_matrix.mul_vec4(clip_pos)) / near * Vec3A::new(1., 1., -1.)
        }

        let inv = self.projection_matrix.inverse();
        let norm_br = normalized_corner(&inv, self.near, 1., -1.);
        let norm_tr = normalized_corner(&inv, self.near, 1., 1.);
        let norm_tl = normalized_corner(&inv, self.near, -1., 1.);
        let norm_bl = normalized_corner(&inv, self.near, -1., -1.);

        [
            norm_br * z_near,
            norm_tr * z_near,
            norm_tl * z_near,
            norm_bl * z_near,
            norm_br * z_far,
            norm_tr * z_far,
            norm_tl * z_far,
            norm_bl * z_far,
        ]
    }

    fn get_clip_from_view(&self) -> Mat4 {
        self.projection_matrix
    }

    fn get_clip_from_view_for_sub(&self, _sub_view: &SubCameraView) -> Mat4 {
        panic!("sub view not supported for xr camera");
    }
}
