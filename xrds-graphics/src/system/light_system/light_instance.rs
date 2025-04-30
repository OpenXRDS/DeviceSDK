use core::f32;

use glam::{Mat4, Vec3};
use uuid::Uuid;
use xrds_core::ViewDirection;

use crate::{Constant, LightType, XrdsLight};

#[derive(Debug, Default)]
pub struct LightInstanceState {
    view_direction: ViewDirection,
    color: Vec3,
    intensity: f32,
    range: f32,
    cast_shadow: bool,
    /// None if shadowmap is not assigned
    shadow_map_index: Option<u32>,
}

#[derive(Debug)]
pub struct LightInstance {
    entity_id: Uuid,
    light_type: LightType,
    state: LightInstanceState,
}

impl LightInstanceState {
    pub fn view_direction(&self) -> &ViewDirection {
        &self.view_direction
    }

    pub fn color(&self) -> &Vec3 {
        &self.color
    }

    pub fn intensity(&self) -> f32 {
        self.intensity
    }

    pub fn range(&self) -> f32 {
        self.range
    }

    pub fn cast_shadow(&self) -> bool {
        self.cast_shadow
    }

    pub fn shadow_map_index(&self) -> Option<u32> {
        self.shadow_map_index
    }

    pub fn set_transform(&mut self, view_direction: ViewDirection) {
        self.view_direction = view_direction;
    }

    pub fn set_color(&mut self, color: Vec3) {
        self.color = color;
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity;
    }

    pub fn set_range(&mut self, range: f32) {
        self.range = range;
    }

    pub fn set_cast_shadow(&mut self, cast_shadow: bool) {
        self.cast_shadow = cast_shadow
    }

    pub fn set_shadow_map_index(&mut self, shadow_map_index: u32) {
        self.shadow_map_index = Some(shadow_map_index);
    }
}

impl LightInstance {
    pub fn new(entity_id: Uuid, light_type: LightType) -> Self {
        Self {
            entity_id,
            light_type,
            state: LightInstanceState::default(),
        }
    }

    pub fn entity_id(&self) -> &Uuid {
        &self.entity_id
    }

    pub fn light_type(&self) -> &LightType {
        &self.light_type
    }

    pub fn state(&self) -> &LightInstanceState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut LightInstanceState {
        &mut self.state
    }

    fn view(&self) -> glam::Mat4 {
        self.state.view_direction().to_matrix()
    }

    fn projection(&self) -> glam::Mat4 {
        match self.light_type {
            LightType::Directional => {
                const DIR_LIGHT_EXTENT: f32 = 10.0; // Example size
                Mat4::orthographic_rh(
                    -DIR_LIGHT_EXTENT,
                    DIR_LIGHT_EXTENT,
                    -DIR_LIGHT_EXTENT,
                    DIR_LIGHT_EXTENT,
                    -DIR_LIGHT_EXTENT,
                    DIR_LIGHT_EXTENT,
                )
            }
            LightType::Point(point) => {
                glam::Mat4::perspective_rh(90.0f32.to_radians(), 1.0, 0.0, point.range)
            }
            LightType::Spot(spot) => {
                let outer_angle = spot.outer_cons_cos.acos();
                glam::Mat4::perspective_rh(outer_angle * 2.0, 1.0, 0.0, spot.range)
            }
        }
    }

    fn view_proj(&self) -> glam::Mat4 {
        self.projection() * self.view()
    }
}

impl From<&LightInstance> for XrdsLight {
    fn from(light_instance: &LightInstance) -> Self {
        let state = light_instance.state();
        let mut light = XrdsLight::default();
        light.view = light_instance.view();
        light.view_proj = light_instance.view_proj();
        light.cast_shadow = state.cast_shadow().then_some(1).unwrap_or(0);
        light.shadow_map_index = state.shadow_map_index().unwrap_or(0);
        light.light_color = *state.color();
        light.intensity = state.intensity();
        light.direction = state.view_direction().direction();
        light.position = state.view_direction().eye();
        light.range = state.range();
        light.light_type = match light_instance.light_type() {
            LightType::Directional => Constant::LIGHT_TYPE_DIRECTIONAL,
            LightType::Point(_) => Constant::LIGHT_TYPE_POINT,
            LightType::Spot(_) => Constant::LIGHT_TYPE_SPOT,
        };
        light.inner_cons_cos = match light_instance.light_type() {
            LightType::Spot(description) => description.inner_cons_cos,
            _ => 0.0,
        };
        light.outer_cons_cos = match light_instance.light_type() {
            LightType::Spot(description) => description.outer_cons_cos,
            _ => 0.0,
        };

        light
    }
}
