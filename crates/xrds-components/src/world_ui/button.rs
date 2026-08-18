use bevy::prelude::{Component, Entity, Message};
use crate::XrGrabHand;

/// Bevy component on a world-space button entity.
///
/// Spawned as a child of an [`super::panel::XrdsWorldPanel`] via
/// `api.spawn_world_button(panel_handle, params)`. The `world_ui_button_system` in
/// `xrds-runtime` runs hit tests each frame and updates [`XrdsWorldButtonState`].
#[derive(Component, Debug, Clone)]
pub struct XrdsWorldButton {
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Button width × height in metres.
    pub size: [f32; 2],
    /// Background colour when idle. RGBA 0–1.
    pub normal_color: [f32; 4],
    /// Background colour when the pointer is over the button. RGBA 0–1.
    pub hover_color: [f32; 4],
    /// Background colour while the trigger is held. RGBA 0–1.
    pub pressed_color: [f32; 4],
}

/// Current interaction state of an [`XrdsWorldButton`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XrdsWorldButtonState {
    /// No pointer within button bounds.
    #[default]
    Idle,
    /// Pointer is within button bounds; trigger not held.
    Hovered,
    /// Trigger held while pointer was within button bounds.
    Pressed,
}

/// Parameters for [`super::super::XrdsAPI::spawn_world_button`].
#[derive(Debug, Clone)]
pub struct XrdsWorldButtonParams {
    pub label: String,
    /// Em size of the label text in metres.
    pub font_size: f32,
    /// Label text colour. RGBA 0–1.
    pub label_color: [f32; 4],
    /// Button width × height in metres.
    pub size: [f32; 2],
    /// Position relative to the parent panel centre, in metres (X right, Y up).
    pub local_position: [f32; 2],
    /// Background colour when idle. RGBA 0–1.
    pub normal_color: [f32; 4],
    /// Background colour when hovered. RGBA 0–1.
    pub hover_color: [f32; 4],
    /// Background colour when pressed. RGBA 0–1.
    pub pressed_color: [f32; 4],
}

impl Default for XrdsWorldButtonParams {
    fn default() -> Self {
        Self {
            label: "Button".to_string(),
            font_size: 0.04,
            label_color: [1.0, 1.0, 1.0, 1.0],
            size: [0.20, 0.06],
            local_position: [0.0, 0.0],
            normal_color:  [0.15, 0.15, 0.15, 0.90],
            hover_color:   [0.25, 0.40, 0.80, 0.95],
            pressed_color: [0.10, 0.20, 0.55, 1.00],
        }
    }
}

/// Fired when a controller trigger is pressed while the pointer is within a button's bounds.
///
/// # Example
/// ```ignore
/// for ev in ctx.world_button_presses() {
///     if ev.button_entity == my_btn.entity() {
///         ctx.set_world_label_text(&lbl, "Pressed!");
///     }
/// }
/// ```
#[derive(Debug, Clone, Message)]
pub struct XrWorldButtonPressEvent {
    /// The Bevy entity of the [`XrdsWorldButton`] that was pressed.
    pub button_entity: Entity,
    /// Which hand triggered the press.
    pub hand: XrGrabHand,
}

/// Fired when the controller trigger is released after an [`XrWorldButtonPressEvent`].
#[derive(Debug, Clone, Message)]
pub struct XrWorldButtonReleaseEvent {
    /// The Bevy entity of the [`XrdsWorldButton`] that was released.
    pub button_entity: Entity,
    /// Which hand released the trigger.
    pub hand: XrGrabHand,
}
