use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use xrds_scene_graph::{XrdsPlayerLocomotionMode, XrdsSceneDocument, XrdsSceneNodePayload};
use xrds_runtime::{
    ActivePlayerAnchorEntity, Runtime, RuntimeParameters, XrdsAPI, XrdsApp,
    XrdsInitialAnchor, XrdsPlayerAnchorRoot, XrdsPlayerCamera, XrdsUpdateContext,
};
use xrds_openxr::{OpenXrCameraIndex, OpenXrPlayerRoot, XrControllerModelAssets, XrInput};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const MOVE_SPEED:    f32 = 5.0;
const LOOK_SENS:     f32 = 0.003;
const GRAVITY:       f32 = -9.8;
const JUMP_IMPULSE:  f32 = 4.5;
const EYE_HEIGHT:    f32 = 1.6;

// ---------------------------------------------------------------------------
// Components / resources
// ---------------------------------------------------------------------------

#[derive(Component)]
struct AppCamera;

#[derive(Component)]
enum ControllerHand { Left, Right }

#[derive(Component)]
struct LocomotionMode(XrdsPlayerLocomotionMode);

#[derive(Component)]
struct VerticalState {
    velocity: f32,
    ground_y: f32,
    grounded: bool,
}

/// Spawn configuration extracted from the scene document.
///
/// Priority: initial PlayerAnchor → Player node → PlayerSpawn → default.
/// Inserted as a resource during `configure` so `spawn_app_camera` can read it.
#[derive(Resource)]
struct SpawnConfig {
    position:   Vec3,
    rotation:   Quat,
    fov_deg:    f32,
    locomotion: XrdsPlayerLocomotionMode,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            position:   Vec3::new(0.0, EYE_HEIGHT, 8.0),
            rotation:   Transform::IDENTITY.looking_at(Vec3::ZERO, Vec3::Y).rotation,
            fov_deg:    60.0,
            locomotion: XrdsPlayerLocomotionMode::Flying,
        }
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

struct SceneFileApp {
    scene_path: std::path::PathBuf,
}

impl SceneFileApp {
    /// Read the scene document to determine spawn position, FOV, and locomotion mode.
    ///
    /// Priority:
    /// 1. PlayerAnchor with `is_initial = true` — spawn at that anchor's world position.
    /// 2. Player node — spawn at the player body origin (+ EYE_HEIGHT).
    /// 3. PlayerSpawn node — legacy spawn marker.
    /// 4. Default.
    fn read_spawn_config(&self) -> SpawnConfig {
        let doc = match XrdsSceneDocument::load_json(&self.scene_path) {
            Ok(d) => d,
            Err(_) => return SpawnConfig::default(),
        };

        // Helper: build a Bevy Transform from a document node's authored transform.
        let node_tf = |n: &xrds_scene_graph::XrdsSceneNode| -> Transform {
            Transform {
                translation: Vec3::from_array(n.transform.translation),
                rotation:    Quat::from_array(n.transform.rotation_quat_xyzw),
                scale:       Vec3::from_array(n.transform.scale),
            }
        };

        // Priority 1: PlayerAnchor with is_initial — use its authored world position.
        let initial_anchor = doc.nodes.iter().find(|n| {
            matches!(&n.payload, XrdsSceneNodePayload::PlayerAnchor(a) if a.is_initial)
        });
        if let Some(anchor_node) = initial_anchor {
            if let XrdsSceneNodePayload::PlayerAnchor(a) = &anchor_node.payload {
                // Compute world-space position: parent_transform * local_transform.
                let parent_tf = anchor_node.parent_id
                    .and_then(|pid| doc.nodes.iter().find(|n| n.id == pid))
                    .map(|pn| node_tf(pn))
                    .unwrap_or(Transform::IDENTITY);
                let local_tf = node_tf(anchor_node);
                let world_pos = parent_tf.transform_point(local_tf.translation);
                let world_rot = parent_tf.rotation * local_tf.rotation;
                return SpawnConfig {
                    position:   world_pos,
                    rotation:   world_rot,
                    fov_deg:    a.fov_deg,
                    locomotion: a.locomotion_mode,
                };
            }
        }

        // Priority 2: Player node. FOV is per-anchor; default 60° until the initial anchor applies.
        if let Some(n) = doc.nodes.iter().find(|n| matches!(n.payload, XrdsSceneNodePayload::Player(_))) {
            if let XrdsSceneNodePayload::Player(p) = &n.payload {
                let feet = Vec3::from_array(n.transform.translation);
                return SpawnConfig {
                    position:   Vec3::new(feet.x, feet.y + EYE_HEIGHT, feet.z),
                    rotation:   Quat::from_array(n.transform.rotation_quat_xyzw),
                    fov_deg:    60.0,
                    locomotion: p.locomotion_mode,
                };
            }
        }

        // Priority 3: PlayerSpawn node.
        if let Some(n) = doc.nodes.iter().find(|n| matches!(n.payload, XrdsSceneNodePayload::PlayerSpawn(_))) {
            if let XrdsSceneNodePayload::PlayerSpawn(s) = &n.payload {
                let feet = Vec3::from_array(n.transform.translation);
                return SpawnConfig {
                    position:   Vec3::new(feet.x, feet.y + EYE_HEIGHT, feet.z),
                    rotation:   Quat::from_array(n.transform.rotation_quat_xyzw),
                    fov_deg:    s.fov_deg,
                    locomotion: s.locomotion_mode,
                };
            }
        }

        SpawnConfig::default()
    }
}

impl XrdsApp for SceneFileApp {
    fn configure(&mut self, app: &mut App) {
        let config = self.read_spawn_config();
        eprintln!(
            "[xrds-app] spawn at {:?}, fov {}°, mode {:?}",
            config.position, config.fov_deg, config.locomotion
        );
        app.insert_resource(config);
        app.add_systems(PostStartup, (spawn_app_camera, spawn_controller_visuals));
        // set_initial_anchor_system runs after spawn_app_camera to ensure the camera entity
        // exists when we first set ActivePlayerAnchorEntity.
        app.add_systems(PostStartup, set_initial_anchor_system.after(spawn_app_camera));
        app.add_systems(Update, (deactivate_scene_cameras, manage_window_camera, fly_camera_system, grounded_camera_system, xr_locomotion_system, update_controller_visuals, attach_controller_models, player_anchor_key_system));
    }

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        match api.import_scene_document_json(&self.scene_path) {
            Ok(ids) => eprintln!(
                "[xrds-app] loaded '{}' — {} entities",
                self.scene_path.display(),
                ids.len()
            ),
            Err(e) => eprintln!(
                "[xrds-app] ERROR loading '{}': {e:?}",
                self.scene_path.display()
            ),
        }
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
}

// ---------------------------------------------------------------------------
// Camera spawn
// ---------------------------------------------------------------------------

fn spawn_app_camera(mut commands: Commands, config: Res<SpawnConfig>) {
    // Player root AND window fallback camera.
    // Renders to the window whenever XR eye cameras are inactive (no HMD, HMD off, covered).
    // When XR eye cameras are active the blit overwrites the window, so this camera is disabled.
    commands.spawn((
        AppCamera,
        OpenXrPlayerRoot,
        XrdsPlayerCamera,
        LocomotionMode(config.locomotion),
        VerticalState {
            velocity: 0.0,
            ground_y: config.position.y,
            grounded: true,
        },
        Transform {
            translation: config.position,
            rotation:    config.rotation,
            ..Default::default()
        },
        GlobalTransform::default(),
        Camera3d::default(),
        Camera::default(),
        Projection::Perspective(PerspectiveProjection {
            fov:  config.fov_deg.to_radians(),
            near: 0.1,
            ..Default::default()
        }),
    ));

    eprintln!(
        "[xrds-app] camera spawned at {:?} (fov {}°)",
        config.position,
        config.fov_deg
    );
}

// ---------------------------------------------------------------------------
// Deactivate scene camera nodes — authored cameras in the scene document.
// AppCamera is excluded (it's the window fallback camera).
// XR eye cameras use TextureView targets so the window filter skips them anyway.
// ---------------------------------------------------------------------------

fn deactivate_scene_cameras(mut cameras: Query<&mut Camera, Without<AppCamera>>) {
    use bevy::camera::RenderTarget;
    for mut cam in cameras.iter_mut() {
        if cam.is_active && matches!(cam.target, RenderTarget::Window(_)) {
            cam.is_active = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Toggle the AppCamera window render based on XR eye camera activity.
// Active XR cameras → disable AppCamera (blit handles the window).
// No active XR cameras (no HMD, covered, session paused) → enable AppCamera.
// ---------------------------------------------------------------------------

fn manage_window_camera(
    mut app_cam_q: Query<&mut Camera, With<AppCamera>>,
    xr_cam_q:      Query<&Camera, (With<OpenXrCameraIndex>, Without<AppCamera>)>,
) {
    let any_xr_active = xr_cam_q.iter().any(|cam| cam.is_active);
    for mut cam in app_cam_q.iter_mut() {
        cam.is_active = !any_xr_active;
    }
}

// ---------------------------------------------------------------------------
// Flying locomotion — WASD + RMB look + scroll speed
// ---------------------------------------------------------------------------

fn fly_camera_system(
    mut cam_q:    Query<(&mut Transform, &LocomotionMode), With<AppCamera>>,
    keyboard:     Res<ButtonInput<KeyCode>>,
    mouse_buttons:Res<ButtonInput<MouseButton>>,
    mut motion:   MessageReader<MouseMotion>,
    mut scroll:   MessageReader<MouseWheel>,
) {
    let Ok((mut tf, mode)) = cam_q.single_mut() else { return; };
    if !matches!(mode.0, XrdsPlayerLocomotionMode::Flying) { return; }

    let mut delta = Vec2::ZERO;
    for ev in motion.read() { delta += Vec2::new(ev.delta.x, ev.delta.y); }

    if mouse_buttons.pressed(MouseButton::Right) && delta != Vec2::ZERO {
        let (mut yaw, mut pitch, _) = tf.rotation.to_euler(EulerRot::YXZ);
        yaw   -= delta.x * LOOK_SENS;
        pitch  = (pitch - delta.y * LOOK_SENS)
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    }

    let mut speed_mult = 1.0f32;
    for ev in scroll.read() {
        speed_mult *= match ev.unit {
            MouseScrollUnit::Line  => 1.0 + ev.y * 0.1,
            MouseScrollUnit::Pixel => 1.0 + ev.y * 0.005,
        };
    }
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let speed = MOVE_SPEED * speed_mult * if shift { 3.0 } else { 1.0 } * 0.016;

    let fwd = tf.rotation * -Vec3::Z;
    let rgt = tf.rotation *  Vec3::X;
    if keyboard.pressed(KeyCode::KeyW) { tf.translation += fwd * speed; }
    if keyboard.pressed(KeyCode::KeyS) { tf.translation -= fwd * speed; }
    if keyboard.pressed(KeyCode::KeyA) { tf.translation -= rgt * speed; }
    if keyboard.pressed(KeyCode::KeyD) { tf.translation += rgt * speed; }
    if keyboard.pressed(KeyCode::KeyE) { tf.translation += Vec3::Y * speed; }
    if keyboard.pressed(KeyCode::KeyQ) { tf.translation -= Vec3::Y * speed; }
}

// ---------------------------------------------------------------------------
// Grounded locomotion — WASD on XZ + gravity + jump
// ---------------------------------------------------------------------------

fn grounded_camera_system(
    mut cam_q:    Query<(&mut Transform, &mut VerticalState, &LocomotionMode), With<AppCamera>>,
    keyboard:     Res<ButtonInput<KeyCode>>,
    mouse_buttons:Res<ButtonInput<MouseButton>>,
    mut motion:   MessageReader<MouseMotion>,
    time:         Res<Time>,
) {
    let Ok((mut tf, mut vs, mode)) = cam_q.single_mut() else { return; };
    if matches!(mode.0, XrdsPlayerLocomotionMode::Flying) { return; }

    let mut delta = Vec2::ZERO;
    for ev in motion.read() { delta += Vec2::new(ev.delta.x, ev.delta.y); }

    if mouse_buttons.pressed(MouseButton::Right) && delta != Vec2::ZERO {
        let (mut yaw, mut pitch, _) = tf.rotation.to_euler(EulerRot::YXZ);
        yaw   -= delta.x * LOOK_SENS;
        pitch  = (pitch - delta.y * LOOK_SENS)
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    }

    let dt = time.delta_secs();
    if !vs.grounded { vs.velocity += GRAVITY * dt; }
    tf.translation.y += vs.velocity * dt;
    if tf.translation.y <= vs.ground_y {
        tf.translation.y = vs.ground_y; vs.velocity = 0.0; vs.grounded = true;
    }
    if vs.grounded && keyboard.just_pressed(KeyCode::Space) {
        vs.velocity = JUMP_IMPULSE; vs.grounded = false;
    }

    let speed = MOVE_SPEED * dt;
    let fwd3  = tf.rotation * -Vec3::Z;
    let rgt3  = tf.rotation *  Vec3::X;
    let fwd   = Vec3::new(fwd3.x, 0.0, fwd3.z).normalize_or_zero();
    let rgt   = Vec3::new(rgt3.x, 0.0, rgt3.z).normalize_or_zero();
    if keyboard.pressed(KeyCode::KeyW) { tf.translation += fwd * speed; }
    if keyboard.pressed(KeyCode::KeyS) { tf.translation -= fwd * speed; }
    if keyboard.pressed(KeyCode::KeyA) { tf.translation -= rgt * speed; }
    if keyboard.pressed(KeyCode::KeyD) { tf.translation += rgt * speed; }
}

// ---------------------------------------------------------------------------
// Controller visuals — thin ray pointer at aim pose for each hand.
// Hidden when the hand is not tracked (pose is None).
// ---------------------------------------------------------------------------

fn spawn_controller_visuals(mut commands: Commands) {
    // Spawn root transform entities for each hand. No geometry yet — either the
    // runtime-provided GLTF model attaches as a child (attach_controller_models),
    // or a fallback ray is spawned if the XR_FB_render_model extension is absent.
    for hand in [ControllerHand::Left, ControllerHand::Right] {
        commands.spawn((
            hand,
            Transform::IDENTITY,
            Visibility::Hidden,
        ));
    }
}

/// Tracks whether we have already attached real controller models.
#[derive(Component)]
struct ControllerModelAttached;

/// Once XrControllerModelAssets is ready, attach real models or fallback rays as children.
fn attach_controller_models(
    mut commands:  Commands,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models:        Option<Res<XrControllerModelAssets>>,
    hand_q:        Query<(Entity, &ControllerHand), Without<ControllerModelAttached>>,
) {
    let Some(models) = models else { return };
    if !models.is_ready { return; }

    for (entity, hand) in hand_q.iter() {
        let scene_handle = match hand {
            ControllerHand::Left  => models.left.clone(),
            ControllerHand::Right => models.right.clone(),
        };

        if let Some(handle) = scene_handle {
            commands.entity(entity)
                .insert(ControllerModelAttached)
                .with_children(|p| { p.spawn(SceneRoot(handle)); });
        } else {
            attach_fallback_ray(&mut commands, &mut meshes, &mut materials, entity, hand);
            commands.entity(entity).insert(ControllerModelAttached);
        }
    }
}

fn attach_fallback_ray(
    commands:  &mut Commands,
    meshes:    &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    entity:    Entity,
    hand:      &ControllerHand,
) {
    let color = match hand {
        ControllerHand::Left  => Color::srgb(0.3, 0.6, 1.0),
        ControllerHand::Right => Color::srgb(1.0, 0.45, 0.3),
    };
    let mesh = meshes.add(Cuboid::new(0.01, 0.01, 0.5));
    let mat  = materials.add(StandardMaterial { base_color: color, unlit: true, ..Default::default() });
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(Vec3::new(0.0, 0.0, -0.25)),
        ));
    });
}

fn update_controller_visuals(
    mut vis_q:  Query<(&mut Transform, &mut Visibility, &ControllerHand)>,
    root_q:     Query<&Transform, (With<OpenXrPlayerRoot>, Without<ControllerHand>)>,
    xr_input:   Option<Res<XrInput>>,
) {
    let Some(xr) = xr_input else { return };
    let root = root_q.single().ok();

    for (mut tf, mut vis, hand) in vis_q.iter_mut() {
        let state = match hand {
            ControllerHand::Left  => &xr.left,
            ControllerHand::Right => &xr.right,
        };
        match state.pose {
            Some(stage_pose) => {
                *tf  = stage_to_world(root, &stage_pose);
                *vis = Visibility::Visible;
            }
            None => { *vis = Visibility::Hidden; }
        }
    }
}

/// Maps a stage-space pose to world space using the player root's locomotion transform.
/// Mirrors `apply_root_to_pose` in render.rs: only XZ translation + yaw are applied,
/// preserving the physical Y and full rotation from the real-world pose.
fn stage_to_world(root: Option<&Transform>, stage: &Transform) -> Transform {
    match root {
        Some(r) => {
            let yaw     = r.rotation.to_euler(EulerRot::YXZ).0;
            let yaw_rot = Quat::from_rotation_y(yaw);
            let origin  = Vec3::new(r.translation.x, 0.0, r.translation.z);
            Transform::from_translation(origin + yaw_rot * stage.translation)
                .with_rotation(yaw_rot * stage.rotation)
        }
        None => *stage,
    }
}

// ---------------------------------------------------------------------------
// XR controller locomotion — left stick moves, right stick turns.
// Reads Option<Res<XrInput>> so it compiles even when the XR plugin is absent.
// ---------------------------------------------------------------------------

fn xr_locomotion_system(
    mut cam_q:  Query<(&mut Transform, &LocomotionMode), With<AppCamera>>,
    xr_input:   Option<Res<XrInput>>,
    time:       Res<Time>,
) {
    let Some(xr_input) = xr_input else { return };
    let Ok((mut tf, _)) = cam_q.single_mut() else { return };

    let stick_l = xr_input.left.thumbstick;
    let stick_r = xr_input.right.thumbstick;

    // Left stick: strafe/forward locomotion (XZ plane only)
    if stick_l != Vec2::ZERO {
        let speed = MOVE_SPEED * time.delta_secs();
        let fwd = {
            let f = tf.rotation * -Vec3::Z;
            Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
        };
        let rgt = {
            let r = tf.rotation * Vec3::X;
            Vec3::new(r.x, 0.0, r.z).normalize_or_zero()
        };
        tf.translation += fwd * stick_l.y * speed + rgt * stick_l.x * speed;
    }

    // Right stick X: smooth yaw turn
    if stick_r.x.abs() > 0.1 {
        let turn_speed = 1.8_f32; // rad/s
        let (mut yaw, pitch, _) = tf.rotation.to_euler(EulerRot::YXZ);
        yaw -= stick_r.x * turn_speed * time.delta_secs();
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    }
}

// ---------------------------------------------------------------------------
// Anchor switching
// ---------------------------------------------------------------------------

/// PostStartup: set `ActivePlayerAnchorEntity` so exactly one anchor is active
/// on startup.  Runs after `spawn_app_camera` so the camera entity exists before
/// `teleport_on_anchor_switch_system` fires.
///
/// Priority:
/// 1. The anchor with `is_initial = true` (XrdsInitialAnchor marker).
/// 2. The first `XrdsPlayerAnchorRoot` entity in query order (fallback).
///
/// Without an active anchor, `is_active_anchor` returns `true` for ALL anchors,
/// causing every body/head-locked text to follow the camera regardless of which
/// player it belongs to.
fn set_initial_anchor_system(
    initial_q: Query<Entity, With<XrdsInitialAnchor>>,
    all_anchors_q: Query<Entity, With<XrdsPlayerAnchorRoot>>,
    mut active: ResMut<ActivePlayerAnchorEntity>,
) {
    let all_count = all_anchors_q.iter().count();
    let initial_count = initial_q.iter().count();
    eprintln!(
        "[xrds-app] set_initial_anchor: {all_count} XrdsPlayerAnchorRoot entity(s), {initial_count} XrdsInitialAnchor entity(s)"
    );
    if let Some(entity) = initial_q.iter().next() {
        eprintln!("[xrds-app] set_initial_anchor: activating initial anchor {entity:?}");
        active.0 = Some(entity);
    } else if let Some(entity) = all_anchors_q.iter().next() {
        eprintln!("[xrds-app] set_initial_anchor: no initial marker, falling back to first anchor {entity:?}");
        active.0 = Some(entity);
    } else {
        eprintln!("[xrds-app] set_initial_anchor: WARNING — no PlayerAnchor entities found, ActivePlayerAnchorEntity stays None");
    }
}

/// Cycle through PlayerAnchor entities with Tab (next) or digits 1-9 (direct).
///
/// Collects all `XrdsPlayerAnchorRoot` entities and updates `ActivePlayerAnchorEntity`
/// so `sync_player_root_system` and `apply_anchor_fov_system` react in the same frame.
///
/// NOTE: This keyboard-based switching is a temporary placeholder.
/// It will be replaced by a trigger-action system (e.g. InteractionZone entry,
/// XrdsAPI call, or scripted event) once that mechanism is designed.
fn player_anchor_key_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    anchor_q: Query<Entity, With<XrdsPlayerAnchorRoot>>,
    mut active: ResMut<ActivePlayerAnchorEntity>,
) {
    let anchors: Vec<Entity> = anchor_q.iter().collect();

    if keyboard.just_pressed(KeyCode::Tab) {
        eprintln!(
            "[xrds-app] Tab pressed: {} anchor(s) available, current = {:?}",
            anchors.len(), active.0
        );
        if anchors.is_empty() { return; }
        let next = match active.0 {
            None => anchors.first().copied(),
            Some(cur) => {
                let pos = anchors.iter().position(|&e| e == cur).unwrap_or(0);
                anchors.get((pos + 1) % anchors.len()).copied()
            }
        };
        eprintln!("[xrds-app] Tab: switching to anchor {next:?}");
        active.0 = next;
        return;
    }

    if anchors.is_empty() { return; }

    let digit_keys = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
        KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
        KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    ];
    for (i, &key) in digit_keys.iter().enumerate() {
        if keyboard.just_pressed(key) {
            active.0 = anchors.get(i).copied();
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let exe_dir = std::env::current_exe()
        .expect("cannot determine executable path")
        .parent()
        .expect("executable has no parent directory")
        .to_path_buf();

    let scene_path = exe_dir.join("scene.json");
    let asset_path = exe_dir.join("assets").to_string_lossy().into_owned();

    if !scene_path.exists() {
        eprintln!("[xrds-app] ERROR: scene not found at '{}'", scene_path.display());
        std::process::exit(1);
    }

    Runtime::new(RuntimeParameters {
        app_name: "XRDS App".to_owned(),
        enable_xr: true,
        asset_path: Some(asset_path),
        allow_unapproved_paths: true,
        ..Default::default()
    })
    .run_xrds(SceneFileApp { scene_path })
    .expect("runtime error");
}
