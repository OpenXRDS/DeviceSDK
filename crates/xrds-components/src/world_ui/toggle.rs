use bevy::prelude::{Component, Entity, Message};
use crate::XrGrabHand;

/// Bevy component on a world-space toggle root entity.
///
/// Spawned via `api.spawn_world_toggle(panel_handle, params)`. The entity is an invisible
/// transform anchor; visual children (track quad + thumb quad) are attached as `ChildOf` it.
///
/// A single trigger press while hovering the toggle flips [`checked`](XrdsWorldToggle::checked)
/// and fires [`XrWorldToggleEvent`]. Use `ctx.set_world_toggle(handle, bool)` to set state
/// programmatically.
#[derive(Component, Debug, Clone)]
pub struct XrdsWorldToggle {
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Track width × height in metres. The thumb is a square of side `height * 0.85`.
    pub size: [f32; 2],
    pub checked: bool,
    /// Track colour when `checked == false`. RGBA 0–1.
    pub track_off_color: [f32; 4],
    /// Track colour when `checked == true`. RGBA 0–1.
    pub track_on_color: [f32; 4],
    /// Thumb knob colour. RGBA 0–1.
    pub thumb_color: [f32; 4],
}

impl XrdsWorldToggle {
    /// Thumb X offset (metres, relative to toggle root) for the current state.
    pub fn thumb_x(&self) -> f32 {
        let travel = self.size[0] * 0.5 - self.size[1] * 0.85 * 0.5;
        if self.checked { travel } else { -travel }
    }
}

/// Parameters for `XrdsAPI::spawn_world_toggle`.
#[derive(Debug, Clone)]
pub struct XrdsWorldToggleParams {
    pub checked: bool,
    /// Track width × height in metres.
    pub size: [f32; 2],
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Track colour when off. RGBA 0–1.
    pub track_off_color: [f32; 4],
    /// Track colour when on. RGBA 0–1.
    pub track_on_color: [f32; 4],
    /// Thumb knob colour. RGBA 0–1.
    pub thumb_color: [f32; 4],
}

impl Default for XrdsWorldToggleParams {
    fn default() -> Self {
        Self {
            checked:         false,
            size:            [0.10, 0.035],
            local_position:  [0.0,  0.0],
            track_off_color: [0.15, 0.15, 0.15, 0.90],
            track_on_color:  [0.20, 0.55, 0.25, 0.95],
            thumb_color:     [0.90, 0.90, 0.90, 1.00],
        }
    }
}

/// Fired when the toggle's [`checked`](XrdsWorldToggle::checked) state changes.
///
/// # Example
/// ```ignore
/// for ev in ctx.world_toggle_events() {
///     if ev.toggle_entity == shadows_toggle.entity() {
///         ctx.set_shadows_enabled(ev.checked);
///     }
/// }
/// ```
#[derive(Debug, Clone, Message)]
pub struct XrWorldToggleEvent {
    /// The Bevy entity of the [`XrdsWorldToggle`] root that changed.
    pub toggle_entity: Entity,
    /// New state.
    pub checked: bool,
    /// Which hand triggered the flip.
    pub hand: XrGrabHand,
}
