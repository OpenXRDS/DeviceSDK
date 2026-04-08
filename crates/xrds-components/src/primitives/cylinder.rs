use crate::{
    default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject,
};

#[derive(Debug, Clone)]
pub struct XrdsCylinder {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub radius: f32,
    pub height: f32,
}

impl XrdsCylinder {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            radius: 0.5,
            height: 1.0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl XrdsObject for XrdsCylinder {
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

impl XrdsComponent for XrdsCylinder {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsCylinder {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
