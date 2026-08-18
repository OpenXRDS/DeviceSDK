use bevy::math::Vec2;
use bevy::prelude::Component;

/// Marks a panel entity as an interactive world-space raycast surface.
///
/// Inserted automatically by the runtime when a [`XrdsWorldPanel`] is spawned.
/// The world-UI pointer system uses this component to perform precise ray-vs-plane
/// intersection and convert world hit points to panel-local UV coordinates (0..1, 0..1).
///
/// The panel's front face is its local +Z direction. The pointer must hit the front
/// face (i.e. arrive from the +Z side) for a hit to register.
///
/// [`XrdsWorldPanel`]: crate::world_ui::XrdsWorldPanel
#[derive(Component, Debug, Clone)]
pub struct XrdsWorldSurface {
    /// Panel dimensions in metres (local X = width, local Y = height).
    pub size: Vec2,
    /// Whether this surface accepts pointer input. Set to `false` to temporarily
    /// disable a panel without despawning it.
    pub enabled: bool,
}

impl XrdsWorldSurface {
    pub fn new(width: f32, height: f32) -> Self {
        Self { size: Vec2::new(width, height), enabled: true }
    }
}
