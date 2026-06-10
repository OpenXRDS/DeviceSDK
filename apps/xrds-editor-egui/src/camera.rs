use xrds::editor::{
    bevy_ecs, ButtonInput, Camera3d, Commands, Component, EguiContexts, EulerRot, MessageReader,
    KeyCode, MouseButton, MouseMotion, MouseScrollUnit, MouseWheel, Quat, Query, Res, ResMut,
    Resource, Transform, Vec2, Vec3, With,
};

use crate::state::{CameraMode, EditorSession, EditorState};

// ── Tuning ────────────────────────────────────────────────────────────────────

const ORBIT_MOUSE_SENSITIVITY: f32 = 0.007;
const PAN_SENSITIVITY: f32 = 0.0025;
const ZOOM_LINE_SENSITIVITY: f32 = 0.12;
const ZOOM_PIXEL_SENSITIVITY: f32 = 0.004;
const FLY_SPEED: f32 = 0.08;            // world-units per frame
const FLY_FAST_MULT: f32 = 4.0;
const MIN_DISTANCE: f32 = 0.05;
const MAX_DISTANCE: f32 = 2000.0;

// ── Components / Resources ────────────────────────────────────────────────────

#[derive(Component)]
pub struct EditorCameraMarker;

#[derive(Resource)]
pub struct EditorCameraState {
    /// World-space look-at / orbit pivot.
    pub pivot: Vec3,
    /// Distance from camera to pivot.
    pub distance: f32,
    /// Horizontal angle (radians, rotation around world-Y).
    pub yaw: f32,
    /// Vertical angle (radians, positive = tilted down).
    pub pitch: f32,
    /// Pending (yaw, pitch) snap set by view-preset buttons; consumed in one frame.
    pub view_snap: Option<(f32, f32)>,
    /// Distance saved when entering fly mode so orbit can be restored.
    pub fly_saved_distance: f32,
}

impl Default for EditorCameraState {
    fn default() -> Self {
        Self {
            pivot: Vec3::ZERO,
            distance: 8.0,
            yaw: -0.5,
            pitch: 0.45,
            view_snap: None,
            fly_saved_distance: 8.0,
        }
    }
}

impl EditorCameraState {
    pub fn to_transform(&self) -> Transform {
        let rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        let position = self.pivot + rotation * Vec3::new(0.0, 0.0, self.distance);
        Transform { translation: position, rotation, ..Default::default() }
    }
}

// ── Systems ───────────────────────────────────────────────────────────────────

pub fn spawn_editor_camera(mut commands: Commands) {
    commands.spawn((Camera3d::default(), EditorCameraMarker));
}

/// Per-frame camera update.
///
/// **Orbit mode** (default):
/// | Input                          | Action                          |
/// |--------------------------------|---------------------------------|
/// | Middle drag                    | Orbit                           |
/// | Shift + Middle drag            | Pan in view-plane               |
/// | Scroll wheel                   | Zoom                            |
/// | Arrow keys                     | Orbit (keyboard, slow)          |
/// | WASD / Q / E                   | Move pivot through scene        |
///
/// **Fly mode** (toggle via toolbar button):
/// | Input                          | Action                          |
/// |--------------------------------|---------------------------------|
/// | RMB hold + mouse               | Free-look (yaw / pitch)         |
/// | WASD / Q / E                   | Fly camera position (2× speed)  |
/// | Shift (+ WASD)                 | Fast fly                        |
/// | Scroll wheel                   | Adjust fly speed multiplier     |
pub fn orbit_camera_system(
    mut cam: ResMut<EditorCameraState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut camera_q: Query<&mut Transform, With<EditorCameraMarker>>,
    mut contexts: EguiContexts,
    mut editor_state: ResMut<EditorState>,
    session: Res<EditorSession>,
) {
    // In play mode the pawn locomotion system drives the active camera instead.
    if editor_state.is_playing {
        for _ in mouse_motion.read() {}
        for _ in mouse_wheel.read() {}
        return;
    }

    // ── Egui guard ────────────────────────────────────────────────────────────
    let egui_wants_pointer = contexts
        .ctx_mut()
        .map(|ctx| ctx.wants_pointer_input())
        .unwrap_or(false);
    let egui_wants_keyboard = contexts
        .ctx_mut()
        .map(|ctx| ctx.wants_keyboard_input())
        .unwrap_or(false);

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let middle = mouse_buttons.pressed(MouseButton::Middle);
    let right  = mouse_buttons.pressed(MouseButton::Right);
    let is_fly = editor_state.camera_mode == CameraMode::Fly;

    // Accumulate mouse delta.
    let mut delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        delta += Vec2::new(ev.delta.x, ev.delta.y);
    }

    // ── View snap (set by view-preset buttons in the viewport overlay) ────────
    if let Some((snap_yaw, snap_pitch)) = cam.view_snap.take() {
        cam.yaw   = snap_yaw;
        cam.pitch = snap_pitch;
    }

    // ── Mode-dependent look / orbit ───────────────────────────────────────────
    if is_fly {
        // Fly mode: hold RMB to free-look.
        let fly_looking = right && !egui_wants_pointer;
        if fly_looking && delta != Vec2::ZERO {
            cam.yaw -= delta.x * ORBIT_MOUSE_SENSITIVITY;
            cam.pitch = (cam.pitch + delta.y * ORBIT_MOUSE_SENSITIVITY)
                .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        }
        // MMB still pans even in fly mode.
        let panning = middle && shift && !egui_wants_pointer;
        if panning && delta != Vec2::ZERO {
            let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
            let factor = cam.distance * PAN_SENSITIVITY;
            cam.pivot -= rot * Vec3::X * delta.x * factor;
            cam.pivot += rot * Vec3::Y * delta.y * factor;
        }
    } else {
        // Orbit mode.
        let orbiting = middle && !shift && !egui_wants_pointer;
        let panning   = middle &&  shift && !egui_wants_pointer;
        if orbiting && delta != Vec2::ZERO {
            cam.yaw -= delta.x * ORBIT_MOUSE_SENSITIVITY;
            cam.pitch = (cam.pitch + delta.y * ORBIT_MOUSE_SENSITIVITY)
                .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        }
        if panning && delta != Vec2::ZERO {
            let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
            let factor = cam.distance * PAN_SENSITIVITY;
            cam.pivot -= rot * Vec3::X * delta.x * factor;
            cam.pivot += rot * Vec3::Y * delta.y * factor;
        }
    }

    // ── Zoom (scroll wheel) ───────────────────────────────────────────────────
    if !egui_wants_pointer {
        for ev in mouse_wheel.read() {
            let scroll = match ev.unit {
                MouseScrollUnit::Line  => ev.y * ZOOM_LINE_SENSITIVITY,
                MouseScrollUnit::Pixel => ev.y * ZOOM_PIXEL_SENSITIVITY,
            };
            cam.distance = (cam.distance * (1.0 - scroll)).clamp(MIN_DISTANCE, MAX_DISTANCE);
        }
    } else {
        for _ in mouse_wheel.read() {}
    }

    // ── WASD / Q / E movement ─────────────────────────────────────────────────
    if !egui_wants_keyboard {
        let base_speed = if is_fly { FLY_SPEED * 2.0 } else { FLY_SPEED };
        let speed = if shift { base_speed * FLY_FAST_MULT } else { base_speed };
        let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
        let forward = rot * -Vec3::Z;
        let right_v  = rot * Vec3::X;
        let up       = Vec3::Y;

        if keyboard.pressed(KeyCode::KeyW) { cam.pivot += forward * speed; }
        if keyboard.pressed(KeyCode::KeyS) { cam.pivot -= forward * speed; }
        if keyboard.pressed(KeyCode::KeyA) { cam.pivot -= right_v * speed; }
        if keyboard.pressed(KeyCode::KeyD) { cam.pivot += right_v * speed; }
        if keyboard.pressed(KeyCode::KeyE) { cam.pivot += up * speed; }
        if keyboard.pressed(KeyCode::KeyQ) { cam.pivot -= up * speed; }
    }

    // ── Frame Selected (F key) ────────────────────────────────────────────────
    if let Some(target) = editor_state.frame_selected_target.take() {
        cam.pivot = Vec3::from_array(target);
    } else if !egui_wants_keyboard && keyboard.just_pressed(KeyCode::KeyF) {
        if let Some(sel_id) = editor_state.selection.primary() {
            if let Some(node) = session.document().node(sel_id) {
                let [tx, ty, tz] = node.transform.translation;
                cam.pivot = Vec3::new(tx, ty, tz);
            }
        }
    }

    // ── Apply transform ──────────────────────────────────────────────────────
    let t = cam.to_transform();
    for mut transform in camera_q.iter_mut() {
        *transform = t;
    }
}
