use bevy::prelude::*;
use xrds_runtime::XrdsUpdateContext;
use crate::bevy_bridge::BevyBridgeResource;
use crate::bridge::EditorCommand;
use crate::editor_state::EditorState;

/// Intercept keyboard shortcuts when the Bevy window has focus (after any viewport click).
/// Without this, shortcuts typed after orbiting/selecting would be silently consumed by
/// winit and never reach the React keydown handler in the WebView.
///
/// File-dialog shortcuts (Ctrl+O, Ctrl+I, Ctrl+Shift+S/E/A) are intentionally omitted —
/// those require a path from the OS dialog and can still be triggered from the menubar.
pub fn keyboard_shortcut_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    bridge:   Res<BevyBridgeResource>,
    state:    Res<EditorState>,
) {
    let ctrl  = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft)   || keyboard.pressed(KeyCode::ShiftRight);

    macro_rules! push {
        ($c:expr) => { bridge.0.inbound.lock().unwrap().push_back($c); };
    }

    if ctrl {
        if      keyboard.just_pressed(KeyCode::KeyZ) { push!(EditorCommand::Undo); }
        else if keyboard.just_pressed(KeyCode::KeyY) { push!(EditorCommand::Redo); }
        else if keyboard.just_pressed(KeyCode::KeyC) { push!(EditorCommand::CopySelection); }
        else if keyboard.just_pressed(KeyCode::KeyX) { push!(EditorCommand::CutSelection); }
        else if keyboard.just_pressed(KeyCode::KeyV) { push!(EditorCommand::PasteClipboard); }
        else if keyboard.just_pressed(KeyCode::KeyD) { push!(EditorCommand::DuplicateSelection); }
        else if keyboard.just_pressed(KeyCode::KeyN) { push!(EditorCommand::NewScene); }
        else if keyboard.just_pressed(KeyCode::KeyS) && !shift { push!(EditorCommand::SaveScene); }
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        if state.is_playing { push!(EditorCommand::SetPlayMode { playing: false }); }
        else { push!(EditorCommand::DeselectAll); }
    } else if keyboard.just_pressed(KeyCode::F5) {
        push!(EditorCommand::SetPlayMode { playing: !state.is_playing });
    } else if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        push!(EditorCommand::DeleteSelection);
    } else if keyboard.just_pressed(KeyCode::KeyT) {
        push!(EditorCommand::SetGizmoMode { mode: "Translate".to_string() });
    } else if keyboard.just_pressed(KeyCode::KeyR) {
        push!(EditorCommand::SetGizmoMode { mode: "Rotate".to_string() });
    } else if keyboard.just_pressed(KeyCode::KeyY) {
        push!(EditorCommand::SetGizmoMode { mode: "Scale".to_string() });
    } else if keyboard.just_pressed(KeyCode::KeyG) {
        push!(EditorCommand::ToggleGrid);
    }
    // F and WASD are handled directly by orbit_camera_system.
}

/// Press **Z** in play mode to fire a debug ray from the active camera and log hits.
/// Remove this system once raycasting is wired into real gameplay logic.
pub fn raycast_debug_system(world: &mut World) {
    if !world.resource::<EditorState>().is_playing { return; }
    if !world.resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::KeyZ) { return; }

    // Capture camera pose before creating XrdsUpdateContext (separate borrow).
    let cam_pose: Option<(Vec3, Vec3)> = {
        let mut q = world.query_filtered::<(&GlobalTransform, &Camera), With<Camera3d>>();
        q.iter(world)
            .find(|(_, cam)| cam.is_active)
            .map(|(gt, _)| {
                let tf = gt.compute_transform();
                (tf.translation, tf.rotation * Vec3::NEG_Z)
            })
    };

    let Some((origin, dir)) = cam_pose else {
        info!("[raycast] Z: no active camera found");
        return;
    };

    let hits = XrdsUpdateContext::new(world).raycast(origin, dir, 100.0);
    if hits.is_empty() {
        info!("[raycast] Z: no hits  origin={origin:.2?}  dir={dir:.2?}");
    } else {
        for h in &hits {
            info!("[raycast] hit id={:?}  dist={:.2}m  pt={:.3?}", h.id, h.distance, h.point);
        }
    }
}
