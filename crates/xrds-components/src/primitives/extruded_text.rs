use crate::{
    default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject,
};

#[derive(Debug, Clone)]
pub struct XrdsExtrudedText {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
    /// Z-axis extrusion depth in world units. 0.0 = flat front face only.
    pub depth: f32,
    pub alignment: XrdsExtrudedTextAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XrdsExtrudedTextAlignment {
    Left,
    #[default]
    Center,
    Right,
}

impl XrdsExtrudedText {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            text: "Text".to_string(),
            font_size: 24.0,
            color: [1.0, 1.0, 1.0, 1.0],
            depth: 0.1,
            alignment: XrdsExtrudedTextAlignment::Center,
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

    pub fn with_depth(mut self, depth: f32) -> Self {
        self.depth = depth;
        self
    }
}

impl XrdsObject for XrdsExtrudedText {
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

impl XrdsComponent for XrdsExtrudedText {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsExtrudedText {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
