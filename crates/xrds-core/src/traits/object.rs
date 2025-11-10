use std::{fmt::Debug, time::Duration};

use glam::{Quat, Vec3};

use crate::{XrdsResource, XrdsWorldComponentInner};

pub trait XrdsObject: Debug {
    fn name(&self) -> Option<&str> {
        None
    }
    fn on_construct(&self);
    fn on_destroy(&self);
}

pub trait XrdsComponent: XrdsObject {
    fn update(&mut self, elapsed: Duration);
    fn query_resources(&self) -> Vec<XrdsResource>;
}

pub trait XrdsWorldComponent: XrdsComponent {
    fn world_component(&self) -> &XrdsWorldComponentInner;
    fn world_component_mut(&mut self) -> &mut XrdsWorldComponentInner;

    fn world_position(&self) -> &Vec3 {
        self.world_component().position()
    }
    fn world_rotation(&self) -> &Quat {
        self.world_component().roation()
    }
    fn world_scale(&self) -> &Vec3 {
        self.world_component().scale()
    }
    fn world_position_mut(&mut self) -> &mut Vec3 {
        self.world_component_mut().position_mut()
    }
    fn world_rotation_mut(&mut self) -> &mut Quat {
        self.world_component_mut().roation_mut()
    }
    fn world_scale_mut(&mut self) -> &mut Vec3 {
        self.world_component_mut().scale_mut()
    }
}
