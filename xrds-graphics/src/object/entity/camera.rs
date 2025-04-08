use crate::CameraInfo;

/// Define render target type for camera
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RenderTargetType {
    #[default]
    Texture2D,
    Texture2DArray,
    Cube,
}

/// Define render action after render camera view
#[derive(Debug, Default, Clone, Copy)]
pub enum PostRenderAction {
    /// Copy render result to swapchain image (Both openxr and window).
    /// Relative render target texture must be has same extent with XR system.
    CopyFinal,
    /// Swap current render target with it's backbuffer for used by material in next frame
    SwapBackbuffer,
    #[default]
    /// Nothing to do
    None,
}

#[derive(Debug, Default, Clone)]
pub struct CameraComponent {
    cameras: Vec<CameraInfo>,
    render_target_type: RenderTargetType,
    post_render_action: PostRenderAction,
}

impl CameraComponent {
    pub fn with_camera(mut self, cameras: &[CameraInfo]) -> Self {
        self.cameras = cameras.to_vec();
        self
    }

    pub fn with_render_target_type(mut self, render_target_type: RenderTargetType) -> Self {
        self.render_target_type = render_target_type;
        self
    }

    pub fn with_post_render_action(mut self, post_render_action: PostRenderAction) -> Self {
        self.post_render_action = post_render_action;
        self
    }

    pub fn cameras(&self) -> &[CameraInfo] {
        &self.cameras
    }

    pub fn cameras_mut(&mut self) -> &mut [CameraInfo] {
        &mut self.cameras
    }

    pub fn render_target_type(&self) -> RenderTargetType {
        self.render_target_type
    }

    pub fn post_render_action(&self) -> PostRenderAction {
        self.post_render_action
    }

    pub fn set_render_target_type(&mut self, render_target_type: RenderTargetType) {
        self.render_target_type = render_target_type
    }

    pub fn set_post_render_action_mut(&mut self, post_render_action: PostRenderAction) {
        self.post_render_action = post_render_action;
    }
}
