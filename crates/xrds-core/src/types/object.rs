use glam::{Quat, Vec3};

#[derive(Debug, Clone)]
pub struct XrdsWorldComponentInner {
    position: Vec3,
    roation: Quat,
    scale: Vec3,
}

impl XrdsWorldComponentInner {
    pub fn position(&self) -> &Vec3 {
        &self.position
    }
    pub fn roation(&self) -> &Quat {
        &self.roation
    }
    pub fn scale(&self) -> &Vec3 {
        &self.scale
    }
    pub fn position_mut(&mut self) -> &mut Vec3 {
        &mut self.position
    }
    pub fn roation_mut(&mut self) -> &mut Quat {
        &mut self.roation
    }
    pub fn scale_mut(&mut self) -> &mut Vec3 {
        &mut self.scale
    }
}
