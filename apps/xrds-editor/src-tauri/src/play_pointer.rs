//! Play-mode mouse → world-UI pointer bridge.
//!
//! On desktop there is no OpenXR runtime, so `XrInput` is never populated and the
//! world-UI systems (pointer / button / slider / toggle) see no input. This system
//! synthesises the **right-hand** XR pointer from the mouse while play mode is active:
//!
//! - Pose: a ray from the pawn camera through the mouse cursor (the runtime pointer
//!   system uses the pose's -Z axis as the ray, exactly like a controller aim pose).
//! - Select/trigger: left mouse button.
//!
//! Runs in `PreUpdate` (after input processing) so the runtime's `Update`-scheduled
//! world-UI systems always read fresh state in the same frame.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use xrds_runtime::XrInput;

use crate::editor_state::EditorState;
use crate::viewport_player::PlayerPawnMarker;

pub fn mouse_world_ui_input_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    pawn_q: Query<(&Camera, &GlobalTransform), With<PlayerPawnMarker>>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<EditorState>,
    xr: Option<ResMut<XrInput>>,
) {
    let Some(mut xr) = xr else { return; };

    let clear = |xr: &mut XrInput| {
        if xr.right.pose.is_some() || xr.right.select {
            xr.right = Default::default();
        } else {
            // Edge flags must not persist across frames.
            xr.right.select_just_pressed = false;
            xr.right.select_just_released = false;
        }
    };

    if !state.is_playing {
        clear(&mut xr);
        return;
    }
    let Ok(window) = windows.single() else { clear(&mut xr); return; };
    let Ok((cam, cam_tf)) = pawn_q.single() else { clear(&mut xr); return; };
    let Some(cursor) = window.cursor_position() else { clear(&mut xr); return; };
    let Ok(ray) = cam.viewport_to_world(cam_tf, cursor) else { clear(&mut xr); return; };

    let pressed = mouse.pressed(MouseButton::Left);
    xr.right.pose = Some(
        Transform::from_translation(ray.origin).looking_to(*ray.direction, Vec3::Y),
    );
    xr.right.select = pressed;
    xr.right.trigger = if pressed { 1.0 } else { 0.0 };
    xr.right.select_just_pressed = mouse.just_pressed(MouseButton::Left);
    xr.right.select_just_released = mouse.just_released(MouseButton::Left);
}
