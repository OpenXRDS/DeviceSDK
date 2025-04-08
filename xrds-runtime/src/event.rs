use uuid::Uuid;
use xrds_core::Transform;
use xrds_graphics::{Fov, XrdsTexture};

#[derive(Debug, Clone)]
pub enum WorldEvent<'a> {
    OnCameraUpdated(WorldOnCameraUpdated<'a>),
}

#[derive(Debug, Clone)]
pub struct WorldOnCameraUpdated<'a> {
    pub camera_id: &'a Uuid,
    pub camera_update_infos: Vec<(Fov, Transform)>,
    pub copy_target: Option<XrdsTexture>,
}
