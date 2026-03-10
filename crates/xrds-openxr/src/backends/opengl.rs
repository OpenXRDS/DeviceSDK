use bevy::{
    math::Mat4,
    render::settings::{RenderResources, WgpuSettings},
};
use wgpu::Extent3d;

use crate::{
    backends::{GraphicsInner, OpenXrGraphicsBackend},
    openxr::{
        graphics::{OpenXrGraphicsExtend, OpenXrGraphicsFamily, OpenXrGraphicsWrap},
        session::OpenXrSessionCreateInfo,
    },
};

unsafe impl OpenXrGraphicsExtend for openxr::OpenGL {
    fn wrap<G: OpenXrGraphicsFamily>(inner: G::Inner<Self>) -> OpenXrGraphicsWrap<G> {
        OpenXrGraphicsWrap::OpenGl(inner)
    }
}

impl GraphicsInner<openxr::OpenGL> {}

impl OpenXrGraphicsBackend<openxr::OpenGL> for GraphicsInner<openxr::OpenGL> {
    fn initialize(
        _openxr_instance: &openxr::Instance,
        _system_id: openxr::SystemId,
        _openxr_appinfo: &openxr::ApplicationInfo,
        _wgpu_settings: WgpuSettings,
    ) -> anyhow::Result<super::OpenXrGraphicsBackends> {
        unimplemented!()
    }

    fn get_render_resource(&self) -> anyhow::Result<RenderResources> {
        unimplemented!()
    }

    fn get_session_create_info(&self) -> anyhow::Result<OpenXrSessionCreateInfo> {
        unimplemented!()
    }

    fn get_swapchain_create_info(
        &self,
        _swapchain_format: wgpu::TextureFormat,
        _size: Extent3d,
        _sample_count: u32,
    ) -> anyhow::Result<openxr::SwapchainCreateInfo<openxr::OpenGL>> {
        unimplemented!()
    }

    fn swapchain_image_to_wgpu(
        &self,
        _swapchain_image: &<openxr::OpenGL as openxr::Graphics>::SwapchainImage,
        _format: wgpu::TextureFormat,
        _size: Extent3d,
        _sample_count: u32,
    ) -> anyhow::Result<wgpu::Texture> {
        unimplemented!()
    }

    fn format_from_raw(
        &self,
        _format: &<openxr::OpenGL as openxr::Graphics>::Format,
    ) -> Option<wgpu::TextureFormat> {
        unimplemented!()
    }

    fn calculate_projection_matrix(&self, near: f32, fov: openxr::Fovf) -> Mat4 {
        let far = -1.0; //   use infinite projection

        let tan_angle_left = fov.angle_left.tan();
        let tan_angle_right = fov.angle_right.tan();

        let tan_angle_down = fov.angle_down.tan();
        let tan_angle_up = fov.angle_up.tan();

        let tan_angle_width = tan_angle_right - tan_angle_left;
        let tan_angle_height = tan_angle_down - tan_angle_up;

        let offset_z = -1.0;

        let mut cols: [f32; 16] = [0.0; 16];

        if far <= near {
            // place the far plane at infinity
            cols[0] = 2.0 / tan_angle_width;
            cols[4] = 0.0;
            cols[8] = (tan_angle_right + tan_angle_left) / tan_angle_width;
            cols[12] = 0.0;

            cols[1] = 0.0;
            cols[5] = 2.0 / tan_angle_height;
            cols[9] = (tan_angle_up + tan_angle_down) / tan_angle_height;
            cols[13] = 0.0;

            cols[2] = 0.0;
            cols[6] = 0.0;
            cols[10] = -1.0;
            cols[14] = -(near + offset_z);

            cols[3] = 0.0;
            cols[7] = 0.0;
            cols[11] = -1.0;
            cols[15] = 0.0;

            //  bevy uses the _reverse_ infinite projection
            //  https://dev.theomader.com/depth-precision/
            let z_reversal = Mat4::from_cols_array_2d(&[
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0, 1.0],
            ]);

            z_reversal * Mat4::from_cols_array(&cols)
        } else {
            // normal projection
            cols[0] = 2.0 / tan_angle_width;
            cols[4] = 0.0;
            cols[8] = (tan_angle_right + tan_angle_left) / tan_angle_width;
            cols[12] = 0.0;

            cols[1] = 0.0;
            cols[5] = 2.0 / tan_angle_height;
            cols[9] = (tan_angle_up + tan_angle_down) / tan_angle_height;
            cols[13] = 0.0;

            cols[2] = 0.0;
            cols[6] = 0.0;
            cols[10] = -(far + offset_z) / (far - near);
            cols[14] = -(far * (near + offset_z)) / (far - near);

            cols[3] = 0.0;
            cols[7] = 0.0;
            cols[11] = -1.0;
            cols[15] = 0.0;

            Mat4::from_cols_array(&cols)
        }
    }
}
