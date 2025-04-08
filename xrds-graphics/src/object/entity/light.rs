use crate::XrdsLight;

#[derive(Debug, Clone)]
pub struct LightComponent {
    pub light: XrdsLight,
}

impl LightComponent {
    pub fn new(light: XrdsLight) -> Self {
        Self { light }
    }
}
