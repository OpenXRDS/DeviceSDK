use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::camera::CameraRenderGraph;

use crate::XrdsColor;

/// XRDS camera clear-color policy.
#[derive(Debug, Clone, Copy, Default)]
pub enum XrdsClearColorConfig {
    #[default]
    Default,
    None,
    Custom(XrdsColor),
}

impl From<XrdsClearColorConfig> for ClearColorConfig {
    fn from(value: XrdsClearColorConfig) -> Self {
        match value {
            XrdsClearColorConfig::Default => ClearColorConfig::Default,
            XrdsClearColorConfig::None => ClearColorConfig::None,
            XrdsClearColorConfig::Custom(color) => ClearColorConfig::Custom(color.into()),
        }
    }
}

/// XRDS tonemapping mode for scene cameras.
#[derive(Debug, Clone, Copy, Default)]
pub enum XrdsTonemapping {
    #[default]
    None,
    Reinhard,
    ReinhardLuminance,
    AcesFitted,
    AgX,
    SomewhatBoringDisplayTransform,
    TonyMcMapface,
    BlenderFilmic,
}

impl From<XrdsTonemapping> for Tonemapping {
    fn from(value: XrdsTonemapping) -> Self {
        match value {
            XrdsTonemapping::None => Tonemapping::None,
            XrdsTonemapping::Reinhard => Tonemapping::Reinhard,
            XrdsTonemapping::ReinhardLuminance => Tonemapping::ReinhardLuminance,
            XrdsTonemapping::AcesFitted => Tonemapping::AcesFitted,
            XrdsTonemapping::AgX => Tonemapping::AgX,
            XrdsTonemapping::SomewhatBoringDisplayTransform => {
                Tonemapping::SomewhatBoringDisplayTransform
            }
            XrdsTonemapping::TonyMcMapface => Tonemapping::TonyMcMapface,
            XrdsTonemapping::BlenderFilmic => Tonemapping::BlenderFilmic,
        }
    }
}

/// XRDS bloom preset for scene cameras.
#[derive(Debug, Clone, Copy, Default)]
pub enum XrdsBloom {
    #[default]
    Disabled,
    Natural,
    OldSchool,
}

impl XrdsBloom {
    pub fn to_bevy(self) -> Option<Bloom> {
        match self {
            Self::Disabled => None,
            Self::Natural => Some(Bloom::NATURAL),
            Self::OldSchool => Some(Bloom::OLD_SCHOOL),
        }
    }
}

/// Implemented by any type that describes how to configure a camera in the Bevy world.
///
/// Built-in implementations: [`PerspectiveCameraParams`], [`OrthographicCameraParams`].
/// Implement this trait on your own struct for custom projection or render pipeline setups.
pub trait CameraKind: Send + 'static {
    /// Insert the appropriate Bevy camera component(s) into `entity`.
    fn insert_into(self, entity: &mut EntityWorldMut);
}

/// Blueprint parameters for a standard perspective camera.
#[derive(Debug, Clone, Copy)]
pub struct PerspectiveCameraParams {
    /// Vertical field of view in degrees.
    pub fov_deg: f32,
    pub near: f32,
    /// `None` enables an infinite far plane (no depth precision loss at distance).
    pub far: Option<f32>,
    pub order: isize,
}

impl Default for PerspectiveCameraParams {
    fn default() -> Self {
        Self {
            fov_deg: 60.0,
            near: 0.1,
            far: None,
            order: 0,
        }
    }
}

impl CameraKind for PerspectiveCameraParams {
    fn insert_into(self, entity: &mut EntityWorldMut) {
        let mut projection = PerspectiveProjection {
            fov: self.fov_deg.to_radians(),
            near: self.near,
            ..default()
        };
        if let Some(far) = self.far {
            projection.far = far;
        }
        entity.insert((
            CameraRenderGraph::new(Core3d),
            Camera3d::default(),
            Projection::Perspective(projection),
        ));
        if let Some(mut camera) = entity.get_mut::<Camera>() {
            camera.order = self.order;
        }
    }
}

/// Blueprint parameters for an orthographic camera (UI overlays, top-down views).
#[derive(Debug, Clone, Copy)]
pub struct OrthographicCameraParams {
    pub scale: f32,
    pub near: f32,
    pub far: f32,
    pub order: isize,
}

impl Default for OrthographicCameraParams {
    fn default() -> Self {
        Self {
            scale: 1.0,
            near: -1000.0,
            far: 1000.0,
            order: 0,
        }
    }
}

impl CameraKind for OrthographicCameraParams {
    fn insert_into(self, entity: &mut EntityWorldMut) {
        entity.insert((
            CameraRenderGraph::new(Core3d),
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                scale: self.scale,
                near: self.near,
                far: self.far,
                ..OrthographicProjection::default_3d()
            }),
        ));
        if let Some(mut camera) = entity.get_mut::<Camera>() {
            camera.order = self.order;
        }
    }
}

/// Authored camera projection choice for XRDS scene cameras.
#[derive(Debug, Clone, Copy)]
pub enum CameraProjectionParams {
    Perspective(PerspectiveCameraParams),
    Orthographic(OrthographicCameraParams),
}

impl Default for CameraProjectionParams {
    fn default() -> Self {
        Self::Perspective(PerspectiveCameraParams::default())
    }
}

impl CameraKind for CameraProjectionParams {
    fn insert_into(self, entity: &mut EntityWorldMut) {
        match self {
            Self::Perspective(params) => params.insert_into(entity),
            Self::Orthographic(params) => params.insert_into(entity),
        }
    }
}
