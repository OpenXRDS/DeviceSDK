use crate::{
    default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject,
    XrdsPhysicsBody,
};

#[derive(Debug, Clone)]
pub struct XrdsCapsule {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub radius: f32,
    /// Length of the straight segment, **excluding** the two hemispherical
    /// caps — matching both `bevy::math::primitives::Capsule3d` and avian3d's
    /// `Collider::capsule`. Total visible height is `length + 2 * radius`.
    pub length: f32,
    pub physics_body: XrdsPhysicsBody,
    pub gravity_scale: f32,
    pub mass: f32,
}

impl XrdsCapsule {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            radius: 0.5,
            length: 1.0,
            physics_body: XrdsPhysicsBody::None,
            gravity_scale: 1.0,
            mass: 1.0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl XrdsObject for XrdsCapsule {
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

impl XrdsComponent for XrdsCapsule {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsCapsule {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
