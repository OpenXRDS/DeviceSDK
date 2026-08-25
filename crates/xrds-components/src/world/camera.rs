use crate::{
    default_component_name, CameraProjectionParams, OrthographicCameraParams,
    PerspectiveCameraParams, TransformParams, XrdsBloom, XrdsClearColorConfig, XrdsComponent,
    XrdsMutableComponent, XrdsObject, XrdsTonemapping,
};

/// Blueprint node representing a camera in the scene tree.
///
/// Authored data here is the design-time default. At runtime the Bevy `Camera`,
/// `Projection`, and `Transform` components are the live state.
///
/// For XR eye cameras (driven by OpenXR tracking), use the `xrds-openxr` crate instead.
#[derive(Debug, Clone)]
pub struct XrdsCamera {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub projection: CameraProjectionParams,
    /// Look-at target in world space. When `Some`, overrides transform rotation on spawn.
    pub look_at: Option<[f32; 3]>,
    pub clear_color: XrdsClearColorConfig,
    pub tonemapping: XrdsTonemapping,
    pub bloom: XrdsBloom,
}

impl XrdsCamera {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            projection: CameraProjectionParams::default(),
            look_at: None,
            clear_color: XrdsClearColorConfig::default(),
            tonemapping: XrdsTonemapping::default(),
            bloom: XrdsBloom::default(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn perspective(fov_deg: f32) -> Self {
        let mut camera = Self::new();
        camera.projection = CameraProjectionParams::Perspective(PerspectiveCameraParams {
            fov_deg,
            ..PerspectiveCameraParams::default()
        });
        camera
    }

    pub fn perspective_with_params(params: PerspectiveCameraParams) -> Self {
        let mut camera = Self::new();
        camera.projection = CameraProjectionParams::Perspective(params);
        camera
    }

    pub fn orthographic(scale: f32) -> Self {
        let mut camera = Self::new();
        camera.projection = CameraProjectionParams::Orthographic(OrthographicCameraParams {
            scale,
            ..OrthographicCameraParams::default()
        });
        camera
    }

    pub fn orthographic_with_params(params: OrthographicCameraParams) -> Self {
        let mut camera = Self::new();
        camera.projection = CameraProjectionParams::Orthographic(params);
        camera
    }

    pub fn at(mut self, translation: [f32; 3]) -> Self {
        self.transform.translation = translation;
        self
    }

    pub fn near(mut self, near: f32) -> Self {
        match &mut self.projection {
            CameraProjectionParams::Perspective(params) => params.near = near,
            CameraProjectionParams::Orthographic(params) => params.near = near,
        }
        self
    }

    pub fn far(mut self, far: f32) -> Self {
        match &mut self.projection {
            CameraProjectionParams::Perspective(params) => params.far = Some(far),
            CameraProjectionParams::Orthographic(params) => params.far = far,
        }
        self
    }

    pub fn order(mut self, order: isize) -> Self {
        match &mut self.projection {
            CameraProjectionParams::Perspective(params) => params.order = order,
            CameraProjectionParams::Orthographic(params) => params.order = order,
        }
        self
    }

    pub fn looking_at(mut self, target: [f32; 3]) -> Self {
        self.look_at = Some(target);
        self
    }

    pub fn with_clear_color(mut self, clear_color: XrdsClearColorConfig) -> Self {
        self.clear_color = clear_color;
        self
    }

    pub fn with_tonemapping(mut self, tonemapping: XrdsTonemapping) -> Self {
        self.tonemapping = tonemapping;
        self
    }

    pub fn with_bloom(mut self, bloom: XrdsBloom) -> Self {
        self.bloom = bloom;
        self
    }
}

impl XrdsObject for XrdsCamera {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsCamera {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsCamera {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
