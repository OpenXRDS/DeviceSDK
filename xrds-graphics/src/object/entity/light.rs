use glam::Vec3;

use crate::{LightColor, LightDescription, LightType};

#[derive(Debug, Clone)]
pub struct LightComponent {
    pub description: LightDescription,
}

impl LightComponent {
    pub fn new(light_description: &LightDescription) -> Self {
        Self {
            description: light_description.clone(),
        }
    }

    pub fn light_type(&self) -> &LightType {
        &self.description.ty
    }

    pub fn color(&self) -> &Vec3 {
        &self.description.color
    }

    pub fn intensity(&self) -> f32 {
        self.description.intensity
    }

    pub fn range(&self) -> Option<f32> {
        match self.description.ty {
            LightType::Directional => None,
            LightType::Point(description) => Some(description.range),
            LightType::Spot(description) => Some(description.range),
        }
    }

    pub fn cast_shadow(&self) -> bool {
        self.description.cast_shadow
    }

    pub fn set_light_type(&mut self, light_type: LightType) {
        self.description.ty = light_type;
    }

    pub fn set_color(&mut self, color: Vec3) {
        self.description.color = color;
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.description.intensity = intensity;
    }

    pub fn set_range(&mut self, range: f32) {
        match &mut self.description.ty {
            LightType::Directional => {}
            LightType::Point(description) => description.range = range,
            LightType::Spot(description) => description.range = range,
        }
    }

    pub fn set_cast_shadow(&mut self, cast_shadow: bool) {
        self.description.cast_shadow = cast_shadow
    }
}

impl Default for LightComponent {
    fn default() -> Self {
        Self {
            description: LightDescription {
                color: LightColor::DIRECT_SUNLIGHT,
                intensity: 1.0,
                ty: LightType::Directional,
                cast_shadow: true,
            },
        }
    }
}
