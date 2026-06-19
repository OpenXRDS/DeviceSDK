use bevy::prelude::*;
use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::window::PrimaryWindow;
use xrds_runtime::{XrdsReceivesEnvironment, XrdsIdIndex, sdk::XrdsId};
use crate::editor_state::{CameraMode, EditorSession, EditorState};
use crate::wry_overlay::{LEFT_W, RIGHT_W, TOP_H, BOT_H};

const ORBIT_SENSITIVITY: f32 = 0.007;
const PAN_SENSITIVITY:   f32 = 0.0025;
const ZOOM_LINE:         f32 = 0.12;
const ZOOM_PIXEL:        f32 = 0.004;
const FLY_SPEED:         f32 = 0.08;
const FLY_FAST:          f32 = 4.0;
const MIN_DIST:          f32 = 0.05;
const MAX_DIST:          f32 = 2000.0;

#[derive(Component)]
pub struct EditorCameraMarker;

#[derive(Resource)]
pub struct EditorCameraState {
    pub pivot:             Vec3,
    pub distance:          f32,
    pub yaw:               f32,
    pub pitch:             f32,
    pub view_snap:         Option<(f32, f32)>,
    pub fly_saved_distance: f32,
}

impl Default for EditorCameraState {
    fn default() -> Self {
        Self { pivot: Vec3::ZERO, distance: 8.0, yaw: -0.5, pitch: 0.45,
               view_snap: None, fly_saved_distance: 8.0 }
    }
}

impl EditorCameraState {
    pub fn to_transform(&self) -> Transform {
        let rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        let position = self.pivot + rotation * Vec3::new(0.0, 0.0, self.distance);
        Transform { translation: position, rotation, ..Default::default() }
    }
}

/// Spawns the editor camera rendering directly to the OS window viewport.
/// The viewport covers the centre area between the wry panel strips.
/// Physical bounds are recalculated on every `WindowResized` event by
/// `wry_overlay::handle_editor_resize`, so the initial values only need to be
/// roughly correct (they are updated on the first resize if the window opens
/// at a different size than the constants below assume).
pub fn spawn_editor_camera(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let (pw, ph, sf) = windows.single()
        .map(|w| (w.physical_width(), w.physical_height(), w.scale_factor()))
        .unwrap_or((1600, 900, 1.0));

    let left_phys  = (LEFT_W  as f32 * sf) as u32;
    let right_phys = (RIGHT_W as f32 * sf) as u32;
    let top_phys   = (TOP_H   as f32 * sf) as u32;
    let bot_phys   = (BOT_H   as f32 * sf) as u32;

    commands.spawn((
        Camera3d::default(),
        Camera {
            viewport: Some(bevy::camera::Viewport {
                physical_position: UVec2::new(left_phys, top_phys),
                physical_size: UVec2::new(
                    pw.saturating_sub(left_phys + right_phys),
                    ph.saturating_sub(top_phys + bot_phys),
                ),
                ..default()
            }),
            ..Default::default()
        },
        EditorCameraMarker,
        XrdsReceivesEnvironment,
    ));
}

/// Manages which camera is active based on `EditorState::active_camera_id`.
///
/// - `None`      → editor camera on, all scene cameras off.
/// - `Some(id)`  → editor camera off, matching scene camera on, others off.
/// - Player pawn camera is always excluded (managed separately by viewport_player).
pub fn apply_camera_selection_system(
    state: Res<EditorState>,
    id_index: Res<XrdsIdIndex>,
    mut all_cams_q: Query<(Entity, &mut Camera)>,
    editor_cam_q: Query<Entity, With<EditorCameraMarker>>,
    pawn_q: Query<Entity, With<crate::viewport_player::PlayerPawnMarker>>,
) {
    let editor_entity = editor_cam_q.single().ok();
    let pawn_entities: Vec<Entity> = pawn_q.iter().collect();

    let want_entity: Option<Entity> = state.active_camera_id
        .and_then(|id| id_index.entity_of(XrdsId(id.0)));

    for (entity, mut cam) in all_cams_q.iter_mut() {
        // Never touch the pawn camera — it's managed by viewport_player.
        if pawn_entities.contains(&entity) { continue; }

        let should_be_active = if state.is_playing {
            // During play mode all non-pawn cameras must be off.
            // Without this guard, this system re-enables the editor camera
            // every frame, causing two cameras to render the same window.
            false
        } else {
            match want_entity {
                None => Some(entity) == editor_entity,
                Some(scene_entity) => entity == scene_entity,
            }
        };
        if cam.is_active != should_be_active {
            cam.is_active = should_be_active;
        }
    }
}

pub fn orbit_camera_system(
    mut cam:          ResMut<EditorCameraState>,
    mouse_buttons:    Res<ButtonInput<MouseButton>>,
    keyboard:         Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel:  MessageReader<MouseWheel>,
    mut camera_q:     Query<&mut Transform, With<EditorCameraMarker>>,
    mut editor_state: ResMut<EditorState>,
    session:          Res<EditorSession>,
) {
    if editor_state.is_playing {
        for _ in mouse_motion.read() {}
        for _ in mouse_wheel.read() {}
        return;
    }

    let shift  = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let middle = mouse_buttons.pressed(MouseButton::Middle);
    let right  = mouse_buttons.pressed(MouseButton::Right);
    let is_fly = editor_state.camera_mode == CameraMode::Fly;

    let mut delta = Vec2::ZERO;
    for ev in mouse_motion.read() { delta += Vec2::new(ev.delta.x, ev.delta.y); }

    if let Some((snap_yaw, snap_pitch)) = cam.view_snap.take() {
        cam.yaw   = snap_yaw;
        cam.pitch = snap_pitch;
    }

    if is_fly {
        if right && delta != Vec2::ZERO {
            cam.yaw -= delta.x * ORBIT_SENSITIVITY;
            cam.pitch = (cam.pitch + delta.y * ORBIT_SENSITIVITY)
                .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        }
        if middle && shift && delta != Vec2::ZERO {
            let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
            let f = cam.distance * PAN_SENSITIVITY;
            cam.pivot -= rot * Vec3::X * delta.x * f;
            cam.pivot += rot * Vec3::Y * delta.y * f;
        }
    } else {
        // Orbit: middle OR right button drag.  Shift + either button = pan.
        let drag = middle || right;
        if drag && !shift && delta != Vec2::ZERO {
            cam.yaw -= delta.x * ORBIT_SENSITIVITY;
            cam.pitch = (cam.pitch + delta.y * ORBIT_SENSITIVITY)
                .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        }
        if drag && shift && delta != Vec2::ZERO {
            let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
            let f = cam.distance * PAN_SENSITIVITY;
            cam.pivot -= rot * Vec3::X * delta.x * f;
            cam.pivot += rot * Vec3::Y * delta.y * f;
        }
    }

    for ev in mouse_wheel.read() {
        let scroll = match ev.unit {
            MouseScrollUnit::Line  => ev.y * ZOOM_LINE,
            MouseScrollUnit::Pixel => ev.y * ZOOM_PIXEL,
        };
        cam.distance = (cam.distance * (1.0 - scroll)).clamp(MIN_DIST, MAX_DIST);
    }

    let ctrl  = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let base  = if is_fly { FLY_SPEED * 2.0 } else { FLY_SPEED };
    let speed = if shift { base * FLY_FAST } else { base };
    let rot   = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
    let fwd   = rot * -Vec3::Z;
    let rgt   = rot * Vec3::X;

    if !ctrl {
        if keyboard.pressed(KeyCode::KeyW) { cam.pivot += fwd * speed; }
        if keyboard.pressed(KeyCode::KeyS) { cam.pivot -= fwd * speed; }
        if keyboard.pressed(KeyCode::KeyA) { cam.pivot -= rgt * speed; }
        if keyboard.pressed(KeyCode::KeyD) { cam.pivot += rgt * speed; }
        if keyboard.pressed(KeyCode::KeyE) { cam.pivot += Vec3::Y * speed; }
        if keyboard.pressed(KeyCode::KeyQ) { cam.pivot -= Vec3::Y * speed; }
    }

    // Preview from PlayerAnchor — teleport editor camera to the anchor's authored world position.
    // Must compose the full parent chain because node.transform is parent-local.
    if let Some(anchor_id) = editor_state.preview_anchor_target.take() {
        let doc = session.0.document();
        let (world_pos, world_rot) = world_transform_of(doc, anchor_id);
        let (yaw, pitch, _) = world_rot.to_euler(EulerRot::YXZ);
        cam.yaw      = yaw;
        cam.pitch    = -pitch;
        cam.pivot    = world_pos;
        cam.distance = 0.01;
    }

    // F key or frame_selected_target from toolbar
    if let Some(_trigger) = editor_state.frame_selected_target.take() {
        if let Some(sel_id) = editor_state.selection.primary() {
            if let Some(node) = session.0.document().node(sel_id) {
                let [tx, ty, tz] = node.transform.translation;
                cam.pivot = Vec3::new(tx, ty, tz);
            }
        }
    } else if keyboard.just_pressed(KeyCode::KeyF) {
        if let Some(sel_id) = editor_state.selection.primary() {
            if let Some(node) = session.0.document().node(sel_id) {
                let [tx, ty, tz] = node.transform.translation;
                cam.pivot = Vec3::new(tx, ty, tz);
            }
        }
    }

    let t = cam.to_transform();
    for mut transform in camera_q.iter_mut() { *transform = t; }
}

/// Compute a node's world-space position and rotation by composing the full
/// parent chain.  Node transforms in the document are parent-local, so a
/// PlayerAnchor child of Player at (2,0,0) has local translation (0,0,0) if
/// authored at the Player's origin — reading it directly gives the wrong result.
fn world_transform_of(
    doc: &xrds_scene_graph::XrdsSceneDocument,
    node_id: xrds_scene_graph::XrdsSceneNodeId,
) -> (Vec3, Quat) {
    // Collect ancestors from target up to root, then reverse to compose root→leaf.
    let mut chain = Vec::new();
    let mut cur_id = Some(node_id);
    while let Some(id) = cur_id {
        let Some(node) = doc.node(id) else { break; };
        chain.push(node);
        cur_id = node.parent_id;
    }

    let mut world_pos = Vec3::ZERO;
    let mut world_rot = Quat::IDENTITY;
    for node in chain.iter().rev() {
        let t = &node.transform;
        let local_pos = Vec3::from(t.translation);
        let local_rot = Quat::from_xyzw(
            t.rotation_quat_xyzw[0], t.rotation_quat_xyzw[1],
            t.rotation_quat_xyzw[2], t.rotation_quat_xyzw[3],
        );
        world_pos = world_pos + world_rot * local_pos;
        world_rot = world_rot * local_rot;
    }
    (world_pos, world_rot)
}
