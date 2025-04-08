pub struct Constant {}

impl Constant {
    pub const BIND_GROUP_ID_VIEW_PARAMS: u32 = 0;
    pub const BIND_GROUP_ID_TEXTURE_INPUT: u32 = 1;
    pub const BIND_GROUP_ID_SKINNING: u32 = 2;
    pub const BIND_GROUP_ID_LIGHTS: u32 = 3;
    pub const BIND_GROUP_ID_SHADOWMAPS: u32 = 4;

    pub const VERTEX_ID_INSTANCES: u32 = 0;
    /// VertexID is started from 1. 0 is reserved for instance buffer
    pub const VERTEX_ID_BASEMENT: u32 = 1;
}
