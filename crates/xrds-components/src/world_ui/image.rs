use bevy::prelude::Component;

/// Bevy component on a world-space image entity.
///
/// Spawned as a child of an [`super::panel::XrdsWorldPanel`] via
/// `api.spawn_world_image(panel_handle, params)`. The entity holds a flat quad
/// mesh with the specified texture applied as the base colour map.
#[derive(Component, Debug, Clone)]
pub struct XrdsWorldImage {
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Image width × height in metres.
    pub size: [f32; 2],
}

/// Parameters for `XrdsAPI::spawn_world_image`.
#[derive(Debug, Clone)]
pub struct XrdsWorldImageParams {
    /// Asset path relative to the `assets/` directory (e.g. `"textures/icon.png"`).
    pub asset_path: String,
    /// Image width × height in metres.
    pub size: [f32; 2],
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Multiplicative tint applied over the texture. RGBA 0–1; `[1,1,1,1]` = no tint.
    pub tint: [f32; 4],
}

impl Default for XrdsWorldImageParams {
    fn default() -> Self {
        Self {
            asset_path: String::new(),
            size: [0.10, 0.10],
            local_position: [0.0, 0.0],
            tint: [1.0, 1.0, 1.0, 1.0],
        }
    }
}
