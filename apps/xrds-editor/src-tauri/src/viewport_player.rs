use bevy::prelude::*;
use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseMotion;
use xrds_scene_graph::{XrdsSceneNodeId, XrdsSceneNodePayload};
use xrds_scene_graph::XrdsPlayerLocomotionMode;
use xrds_runtime::{ActivePlayerAnchorEntity, PlayerAnchorCameraPose, XrdsIdIndex, XrdsPlayerAnchorRoot, XrdsPlayerRoot};

use crate::editor_state::{EditorSession, EditorState};
use crate::viewport_camera::EditorCameraMarker;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PAWN_MOVE_SPEED:    f32 = 5.0;
const PAWN_LOOK_SENS:     f32 = 0.003;
const GRAVITY:            f32 = -9.8;
const JUMP_IMPULSE:       f32 = 4.5;
const EYE_HEIGHT:         f32 = 1.6;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marks the player pawn spawned during play mode.
/// Excluded from `deactivate_scene_cameras` so the pawn camera stays active.
#[derive(Component)]
pub struct PlayerPawnMarker;

#[derive(Component)]
pub struct PawnLocomotionMode(pub XrdsPlayerLocomotionMode);

#[derive(Component)]
pub struct PlayHudMarker;

#[derive(Component)]
pub struct PawnVerticalState {
    pub velocity:    f32,
    pub is_grounded: bool,
    pub ground_y:    f32,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Spawn the player pawn on the first frame after play mode starts.
pub fn spawn_player_pawn_system(
    mut commands:     Commands,
    mut state:        ResMut<EditorState>,
    session:          Res<EditorSession>,
    cam_state:        Res<crate::viewport_camera::EditorCameraState>,
    mut editor_cams:  Query<&mut Camera, With<EditorCameraMarker>>,
) {
    if !state.is_playing || state.pawn_entity.is_some() { return; }

    // Priority 1: Player node — world-space body position + authored loco/fov settings.
    // Priority 2: PlayerSpawn node — simple spawn marker.
    // Priority 3: Current editor camera pose.
    let (spawn_pos, spawn_rot, fov_deg, loco_mode) = {
        let from_player = session.0.document().nodes.iter().find_map(|n| {
            if let XrdsSceneNodePayload::Player(p) = &n.payload {
                let t = &n.transform;
                Some((
                    Vec3::from(t.translation),
                    Quat::from_array(t.rotation_quat_xyzw),
                    60.0_f32, // FOV is per-anchor; use default until anchor switch applies it
                    p.locomotion_mode,
                ))
            } else { None }
        });
        if let Some(data) = from_player {
            data
        } else {
            session.0.document().nodes.iter().find_map(|n| {
                if let XrdsSceneNodePayload::PlayerSpawn(s) = &n.payload {
                    let t = &n.transform;
                    Some((
                        Vec3::from(t.translation),
                        Quat::from_array(t.rotation_quat_xyzw),
                        s.fov_deg,
                        s.locomotion_mode,
                    ))
                } else { None }
            })
            .unwrap_or_else(|| {
                let t = cam_state.to_transform();
                (t.translation, t.rotation, 90.0, XrdsPlayerLocomotionMode::Flying)
            })
        }
    };

    let eye_pos = Vec3::new(spawn_pos.x, spawn_pos.y + EYE_HEIGHT, spawn_pos.z);

    let pawn = commands.spawn((
        PlayerPawnMarker,
        PawnLocomotionMode(loco_mode),
        PawnVerticalState { velocity: 0.0, is_grounded: true, ground_y: eye_pos.y },
        Camera3d::default(),
        Camera { is_active: true, ..Default::default() },
        Projection::Perspective(PerspectiveProjection {
            fov: fov_deg.to_radians(),
            ..Default::default()
        }),
        Transform { translation: eye_pos, rotation: spawn_rot, ..Default::default() },
        GlobalTransform::default(),
    )).id();

    state.pawn_entity = Some(pawn);

    // Show a minimal play-mode overlay via Bevy UI.
    commands.spawn((
        PlayHudMarker,
        UiTargetCamera(pawn),
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..Default::default() },
    )).with_children(|p| {
        p.spawn((
            PlayHudMarker,
            Text::new("▶ PLAYING   Esc to stop"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0), left: Val::Px(8.0),
                ..Default::default()
            },
            TextFont { font_size: 14.0, ..Default::default() },
            TextColor(Color::srgba(1.0, 0.86, 0.31, 0.9)),
        ));
    });

    // Deactivate the editor camera while the pawn camera takes over.
    for mut cam in editor_cams.iter_mut() { cam.is_active = false; }
}

/// Despawn the player pawn when play mode stops.
pub fn despawn_player_pawn_system(
    mut commands:    Commands,
    mut state:       ResMut<EditorState>,
    mut editor_cams: Query<&mut Camera, With<EditorCameraMarker>>,
    hud_q:           Query<Entity, With<PlayHudMarker>>,
) {
    if state.is_playing || state.pawn_entity.is_none() { return; }

    if let Some(entity) = state.pawn_entity.take() {
        commands.entity(entity).despawn();
    }
    for entity in hud_q.iter() { commands.entity(entity).despawn(); }
    // Restore the editor camera.
    for mut cam in editor_cams.iter_mut() { cam.is_active = true; }
}

/// Player pawn locomotion — Flying (full 3-D) and grounded (Smooth/Teleport).
pub fn pawn_locomotion_system(
    mut pawn_q:       Query<(&mut Transform, &mut PawnVerticalState, &PawnLocomotionMode), With<PlayerPawnMarker>>,
    keyboard:         Res<ButtonInput<KeyCode>>,
    mouse_buttons:    Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    time:             Res<Time>,
    state:            Res<EditorState>,
) {
    let mut delta = Vec2::ZERO;
    for ev in mouse_motion.read() { delta += Vec2::new(ev.delta.x, ev.delta.y); }

    if !state.is_playing { return; }

    let Some((mut tf, mut vs, loco)) = pawn_q.iter_mut().next() else { return; };

    // RMB free-look.
    if mouse_buttons.pressed(MouseButton::Right) && delta != Vec2::ZERO {
        let (mut yaw, mut pitch, _) = tf.rotation.to_euler(EulerRot::YXZ);
        yaw   -= delta.x * PAWN_LOOK_SENS;
        pitch  = (pitch - delta.y * PAWN_LOOK_SENS)
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let dt    = time.delta_secs();

    match loco.0 {
        XrdsPlayerLocomotionMode::Flying => {
            let speed   = if shift { PAWN_MOVE_SPEED * 3.0 } else { PAWN_MOVE_SPEED };
            let forward = tf.rotation * -Vec3::Z;
            let right   = tf.rotation *  Vec3::X;
            if keyboard.pressed(KeyCode::KeyW) { tf.translation += forward * speed * dt; }
            if keyboard.pressed(KeyCode::KeyS) { tf.translation -= forward * speed * dt; }
            if keyboard.pressed(KeyCode::KeyA) { tf.translation -= right   * speed * dt; }
            if keyboard.pressed(KeyCode::KeyD) { tf.translation += right   * speed * dt; }
            if keyboard.pressed(KeyCode::KeyE) { tf.translation += Vec3::Y * speed * dt; }
            if keyboard.pressed(KeyCode::KeyQ) { tf.translation -= Vec3::Y * speed * dt; }
        }
        XrdsPlayerLocomotionMode::Smooth | XrdsPlayerLocomotionMode::Teleport => {
            if !vs.is_grounded { vs.velocity += GRAVITY * dt; }
            tf.translation.y += vs.velocity * dt;
            if tf.translation.y <= vs.ground_y {
                tf.translation.y = vs.ground_y;
                vs.velocity = 0.0; vs.is_grounded = true;
            }
            if vs.is_grounded && keyboard.just_pressed(KeyCode::Space) {
                vs.velocity = JUMP_IMPULSE; vs.is_grounded = false;
            }
            let speed   = if shift { PAWN_MOVE_SPEED * 2.0 } else { PAWN_MOVE_SPEED };
            let fwd3    = tf.rotation * -Vec3::Z;
            let rgt3    = tf.rotation *  Vec3::X;
            let forward = Vec3::new(fwd3.x, 0.0, fwd3.z).normalize_or_zero();
            let right   = Vec3::new(rgt3.x, 0.0, rgt3.z).normalize_or_zero();
            if keyboard.pressed(KeyCode::KeyW) { tf.translation += forward * speed * dt; }
            if keyboard.pressed(KeyCode::KeyS) { tf.translation -= forward * speed * dt; }
            if keyboard.pressed(KeyCode::KeyA) { tf.translation -= right   * speed * dt; }
            if keyboard.pressed(KeyCode::KeyD) { tf.translation += right   * speed * dt; }
        }
    }
}

/// Drive the active Player entity's Transform from the pawn camera each frame.
/// When `ActivePlayerAnchorEntity` is set, only the parent Player of that anchor
/// is synced so other Player entities stay at their authored positions.
/// When no anchor is active, all Player entities follow the pawn (original behaviour).
/// Runs in PostUpdate BEFORE TransformPropagate so Bevy propagates the updated
/// Player position down to all children in the same frame.
pub fn sync_player_root_system(
    pawn_q: Query<&Transform, With<PlayerPawnMarker>>,
    mut player_q: Query<(Entity, &mut Transform), (With<XrdsPlayerRoot>, Without<PlayerPawnMarker>)>,
    state: Res<EditorState>,
    active: Res<ActivePlayerAnchorEntity>,
    anchor_parent_q: Query<Option<&ChildOf>, With<XrdsPlayerAnchorRoot>>,
) {
    if !state.is_playing { return; }
    let Some(pawn_tf) = pawn_q.iter().next() else { return; };

    // Body orientation = yaw only; strip pitch/roll so the Player entity
    // stands upright while the camera can still look up/down.
    let yaw = pawn_tf.rotation.to_euler(EulerRot::YXZ).0;
    let body_rot = Quat::from_rotation_y(yaw);

    // Resolve which Player entity owns the active anchor (if any).
    let target_player: Option<Entity> = active.0.and_then(|anchor_ent| {
        anchor_parent_q.get(anchor_ent).ok()
            .flatten()
            .map(|co| co.0)
    });

    for (player_entity, mut player_tf) in player_q.iter_mut() {
        // Skip Player entities that don't own the active anchor.
        if let Some(target) = target_player {
            if player_entity != target { continue; }
        }
        player_tf.translation = pawn_tf.translation;
        player_tf.rotation    = body_rot;
    }
}

/// Initialise `PlayerAnchorCameraPose` on every `XrdsPlayerAnchorRoot` entity
/// when play mode starts.  Uses the anchor's authored world-space GlobalTransform
/// as the initial pose so the first switch lands at the authored position.
/// Runs once per play session; the `initialized` Local resets on play-stop.
pub fn init_anchor_poses_system(
    mut commands: Commands,
    state: Res<EditorState>,
    session: Res<EditorSession>,
    id_index: Res<XrdsIdIndex>,
    anchor_q: Query<(Entity, &GlobalTransform), (With<XrdsPlayerAnchorRoot>, Without<PlayerAnchorCameraPose>)>,
    mut initialized: Local<bool>,
) {
    if !state.is_playing {
        *initialized = false;
        return;
    }
    if *initialized || state.pawn_entity.is_none() { return; }
    *initialized = true;
    let doc = session.0.document();
    for (entity, gt) in anchor_q.iter() {
        let tf = gt.compute_transform();
        let fov_deg = id_index.id_of(entity)
            .and_then(|xid| doc.node(XrdsSceneNodeId(xid.0)))
            .and_then(|n| if let XrdsSceneNodePayload::PlayerAnchor(a) = &n.payload { Some(a.fov_deg) } else { None })
            .unwrap_or(60.0);
        commands.entity(entity).insert(PlayerAnchorCameraPose {
            translation: tf.translation,
            rotation: tf.rotation,
            fov_deg,
        });
    }
}

/// Teleport the pawn when the active anchor changes during play mode.
///
/// On departure: saves the pawn's current pose into the departing anchor's
/// `PlayerAnchorCameraPose` so switching back restores where the player left off.
/// On arrival: restores the arriving anchor's stored pose.
pub fn switch_player_anchor_system(
    mut pawn_q: Query<(&mut Transform, &mut Projection), With<PlayerPawnMarker>>,
    mut anchor_pose_q: Query<&mut PlayerAnchorCameraPose, With<XrdsPlayerAnchorRoot>>,
    active: Res<ActivePlayerAnchorEntity>,
    state: Res<EditorState>,
    mut last_anchor: Local<Option<Entity>>,
) {
    if !state.is_playing {
        *last_anchor = None;
        return;
    }
    let current = active.0;
    if *last_anchor == current { return; }

    let Some((mut pawn_tf, mut proj)) = pawn_q.iter_mut().next() else { return; };

    // Save current pawn pose to the departing anchor.
    if let Some(departing) = *last_anchor {
        if let Ok(mut pose) = anchor_pose_q.get_mut(departing) {
            pose.translation = pawn_tf.translation;
            pose.rotation    = pawn_tf.rotation;
        }
    }

    // Restore pose and FOV from the arriving anchor.
    if let Some(arriving) = current {
        if let Ok(pose) = anchor_pose_q.get(arriving) {
            pawn_tf.translation = pose.translation;
            pawn_tf.rotation    = pose.rotation;
            if let Projection::Perspective(ref mut persp) = *proj {
                persp.fov = pose.fov_deg.to_radians();
            }
        }
    }

    *last_anchor = current;
}

/// Handle keyboard shortcuts for anchor switching during play mode.
///
/// - **Tab**: cycle to the next `PlayerAnchor` in document order.
/// - **1–9**: jump directly to anchor N (1-indexed).
///
/// Updates `EditorState::active_player_anchor_id`; `sync_active_anchor_system`
/// then translates that to `ActivePlayerAnchorEntity`, and
/// `switch_player_anchor_system` responds to the resulting entity change.
pub fn player_anchor_key_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
    session: Res<EditorSession>,
) {
    if !state.is_playing { return; }

    let anchors: Vec<xrds_scene_graph::XrdsSceneNodeId> = session.0.document().nodes.iter()
        .filter(|n| matches!(n.payload, XrdsSceneNodePayload::PlayerAnchor(_)))
        .map(|n| n.id)
        .collect();
    if anchors.is_empty() { return; }

    if keyboard.just_pressed(KeyCode::Tab) {
        let next = match state.active_player_anchor_id {
            None => anchors.first().copied(),
            Some(id) => {
                let pos = anchors.iter().position(|&a| a == id).unwrap_or(0);
                anchors.get((pos + 1) % anchors.len()).copied()
            }
        };
        state.active_player_anchor_id = next;
    }

    let digit_keys = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
        KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
        KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    ];
    for (i, &key) in digit_keys.iter().enumerate() {
        if keyboard.just_pressed(key) {
            state.active_player_anchor_id = anchors.get(i).copied();
        }
    }
}
