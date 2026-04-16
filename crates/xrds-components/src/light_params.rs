use bevy::prelude::*;

use crate::XrdsColor;

/// Implemented by any type that describes how to instantiate a light into the Bevy world.
///
/// The three built-in implementations are [`PointLightParams`], [`DirectionalLightParams`],
/// and [`SpotLightParams`]. Implement this trait on your own struct to define custom light types.
pub trait LightKind: Send + 'static {
    /// Insert the appropriate Bevy light component(s) into `entity`.
    fn insert_into(self, entity: &mut EntityWorldMut);
}

pub struct AmbientLightParams {
    pub color: XrdsColor,
    pub brightness: f32,
    pub affects_baked_lighting: bool,
}

impl Default for AmbientLightParams {
    fn default() -> Self {
        Self {
            color: XrdsColor::WHITE,
            brightness: 1.0,
            affects_baked_lighting: true,
        }
    }
}

impl LightKind for AmbientLightParams {
    fn insert_into(self, entity: &mut EntityWorldMut) {
        entity.world_scope(|world| {
            world.insert_resource(AmbientLight {
                color: self.color.into(),
                brightness: self.brightness,
                affects_lightmapped_meshes: self.affects_baked_lighting,
            });
        });
    }
}

/// Blueprint parameters for an omnidirectional point source.
#[derive(Debug, Clone, Copy)]
pub struct PointLightParams {
    pub color: XrdsColor,
    pub intensity: f32,
    pub range: f32,
    pub radius: f32,
    pub shadows: bool,
}

impl Default for PointLightParams {
    fn default() -> Self {
        Self {
            color: XrdsColor::WHITE,
            intensity: 1_000_000.0,
            range: 20.0,
            radius: 0.0,
            shadows: false,
        }
    }
}

impl LightKind for PointLightParams {
    fn insert_into(self, entity: &mut EntityWorldMut) {
        entity.insert(PointLight {
            color: self.color.into(),
            intensity: self.intensity,
            range: self.range,
            radius: self.radius,
            shadows_enabled: self.shadows,
            ..default()
        });
    }
}

/// Blueprint parameters for an infinite parallel (sun-like) light.
#[derive(Debug, Clone, Copy)]
pub struct DirectionalLightParams {
    pub color: XrdsColor,
    /// Illuminance in lux.
    pub illuminance: f32,
    pub shadows: bool,
}

impl Default for DirectionalLightParams {
    fn default() -> Self {
        Self {
            color: XrdsColor::WHITE,
            illuminance: 10_000.0,
            shadows: false,
        }
    }
}

impl LightKind for DirectionalLightParams {
    fn insert_into(self, entity: &mut EntityWorldMut) {
        entity.insert(DirectionalLight {
            color: self.color.into(),
            illuminance: self.illuminance,
            shadows_enabled: self.shadows,
            ..default()
        });
    }
}

/// Blueprint parameters for a cone-shaped spot light.
#[derive(Debug, Clone, Copy)]
pub struct SpotLightParams {
    pub color: XrdsColor,
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub shadows: bool,
}

impl Default for SpotLightParams {
    fn default() -> Self {
        Self {
            color: XrdsColor::WHITE,
            intensity: 1_000_000.0,
            range: 20.0,
            inner_angle: 0.2,
            outer_angle: 0.5,
            shadows: false,
        }
    }
}

impl LightKind for SpotLightParams {
    fn insert_into(self, entity: &mut EntityWorldMut) {
        entity.insert(SpotLight {
            color: self.color.into(),
            intensity: self.intensity,
            range: self.range,
            inner_angle: self.inner_angle,
            outer_angle: self.outer_angle,
            shadows_enabled: self.shadows,
            ..default()
        });
    }
}
