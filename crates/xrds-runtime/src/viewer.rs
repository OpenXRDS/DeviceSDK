// XRDS Interface Level: 1 (Default App Layer)
// Purpose: Ready-made scene viewer with orbit camera for exported XRDS applications.
// Target: Non-expert users who want to run an authored scene with minimal code.

use bevy::{
    ecs::message::MessageReader,
    input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel},
    prelude::*,
};

use crate::xrds_api::{XrdsAPI, XrdsApp};

// ── Tuning ─────────────────────────────────────────────────────────────────────

const ORBIT_SENSITIVITY: f32 = 0.007;
const PAN_SENSITIVITY: f32 = 0.0025;
const ZOOM_LINE_SENSITIVITY: f32 = 0.12;
const ZOOM_PIXEL_SENSITIVITY: f32 = 0.004;
const MOVE_SPEED: f32 = 0.08;
const FAST_MULT: f32 = 4.0;
const MIN_DISTANCE: f32 = 0.05;
const MAX_DISTANCE: f32 = 2000.0;

// ── Camera state ───────────────────────────────────────────────────────────────

/// Orbit camera state resource inserted by [`XrdsSceneViewer`].
#[derive(Resource)]
pub struct ViewerCameraState {
    pub pivot: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for ViewerCameraState {
    fn default() -> Self {
        Self { pivot: Vec3::ZERO, distance: 8.0, yaw: -0.5, pitch: 0.45 }
    }
}

impl ViewerCameraState {
    fn to_transform(&self) -> Transform {
        let rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        let position = self.pivot + rotation * Vec3::new(0.0, 0.0, self.distance);
        Transform { translation: position, rotation, ..Default::default() }
    }
}

/// Marker component added to the orbit camera entity spawned by [`XrdsSceneViewer`].
#[derive(Component)]
pub struct ViewerCamera;

// ── Systems ────────────────────────────────────────────────────────────────────

fn spawn_viewer_camera(mut commands: Commands) {
    commands.spawn((Camera3d::default(), ViewerCamera));
}

/// Orbit camera system.
///
/// | Input          | Action              |
/// |----------------|---------------------|
/// | Left drag      | Orbit               |
/// | Middle drag    | Pan                 |
/// | Scroll wheel   | Zoom                |
/// | W/S            | Move pivot fwd/back |
/// | A/D            | Move pivot left/right |
/// | Q/E            | Move pivot down/up  |
/// | Shift          | Fast movement (×4)  |
fn viewer_orbit_system(
    mut cam: ResMut<ViewerCameraState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut camera_q: Query<&mut Transform, With<ViewerCamera>>,
) {
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    let mut delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        delta += Vec2::new(ev.delta.x, ev.delta.y);
    }

    // Left drag → orbit
    if mouse_buttons.pressed(MouseButton::Left) && delta != Vec2::ZERO {
        cam.yaw -= delta.x * ORBIT_SENSITIVITY;
        cam.pitch = (cam.pitch + delta.y * ORBIT_SENSITIVITY)
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
    }

    // Middle drag → pan
    if mouse_buttons.pressed(MouseButton::Middle) && delta != Vec2::ZERO {
        let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
        let factor = cam.distance * PAN_SENSITIVITY;
        cam.pivot -= rot * Vec3::X * delta.x * factor;
        cam.pivot += rot * Vec3::Y * delta.y * factor;
    }

    // Scroll wheel → zoom
    for ev in mouse_wheel.read() {
        let scroll = match ev.unit {
            MouseScrollUnit::Line => ev.y * ZOOM_LINE_SENSITIVITY,
            MouseScrollUnit::Pixel => ev.y * ZOOM_PIXEL_SENSITIVITY,
        };
        cam.distance = (cam.distance * (1.0 - scroll)).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    // WASD / QE → move pivot
    let speed = if shift { MOVE_SPEED * FAST_MULT } else { MOVE_SPEED };
    let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
    let forward = rot * -Vec3::Z;
    let right_v = rot * Vec3::X;

    if keyboard.pressed(KeyCode::KeyW) { cam.pivot += forward * speed; }
    if keyboard.pressed(KeyCode::KeyS) { cam.pivot -= forward * speed; }
    if keyboard.pressed(KeyCode::KeyA) { cam.pivot -= right_v * speed; }
    if keyboard.pressed(KeyCode::KeyD) { cam.pivot += right_v * speed; }
    if keyboard.pressed(KeyCode::KeyE) { cam.pivot += Vec3::Y * speed; }
    if keyboard.pressed(KeyCode::KeyQ) { cam.pivot -= Vec3::Y * speed; }

    // Apply computed transform
    let t = cam.to_transform();
    for mut transform in camera_q.iter_mut() {
        *transform = t;
    }
}

// ── XrdsSceneViewer ────────────────────────────────────────────────────────────

/// A ready-made XRDS scene viewer that loads a `.xrds` document and provides
/// an orbit camera for interactive exploration.
///
/// Pass the filesystem path to `scene.xrds`; all asset URIs inside the document
/// are resolved by Bevy's `AssetServer` relative to the configured `asset_path`
/// (defaults to `assets/` next to the executable).
///
/// Supports both desktop monitor output and OpenXR HMD output — set
/// `RuntimeParameters::enable_xr = true` to enable OpenXR when a headset is
/// available.
///
/// # Example
/// ```rust,no_run,ignore
/// use xrds::{Runtime, RuntimeParameters};
/// use xrds::viewer::XrdsSceneViewer;
///
/// fn main() {
///     let exe_dir = std::env::current_exe().ok()
///         .and_then(|e| e.parent().map(|p| p.to_string_lossy().into_owned()));
///     let asset_path = exe_dir.clone().map(|d| format!("{d}/assets"));
///     let scene_path = exe_dir.map(|d| format!("{d}/assets/scene.xrds"))
///         .unwrap_or_else(|| "assets/scene.xrds".to_string());
///
///     Runtime::new(RuntimeParameters {
///         app_name: "My Scene".to_owned(),
///         enable_xr: false,
///         asset_path,
///         ..Default::default()
///     })
///     .run_xrds(XrdsSceneViewer::new(scene_path))
///     .expect("Could not run application");
/// }
/// ```
pub struct XrdsSceneViewer {
    scene_path: String,
}

impl XrdsSceneViewer {
    pub fn new(scene_path: impl Into<String>) -> Self {
        Self { scene_path: scene_path.into() }
    }
}

impl XrdsApp for XrdsSceneViewer {
    fn configure(&mut self, app: &mut App) {
        app.init_resource::<ViewerCameraState>();
        app.add_systems(Startup, spawn_viewer_camera);
        app.add_systems(Update, viewer_orbit_system);
    }

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        if let Err(e) = api.import_scene_document_json(&self.scene_path) {
            eprintln!("[XrdsSceneViewer] Failed to load '{}': {e:?}", self.scene_path);
        }
    }
}
