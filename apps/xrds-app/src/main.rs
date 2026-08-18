use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::{
    Render, RenderApp, RenderSet,
    render_resource::{CachedPipelineState, PipelineCache},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use xrds_scene_graph::{XrdsPlayerLocomotionMode, XrdsSceneDocument, XrdsSceneNodeId, XrdsSceneNodePayload};
use xrds_runtime::{XrdsGltfAnimationSelector, XrdsGltfAnimationPlaybackOptions};
use xrds_runtime::sdk::world::XrdsGltfAsset;
use xrds_runtime::sdk::XrdsId;
use xrds_runtime::{
    ActivePlayerAnchorEntity, Runtime, RuntimeParameters, TextParams, XrDropEvent, XrGrabEvent,
    XrdsAPI, XrdsApp, XrdsInitialAnchor, XrdsPlayerAnchorRoot, XrdsPlayerCamera,
    XrdsText, XrdsUpdateContext,
};
use xrds_openxr::{OpenXrCameraIndex, OpenXrPlayerRoot, XrControllerModelAssets, XrHand, XrHapticRequest, XrInput};

// ---------------------------------------------------------------------------
// Shader compilation progress — shared between main world and render world
// ---------------------------------------------------------------------------

/// Pipeline compile counts written by the render world, read by the main world.
#[derive(Resource, Clone, Default)]
struct PipelineProgress {
    done:  Arc<AtomicU32>,
    total: Arc<AtomicU32>,
}

struct ShaderProgressPlugin(PipelineProgress);

impl Plugin for ShaderProgressPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(self.0.clone());
            render_app.add_systems(Render, update_pipeline_progress.in_set(RenderSet::Cleanup));
        }
    }
}

fn update_pipeline_progress(
    pipeline_cache: Option<Res<PipelineCache>>,
    progress: Res<PipelineProgress>,
) {
    let Some(pipeline_cache) = pipeline_cache else { return };
    let mut done  = 0u32;
    let mut total = 0u32;
    for p in pipeline_cache.pipelines() {
        total += 1;
        if matches!(p.state, CachedPipelineState::Ok(_) | CachedPipelineState::Err(_)) {
            done += 1;
        }
    }
    progress.done.store(done, Ordering::Relaxed);
    progress.total.store(total, Ordering::Relaxed);
}

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

/// Toggled by `animation_key_system`; consumed in `SceneFileApp::update()`.
#[derive(Resource, Default)]
struct AnimationState {
    /// True when P was pressed this frame; cleared after update() processes it.
    toggled: bool,
    /// Current playback state — toggled each time P is pressed.
    playing: bool,
}

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
    scene_path:    std::path::PathBuf,
    /// GltfAsset node IDs collected at setup time for P-key animation playback.
    gltf_node_ids: Vec<XrdsSceneNodeId>,
    /// Head-locked status label spawned in setup(); updated each time animation state changes.
    hud_handle:    Option<xrds_runtime::Handle<XrdsText>>,
    /// Head-locked shader compilation progress label; cleared when shaders are ready.
    loading_hud:   Option<xrds_runtime::Handle<XrdsText>>,
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
        app.insert_resource(AnimationState::default());
        app.add_plugins(ShaderProgressPlugin(PipelineProgress::default()));
        app.add_systems(PostStartup, (spawn_app_camera, spawn_controller_visuals));
        // After spawn_app_camera so the camera entity exists before
        // ActivePlayerAnchorEntity is first set.
        app.add_systems(PostStartup, set_initial_anchor_system.after(spawn_app_camera));
        app.add_systems(Update, (deactivate_scene_cameras, manage_window_camera, fly_camera_system, grounded_camera_system, xr_locomotion_system, update_controller_visuals, attach_controller_models, player_anchor_key_system, animation_key_system, haptic_test_key_system, grab_event_log_system));
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
        if let Some(spawn_pos) = api.random_spawn_zone_position() {
            eprintln!("[xrds-app] PlayerSpawnZone: teleporting player to {:?}", spawn_pos);
            api.teleport_player(spawn_pos);
        }
        // Cache GltfAsset node IDs so update() can drive animation playback without
        // re-reading the document every frame.
        if let Ok(doc) = XrdsSceneDocument::load_json(&self.scene_path) {
            self.gltf_node_ids = doc.nodes.iter()
                .filter_map(|n| matches!(n.payload, XrdsSceneNodePayload::GltfAsset(_))
                    .then_some(n.id))
                .collect();
            eprintln!("[xrds-app] {} GltfAsset node(s) found for animation", self.gltf_node_ids.len());
        }

        // Shader progress HUD — spawned AFTER scene import so it doesn't claim XrdsId(1).
        self.loading_hud = Some(api.spawn_hud_label(
            "Compiling shaders... 0%",
            Vec3::new(0.0, 0.0, -1.5),
        ));

        // HUD label — 50 cm in front, 15 cm below centre line.
        self.hud_handle = Some(api.spawn_hud_label(
            "P: play/stop  H: haptic L  J: haptic R",
            Vec3::new(0.0, -0.15, -0.5),
        ));

    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        // Update shader compilation progress HUD.
        if let Some(ref loading_hud) = self.loading_hud {
            let (done, total) = ctx.resource::<PipelineProgress>()
                .map(|p| (p.done.load(Ordering::Relaxed), p.total.load(Ordering::Relaxed)))
                .unwrap_or((0, 0));

            if total > 0 && done >= total {
                // All pipelines compiled — clear the label.
                ctx.set_text_params(loading_hud, TextParams {
                    text:      String::new(),
                    font_size: 1.0,
                    color:     [0.0, 0.0, 0.0, 0.0],
                    alignment: xrds_runtime::XrdsTextAlignment::Center,
                });
                self.loading_hud = None;
            } else {
                let pct  = if total > 0 { done * 100 / total } else { 0 };
                let text = if total == 0 {
                    "Initializing...".to_string()
                } else {
                    format!("Compiling shaders... {pct}%  ({done}/{total})")
                };
                ctx.set_text_params(loading_hud, TextParams {
                    text,
                    font_size: 5.0,
                    color:     [1.0, 1.0, 0.6, 1.0],
                    alignment: xrds_runtime::XrdsTextAlignment::Center,
                });
            }
        }

        let toggled = ctx.resource::<AnimationState>().map(|s| s.toggled).unwrap_or(false);
        if !toggled { return; }

        // Flip state and clear the toggle flag.
        let playing = ctx.resource::<AnimationState>().map(|s| !s.playing).unwrap_or(true);
        if let Some(mut state) = ctx.resource_mut::<AnimationState>() {
            state.toggled = false;
            state.playing = playing;
        }

        for &node_id in &self.gltf_node_ids {
            let Some(handle) = ctx.handle_of::<XrdsGltfAsset>(XrdsId::from(node_id)) else { continue };
            if playing {
                let _ = ctx.play_gltf_animation(
                    &handle,
                    XrdsGltfAnimationSelector::Index(0),
                    XrdsGltfAnimationPlaybackOptions::default(),
                );
            } else {
                let _ = ctx.stop_gltf_animation(&handle);
            }
        }

        // Update HUD to reflect current animation state.
        if let Some(ref hud) = self.hud_handle {
            let label = if playing { "▶  P: stop  H: haptic L  J: haptic R" }
                        else       { "⏹  P: play  H: haptic L  J: haptic R" };
            ctx.set_text_params(hud, TextParams {
                text:      label.to_string(),
                font_size: 4.0,
                color:     [1.0, 1.0, 1.0, 1.0],
                alignment: xrds_runtime::XrdsTextAlignment::Center,
            });
        }
    }
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
// Toggle the AppCamera window render based on whether XR eye cameras exist.
// XR cameras present → disable AppCamera unconditionally (the XR compositor
// owns the display; is_active on individual XR cameras can blip false during
// frame timing without meaning the session ended, which previously caused
// AppCamera to re-activate and introduce a spurious 3rd visible-entities pass).
// No XR cameras at all (desktop-only run, no HMD) → enable AppCamera.
// ---------------------------------------------------------------------------

fn manage_window_camera(
    mut app_cam_q: Query<&mut Camera, With<AppCamera>>,
    xr_cam_q:      Query<(), With<OpenXrCameraIndex>>,
) {
    let xr_present = !xr_cam_q.is_empty();
    for mut cam in app_cam_q.iter_mut() {
        cam.is_active = !xr_present;
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
// Animation key — P toggles play/stop on all GLB objects
// ---------------------------------------------------------------------------

fn animation_key_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<AnimationState>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        state.toggled = true;
    }
}

// ---------------------------------------------------------------------------
// Haptic test — H = left controller pulse, J = right controller pulse
// ---------------------------------------------------------------------------

fn haptic_test_key_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut haptic: MessageWriter<XrHapticRequest>,
) {
    if keyboard.just_pressed(KeyCode::KeyH) {
        haptic.write(XrHapticRequest { hand: XrHand::Left,  amplitude: 1.0, duration_secs: 0.2, frequency: 0.0 });
    }
    if keyboard.just_pressed(KeyCode::KeyJ) {
        haptic.write(XrHapticRequest { hand: XrHand::Right, amplitude: 1.0, duration_secs: 0.2, frequency: 0.0 });
    }
}

// ---------------------------------------------------------------------------
// Grab event logging
// ---------------------------------------------------------------------------

/// Read XrGrabEvent / XrDropEvent messages and log them.
///
/// Replace this with gameplay logic (highlight, UI update, physics hand-off, etc.)
/// once the basic grab loop is confirmed working.
fn grab_event_log_system(
    mut grab_events: MessageReader<XrGrabEvent>,
    mut drop_events: MessageReader<XrDropEvent>,
) {
    for ev in grab_events.read() {
        info!("[grab] GRABBED id={:?} hand={:?}", ev.id, ev.hand);
    }
    for ev in drop_events.read() {
        info!("[grab] DROPPED  id={:?} hand={:?}", ev.id, ev.hand);
    }
}

// ---------------------------------------------------------------------------
// Entry point — desktop
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
    .run_xrds(SceneFileApp { scene_path, gltf_node_ids: Vec::new(), hud_handle: None, loading_hud: None })
    .expect("runtime error");
}


// ---------------------------------------------------------------------------
// Entry point — Android (GameActivity via cargo-ndk)
// ---------------------------------------------------------------------------

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    use std::io::Read;

    // Enable full Rust backtraces so panic messages contain a stack trace.
    // Without this, the panic machinery only prints "note: run with RUST_BACKTRACE=1"
    // and the actual panic site is invisible in logcat.
    unsafe { std::env::set_var("RUST_BACKTRACE", "full") };

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("xrds"),
    );

    // Initialize the OpenXR loader BEFORE any OpenXR call (including Entry::load()).
    // On Android the loader requires the JavaVM and Activity context to find the runtime.
    // We grab the raw pointers here while android_app is still owned.
    {
        let vm      = android_app.vm_as_ptr();
        let context = android_app.activity_as_ptr();
        unsafe { xrds_openxr::initialize_openxr_loader_android(vm, context); }
    }

    // Two modes are supported, selected automatically:
    //
    // Dev mode (external storage):
    //   Push scene.json and assets/ to the device before launching:
    //     adb push scene.json /sdcard/Android/data/org.openxrds.devicesdk/files/
    //     adb push assets/    /sdcard/Android/data/org.openxrds.devicesdk/files/assets/
    //   asset_path points at the external directory; Bevy uses normal filesystem I/O.
    //
    // APK-bundled mode:
    //   Build with android/quest/build.sh --scene-dir <dir> to bundle scene.json and
    //   assets/ into the APK.  On first launch (or when files change), assets are
    //   extracted to internal cache storage so all libraries get real filesystem paths.

    const PACKAGE: &str = "org.openxrds.devicesdk";
    let external_dir = std::path::PathBuf::from(
        format!("/sdcard/Android/data/{PACKAGE}/files"),
    );
    let external_scene = external_dir.join("scene.json");

    let (scene_path, opt_asset_path, font_paths) = if external_scene.exists() {
        // Dev mode — scene and assets are on external storage; no extraction needed.
        log::info!("[xrds-app] dev mode: loading scene from external storage");
        let assets_dir = external_dir.join("assets");

        // Set CWD to the external assets dir so validate_gltf_source (which resolves
        // relative paths against CWD) can find pushed GLB/GLTF files by bare filename,
        // mirroring what APK mode does with cache_dir below.
        if let Err(e) = std::env::set_current_dir(&assets_dir) {
            log::warn!("[xrds-app] could not set CWD to '{}': {e}", assets_dir.display());
        }

        // Fonts: prefer pushed fonts (auto-discovered from asset_path/fonts/); if the
        // push omitted them, fall back to the APK-bundled fonts extracted to cache.
        // Without any font, cosmic-text panics at the first text render and the app
        // appears to hang with no frames submitted.
        let font_paths = if assets_dir.join("fonts").join("NotoSans-Regular.ttf").exists() {
            None
        } else {
            log::info!(
                "[xrds-app] no fonts under '{}/fonts' — extracting APK-bundled fonts",
                assets_dir.display()
            );
            extract_apk_fonts(&android_app, PACKAGE)
        };

        let ap = assets_dir.to_string_lossy().into_owned();
        (external_scene, Some(ap), font_paths)
    } else {
        // APK-bundled mode — extract all bundled assets from the APK to the internal
        // cache directory, then point Bevy's AssetServer at that directory.
        //
        // Android's AAssetManager API is opaque (no filesystem paths), but Bevy's GLTF
        // loader, cosmic_text/fontdb, and other libraries require real fs::Path entries.
        // Extracting once to cache (with size-check to skip unchanged files on subsequent
        // launches) is the conventional Android solution for this constraint.
        log::info!("[xrds-app] APK mode: extracting bundled assets to internal cache");

        let cache_dir = std::path::PathBuf::from(format!("/data/data/{PACKAGE}/cache"));
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            log::error!("[xrds-app] failed to create cache dir '{}': {e}", cache_dir.display());
            return;
        }

        // Read ASSET_MANIFEST — generated by build.sh, lists every file in the APK's
        // assets/ root (one relative path per line).
        let manifest_cstr = std::ffi::CString::new("ASSET_MANIFEST").unwrap();
        let manifest_content = match android_app.asset_manager().open(&manifest_cstr) {
            Some(mut f) => {
                let mut s = String::new();
                if let Err(e) = std::io::Read::read_to_string(&mut f, &mut s) {
                    log::error!("[xrds-app] failed to read ASSET_MANIFEST: {e}");
                    return;
                }
                s
            }
            None => {
                log::error!(
                    "[xrds-app] ASSET_MANIFEST not found in APK. \
                     Rebuild with the updated build.sh that generates the manifest."
                );
                return;
            }
        };

        // Decide whether the cache is stale, using ASSET_STAMP — a per-build marker
        // generated by build.sh/build.ps1.
        //
        // This replaces an earlier per-file `cached_size == bytes.len()` check that was
        // wrong in a way no test could catch: edit an asset without changing its byte
        // length and the device silently keeps the old one. Not merely a dev-loop
        // annoyance — Android does not clear the cache dir on APK upgrade, so a shipped
        // update whose asset changed at constant size would never reach existing users.
        //
        // The stamp changes on every build, so it cannot false-match. A no-op rebuild
        // therefore re-extracts, which is the deliberate trade: always applying new work
        // matters more than saving one extraction.
        //
        // It is also strictly *less* work than the old check, which read and decompressed
        // every asset out of the APK before deciding to skip the write — paying the
        // expensive half unconditionally. A stamp match now skips the loop entirely.
        let stamp_cstr = std::ffi::CString::new("ASSET_STAMP").unwrap();
        let apk_stamp: Option<String> = android_app
            .asset_manager()
            .open(&stamp_cstr)
            .and_then(|mut f| {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut f, &mut s)
                    .ok()
                    .map(|_| s.trim().to_owned())
            });
        let stamp_path = cache_dir.join("ASSET_STAMP");
        let cached_stamp = std::fs::read_to_string(&stamp_path)
            .ok()
            .map(|s| s.trim().to_owned());

        // Both sides must be present and equal. A missing APK stamp means an APK from
        // before this mechanism existed: extract every launch rather than trust a cache
        // we cannot date.
        let up_to_date = match (&apk_stamp, &cached_stamp) {
            (Some(apk), Some(cached)) => apk == cached,
            _ => false,
        };

        if apk_stamp.is_none() {
            log::warn!(
                "[xrds-app] ASSET_STAMP not in APK — rebuild to enable cache skipping. \
                 Extracting unconditionally."
            );
        }

        if up_to_date {
            log::info!(
                "[xrds-app] assets already extracted for this build (stamp {}) — skipping",
                cached_stamp.as_deref().unwrap_or("?")
            );
        } else {
            // Remove the stale stamp first. If extraction is interrupted — a crash, the
            // user backing out, the system killing us — no stamp is left behind, so the
            // next launch re-extracts instead of trusting a half-populated cache.
            let _ = std::fs::remove_file(&stamp_path);

            let mut n_extracted = 0u32;
            let mut n_failed = 0u32;
            for rel_path in manifest_content.lines() {
                let rel_path = rel_path.trim();
                if rel_path.is_empty() { continue; }
                // The stamp is written last, from the value already in hand.
                if rel_path == "ASSET_STAMP" { continue; }

                let dest = cache_dir.join(rel_path);
                if let Some(parent) = dest.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        log::warn!("[xrds-app] cannot create parent dir for '{rel_path}': {e}");
                        n_failed += 1;
                        continue;
                    }
                }

                let Ok(asset_cstr) = std::ffi::CString::new(rel_path) else { continue };
                let Some(mut f) = android_app.asset_manager().open(&asset_cstr) else {
                    log::warn!("[xrds-app] asset listed in manifest but missing from APK: {rel_path}");
                    n_failed += 1;
                    continue;
                };

                // Stream rather than read_to_end into a Vec: the largest bundled assets
                // run to tens of MB, and buffering one whole file in memory is a needless
                // peak on a memory-constrained headset.
                let mut out = match std::fs::File::create(&dest) {
                    Ok(out) => out,
                    Err(e) => {
                        log::warn!("[xrds-app] cannot create '{}': {e}", dest.display());
                        n_failed += 1;
                        continue;
                    }
                };
                if let Err(e) = std::io::copy(&mut f, &mut out) {
                    log::warn!("[xrds-app] failed to extract '{rel_path}': {e}");
                    n_failed += 1;
                    continue;
                }
                n_extracted += 1;
            }

            log::info!(
                "[xrds-app] asset extraction complete: {n_extracted} written, {n_failed} failed"
            );

            // Only stamp a fully successful pass, and only after every write. A partial
            // extraction must not be recorded as current.
            match (&apk_stamp, n_failed) {
                (Some(stamp), 0) => {
                    if let Err(e) = std::fs::write(&stamp_path, stamp) {
                        log::warn!("[xrds-app] could not write ASSET_STAMP: {e}");
                    }
                }
                (Some(_), failed) => log::warn!(
                    "[xrds-app] {failed} asset(s) failed — not stamping; \
                     the next launch will re-extract"
                ),
                (None, _) => {}
            }
        }

        let cached_scene = cache_dir.join("scene.json");
        if !cached_scene.exists() {
            log::error!(
                "[xrds-app] scene.json not in cache after extraction. \
                 Ensure build.sh --scene-dir was used or scene.json exists in APK assets."
            );
            return;
        }

        // Set CWD to cache_dir so that validate_gltf_source (which resolves relative
        // paths against CWD) can find bundled GLB/GLTF files by their bare filename.
        if let Err(e) = std::env::set_current_dir(&cache_dir) {
            log::warn!("[xrds-app] could not set CWD to cache dir: {e}");
        } else {
            log::info!("[xrds-app] CWD set to {}", cache_dir.display());
        }

        let cache_dir_str = cache_dir.to_string_lossy().into_owned();
        (cached_scene, Some(cache_dir_str), None)
    };

    // Give bevy_winit the AndroidApp handle before App::run() is called.
    bevy_android::ANDROID_APP
        .set(android_app)
        .expect("AndroidApp already initialized");

    Runtime::new(RuntimeParameters {
        app_name: "XRDS App".to_owned(),
        enable_xr: true,
        asset_path: opt_asset_path,
        allow_unapproved_paths: true,
        font_paths,
        ..Default::default()
    })
    .run_xrds(SceneFileApp { scene_path, gltf_node_ids: Vec::new(), hud_handle: None, loading_hud: None })
    .expect("runtime error");
}

/// Extract the APK-bundled NotoSans fonts to internal cache and return their paths.
///
/// Used by dev mode as a fallback when the pushed external assets have no fonts/
/// directory. Returns `None` if nothing could be extracted (the runtime then falls
/// back to system font scanning).
#[cfg(target_os = "android")]
fn extract_apk_fonts(
    android_app: &winit::platform::android::activity::AndroidApp,
    package: &str,
) -> Option<Vec<String>> {
    use std::io::Read;

    let fonts_cache = std::path::PathBuf::from(format!("/data/data/{package}/cache/fonts"));
    if let Err(e) = std::fs::create_dir_all(&fonts_cache) {
        log::warn!("[xrds-app] cannot create '{}': {e}", fonts_cache.display());
        return None;
    }

    let font_names = [
        "NotoSans-Regular.ttf",
        "NotoSans-Bold.ttf",
        "NotoSans-Italic.ttf",
        "NotoSans-BoldItalic.ttf",
    ];
    let mut extracted = Vec::new();
    for name in font_names {
        let dest = fonts_cache.join(name);
        if !dest.exists() {
            let rel = format!("fonts/{name}");
            let Ok(cstr) = std::ffi::CString::new(rel.as_str()) else { continue };
            let Some(mut f) = android_app.asset_manager().open(&cstr) else {
                log::warn!("[xrds-app] font not bundled in APK: {rel}");
                continue;
            };
            let mut bytes = Vec::new();
            if let Err(e) = f.read_to_end(&mut bytes) {
                log::warn!("[xrds-app] failed to read APK font '{rel}': {e}");
                continue;
            }
            if let Err(e) = std::fs::write(&dest, &bytes) {
                log::warn!("[xrds-app] failed to write '{}': {e}", dest.display());
                continue;
            }
        }
        extracted.push(dest.to_string_lossy().into_owned());
    }

    if extracted.is_empty() {
        log::warn!("[xrds-app] no APK fonts extracted — text rendering may fail");
        None
    } else {
        log::info!("[xrds-app] using {} APK-bundled font(s) from cache", extracted.len());
        Some(extracted)
    }
}
