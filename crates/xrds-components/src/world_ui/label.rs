use bevy::prelude::Component;

/// Bevy component on a world-space label entity.
///
/// Spawned as a child of an [`super::panel::XrdsWorldPanel`] entity via
/// `api.spawn_world_label(panel_handle, params)`. The entity also has `Text3d` +
/// `Text3dStyling` from `bevy_rich_text3d` so text updates work through
/// `ctx.set_world_label_text(handle, "new text")`.
#[derive(Component, Debug, Clone)]
pub struct XrdsWorldLabel {
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Slot size (width × height, metres) used by the layout system.
    /// Has no visual effect — purely a hint so VStack/HStack/Grid can allocate space.
    pub layout_size: [f32; 2],
}

/// Parameters for [`super::super::XrdsAPI::spawn_world_label`].
#[derive(Debug, Clone)]
pub struct XrdsWorldLabelParams {
    pub text: String,
    /// Em size in metres. 0.05 ≈ 5 cm tall text — comfortable reading distance at 1 m.
    pub font_size: f32,
    /// RGBA 0–1.
    pub color: [f32; 4],
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Slot size (width × height, metres) used by layout systems.
    /// Ignored when positioning is manual. Default: 20 cm × 6 cm.
    pub layout_size: [f32; 2],
}

impl Default for XrdsWorldLabelParams {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 0.05,
            color: [1.0, 1.0, 1.0, 1.0],
            local_position: [0.0, 0.0],
            layout_size: [0.20, 0.06],
        }
    }
}
