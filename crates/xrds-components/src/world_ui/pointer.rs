use bevy::math::{Vec2, Vec3};
use bevy::prelude::{Entity, Resource};
use crate::{XrdsId, XrGrabHand};

/// Per-frame pointer state for the world-space UI system, one entry per XR hand.
///
/// Updated each frame by `world_ui_pointer_system` in `xrds-runtime`.
/// Read this resource in `XrdsUpdateContext::update()` to check where the pointer
/// is hovering without consuming events.
#[derive(Resource, Default, Clone)]
pub struct XrdsWorldPointerState {
    pub left: Option<XrdsWorldPointerHit>,
    pub right: Option<XrdsWorldPointerHit>,
}

impl XrdsWorldPointerState {
    pub fn for_hand(&self, hand: XrGrabHand) -> Option<&XrdsWorldPointerHit> {
        match hand {
            XrGrabHand::Left  => self.left.as_ref(),
            XrGrabHand::Right => self.right.as_ref(),
        }
    }
}

/// A resolved pointer hit on a world-space panel surface.
#[derive(Debug, Clone)]
pub struct XrdsWorldPointerHit {
    /// Bevy entity of the hit `XrdsWorldSurface`.
    pub entity: Entity,
    /// XRDS id of the panel.
    pub panel_id: XrdsId,
    /// Panel-local UV: (0,0) = bottom-left, (1,1) = top-right.
    pub uv: Vec2,
    /// World-space intersection point (used to position the cursor visual).
    pub world_point: Vec3,
}

/// Fired once when a hand's pointer ray enters a [`XrdsWorldSurface`].
///
/// Only fires on the transition frame (enter), not every frame while hovering.
/// Read via `ctx.world_hover_enters()` in `XrdsApp::update`.
///
/// [`XrdsWorldSurface`]: crate::world_ui::XrdsWorldSurface
#[derive(Debug, Clone, bevy::prelude::Message)]
pub struct XrWorldHoverEnterEvent {
    pub panel_id: XrdsId,
    pub hand: XrGrabHand,
    /// Panel-local UV at entry point.
    pub uv: Vec2,
}

/// Fired once when a hand's pointer ray leaves a [`XrdsWorldSurface`].
///
/// [`XrdsWorldSurface`]: crate::world_ui::XrdsWorldSurface
#[derive(Debug, Clone, bevy::prelude::Message)]
pub struct XrWorldHoverExitEvent {
    pub panel_id: XrdsId,
    pub hand: XrGrabHand,
}

/// Stores the Bevy entity IDs of the two pointer cursor visuals (one per hand).
/// Populated by `spawn_world_ui_cursors_system` at `Startup`.
#[derive(Resource, Default, Clone)]
pub struct XrdsWorldPointerCursors {
    pub left: Option<Entity>,
    pub right: Option<Entity>,
}
