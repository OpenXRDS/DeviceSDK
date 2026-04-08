use crate::{
    default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject,
};

#[derive(Debug, Clone)]
pub struct XrdsPlane3D {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub size: [f32; 2],
}

impl XrdsPlane3D {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            size: [1.0, 1.0],
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl XrdsObject for XrdsPlane3D {
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

impl XrdsComponent for XrdsPlane3D {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsPlane3D {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
