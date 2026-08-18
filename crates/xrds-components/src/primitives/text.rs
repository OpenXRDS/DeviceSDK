use crate::{
    default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XrdsTextAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum XrdsTextAnchor {
    #[default]
    World,
    Billboard,
    ComfortPinned { depth_m: f32 },
    BodyLocked,
    HeadLocked,
    Cylindrical { radius_m: f32 },
}

#[derive(Debug, Clone)]
pub struct XrdsText {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
    pub alignment: XrdsTextAlignment,
    pub anchor: XrdsTextAnchor,
}

impl XrdsText {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            text: "Text".to_string(),
            font_size: 24.0,
            color: [1.0, 1.0, 1.0, 1.0],
            alignment: XrdsTextAlignment::Center,
            anchor: XrdsTextAnchor::World,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }
}

impl XrdsObject for XrdsText {
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

impl XrdsComponent for XrdsText {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsText {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
