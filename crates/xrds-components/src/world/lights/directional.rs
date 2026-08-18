use crate::{
    default_component_name, TransformParams, XrdsColor, XrdsComponent, XrdsMutableComponent,
    XrdsObject,
};

#[derive(Debug, Clone)]
pub struct XrdsDirectionalLight {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub color: XrdsColor,
    pub illuminance: f32,
    pub shadows: bool,
}

impl XrdsDirectionalLight {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            color: XrdsColor::WHITE,
            illuminance: 1000.0,
            shadows: false,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl XrdsObject for XrdsDirectionalLight {
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

impl XrdsComponent for XrdsDirectionalLight {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsDirectionalLight {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
