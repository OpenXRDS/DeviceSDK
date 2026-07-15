use bevy::prelude::{Component, Entity, Message};
use crate::XrGrabHand;

/// Bevy component on a world-space slider root entity.
///
/// Spawned via `api.spawn_world_slider(panel_handle, params)`. The entity is an invisible
/// transform anchor; visual children (track quad + thumb quad) are attached as `ChildOf` it.
///
/// While the XR trigger is held over the slider track, `world_ui_slider_system` continuously
/// maps the pointer UV to a value in `[min, max]` and fires [`XrWorldSliderChangeEvent`].
#[derive(Component, Debug, Clone)]
pub struct XrdsWorldSlider {
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Track width × height in metres.
    pub size: [f32; 2],
    pub min: f32,
    pub max: f32,
    pub value: f32,
    /// Background track colour. RGBA 0–1.
    pub track_color: [f32; 4],
    /// Fill / accent colour (reserved for a future fill quad). RGBA 0–1.
    pub fill_color: [f32; 4],
    /// Thumb knob colour. RGBA 0–1.
    pub thumb_color: [f32; 4],
    /// Thumb side length in metres.
    pub thumb_size: f32,
    /// Set by the slider system during an active drag; do not mutate directly.
    pub dragging_hand: Option<XrGrabHand>,
}

impl XrdsWorldSlider {
    /// Normalised position of `value` in `[0, 1]`.
    pub fn normalized(&self) -> f32 {
        if (self.max - self.min).abs() < 1e-9 { return 0.0; }
        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    /// Thumb X offset (metres, relative to slider root) for the current value.
    pub fn thumb_x(&self) -> f32 {
        (self.normalized() - 0.5) * (self.size[0] - self.thumb_size)
    }
}

/// Parameters for [`super::super::XrdsAPI::spawn_world_slider`].
#[derive(Debug, Clone)]
pub struct XrdsWorldSliderParams {
    pub min: f32,
    pub max: f32,
    pub value: f32,
    /// Track width × height in metres.
    pub size: [f32; 2],
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Background track colour. RGBA 0–1.
    pub track_color: [f32; 4],
    /// Fill / accent colour (reserved). RGBA 0–1.
    pub fill_color: [f32; 4],
    /// Thumb knob colour. RGBA 0–1.
    pub thumb_color: [f32; 4],
    /// Thumb side length in metres.
    pub thumb_size: f32,
}

impl Default for XrdsWorldSliderParams {
    fn default() -> Self {
        Self {
            min:   0.0,
            max:   1.0,
            value: 0.5,
            size:            [0.20, 0.012],
            local_position:  [0.0,  0.0],
            track_color:     [0.12, 0.12, 0.12, 0.90],
            fill_color:      [0.25, 0.40, 0.80, 1.00],
            thumb_color:     [0.85, 0.85, 0.85, 1.00],
            thumb_size:      0.022,
        }
    }
}

/// Fired while an XR trigger is held and the slider value changes.
///
/// # Example
/// ```ignore
/// for ev in ctx.world_slider_changes() {
///     if ev.slider_entity == vol_slider.entity() {
///         audio.set_volume(ev.value);
///     }
/// }
/// ```
#[derive(Debug, Clone, Message)]
pub struct XrWorldSliderChangeEvent {
    /// The Bevy entity of the [`XrdsWorldSlider`] root that changed.
    pub slider_entity: Entity,
    /// New value in `[min, max]`.
    pub value: f32,
    /// Which hand is dragging.
    pub hand: XrGrabHand,
}
