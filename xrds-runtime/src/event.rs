use uuid::Uuid;
use xrds_openxr::XrRenderParams;

#[derive(Debug, Clone)]
pub enum WorldEvent<'a> {
    OnCameraUpdated(WorldOnCameraUpdated<'a>),
}

#[derive(Debug, Clone)]
pub struct WorldOnCameraUpdated<'a> {
    pub camera_id: &'a Uuid,
    pub params: &'a XrRenderParams,
}
