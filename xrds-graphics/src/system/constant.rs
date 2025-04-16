pub struct Constant {}

impl Constant {
    pub const BIND_GROUP_ID_VIEW_PARAMS: u32 = 0;
    pub const BIND_GROUP_ID_TEXTURE_INPUT: u32 = 1;
    pub const BIND_GROUP_ID_SKINNING: u32 = 2;
    pub const BIND_GROUP_ID_SHADOWMAP_LIGHT: u32 = 0;
    pub const BIND_GROUP_ID_LIGHT: u32 = 3;

    pub const VERTEX_ID_INSTANCES: u32 = 0;
    /// VertexID is started from 1. 0 is reserved for instance buffer
    pub const VERTEX_ID_BASEMENT: u32 = 1;

    pub const SHADOWMAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;
    pub const MAX_SHADOWMAP_COUNT: usize = 32;

    pub const INTERMEDIATE_RENDER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
    pub const LIGHT_TYPE_POINT: u32 = 1;
    pub const LIGHT_TYPE_SPOT: u32 = 2;
}
