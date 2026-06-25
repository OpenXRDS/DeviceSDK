use bevy::camera::visibility::VisibleEntities;
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// PlayerAnchor marker
// ---------------------------------------------------------------------------

/// Inserted by the scene importer on entities spawned from a `Player` document
/// node.  Marks the world-space pawn root; its transform is driven by the
/// locomotion system in Phase 3.
#[derive(Component)]
pub struct XrdsPlayerRoot;

/// Inserted by the scene importer on entities that were spawned from a
/// `PlayerAnchor` document node.  Anchor systems use this to recognise that
/// a child text entity's authored `local_offset` is meaningful anchor-local
/// data rather than a world-space scene position.
#[derive(Component)]
pub struct XrdsPlayerAnchorRoot;

/// Controls which `PlayerAnchor` entity is currently active.
///
/// - `None` → all `XrdsPlayerAnchorRoot` entities are treated as active
///   (Phase 1/2 behaviour, unchanged).
/// - `Some(entity)` → only children of that specific entity receive
///   camera-relative anchor math.  Children of other anchors are left at
///   the position `TransformPropagate` computed (i.e. they follow the Player
///   body without head-locking).
#[derive(Resource, Default, PartialEq)]
pub struct ActivePlayerAnchorEntity(pub Option<Entity>);

/// Stores the authored FOV (degrees) for a `PlayerAnchor` entity.
///
/// Inserted by the scene importer so anchor-switching systems can apply per-anchor
/// FOV without re-reading the document.
#[derive(Component, Clone, Copy)]
pub struct XrdsAnchorFov(pub f32);

/// Per-anchor exposure override (ev100).  `None` = use the scene-wide exposure.
///
/// Inserted by the scene importer on `PlayerAnchor` entities that carry an authored
/// exposure value.  `apply_anchor_exposure_system` reads this and updates
/// `XrdsAnchorExposureOverride` when the active anchor changes.
#[derive(Component, Clone, Copy)]
pub struct XrdsAnchorExposure(pub Option<f32>);

/// Marks the entity that represents the player's head camera in a deployed runtime.
///
/// Insert this on the camera entity that moves with the player (e.g. `AppCamera`
/// in `xrds-app`).  Two runtime systems query it:
/// - `sync_player_root_system` — drives `XrdsPlayerRoot` body entities from this
///   camera so authored content stays correctly positioned relative to the player.
/// - `apply_anchor_fov_system` — updates this camera's `Projection` when
///   `ActivePlayerAnchorEntity` changes.
#[derive(Component)]
pub struct XrdsPlayerCamera;

/// Marks the `PlayerAnchor` entity whose `is_initial` flag was set in the document.
///
/// At most one anchor per scene should carry this component.  Runtime startup
/// systems use it to teleport the player camera to the initial spawn anchor.
#[derive(Component)]
pub struct XrdsInitialAnchor;

/// Stores the camera pose (position + rotation) for a `PlayerAnchor` entity.
///
/// Inserted at play-start on every `XrdsPlayerAnchorRoot` entity, initialised
/// from the anchor's authored world-space position.  When the player switches
/// to a different anchor the departing anchor's last-visited pose is saved here
/// so switching back resumes from where the player left off.
#[derive(Component, Clone, Copy)]
pub struct PlayerAnchorCameraPose {
    pub translation: Vec3,
    pub rotation: Quat,
    pub fov_deg: f32,
}

// ---------------------------------------------------------------------------
// Marker components — one per XR anchor mode
// ---------------------------------------------------------------------------

/// Inserted on text entities whose `XrdsTextAnchor` is `HeadLocked`.
#[derive(Component)]
pub struct XrdsHeadLocked {
    pub local_offset: Transform,
}

/// Inserted on text entities whose `XrdsTextAnchor` is `BodyLocked`.
#[derive(Component)]
pub struct XrdsBodyLocked {
    pub local_offset: Transform,
}

/// Inserted on text entities whose `XrdsTextAnchor` is `ComfortPinned`.
#[derive(Component)]
pub struct XrdsComfortPinned {
    pub depth_m: f32,
    pub local_offset: Transform,
}

/// Inserted on text entities whose `XrdsTextAnchor` is `Cylindrical`.
/// - `local_offset.translation.x` — azimuth in radians relative to the player's yaw
/// - `local_offset.translation.y` — vertical offset above eye level
#[derive(Component)]
pub struct XrdsCylindrical {
    pub radius_m: f32,
    pub local_offset: Transform,
}

// ---------------------------------------------------------------------------
// Per-frame systems
// ---------------------------------------------------------------------------

pub fn head_locked_system(
    mut q: Query<(
        &mut Transform,
        &mut GlobalTransform,
        &XrdsHeadLocked,
        Option<&ChildOf>,
    )>,
    camera_q: Query<
        (&GlobalTransform, &Projection, &Camera),
        (With<Camera3d>, Without<XrdsHeadLocked>),
    >,
    player_cam_q: Query<&GlobalTransform, (With<XrdsPlayerCamera>, Without<XrdsHeadLocked>)>,
    parent_q: Query<&GlobalTransform, Without<XrdsHeadLocked>>,
    anchor_root_q: Query<(), (With<XrdsPlayerAnchorRoot>, Without<XrdsHeadLocked>)>,
    player_root_q: Query<(), (With<XrdsPlayerRoot>, Without<XrdsHeadLocked>)>,
    active: Res<ActivePlayerAnchorEntity>,
) {
    let Some(cam_gt) = pick_head_camera(&camera_q, player_cam_q.iter().next()) else {
        return;
    };
    let cam_tf = cam_gt.compute_transform();
    let cam_pos = cam_tf.translation;

    for (mut tf, mut gt, anchor, child_of) in q.iter_mut() {
        let parent_ent = child_of.map(|co| co.0);
        let parent_anchor = parent_ent.filter(|&e| anchor_root_q.contains(e));

        if let Some(anchor_ent) = parent_anchor {
            // Skip if parent anchor is not the active one.
            if !is_active_anchor(anchor_ent, &anchor_root_q, &active) {
                continue;
            }
        } else if parent_ent.is_some_and(|e| player_root_q.contains(e)) {
            // Parent is a Player (world-space body) — not a camera anchor.
            // Leave at authored world position; do not camera-follow.
            continue;
        }
        // local_offset.translation is in camera-local space (X right, Y up, -Z forward).
        // Rotate it into world space each frame so the entity tracks the camera correctly.
        // Rotation matches the camera exactly — HUD items are screen-painted, not world billboards.
        let world_pos = cam_pos + cam_tf.rotation * anchor.local_offset.translation;
        let world_rot = cam_tf.rotation;
        let world_mat = Mat4::from_scale_rotation_translation(Vec3::ONE, world_rot, world_pos);
        write_world(&mut *tf, &mut *gt, world_mat, child_of, &parent_q);
    }
}

pub fn body_locked_system(
    mut q: Query<(
        &mut Transform,
        &mut GlobalTransform,
        &XrdsBodyLocked,
        Option<&ChildOf>,
    )>,
    camera_q: Query<
        (&GlobalTransform, &Projection, &Camera),
        (With<Camera3d>, Without<XrdsBodyLocked>),
    >,
    player_cam_q: Query<&GlobalTransform, (With<XrdsPlayerCamera>, Without<XrdsBodyLocked>)>,
    parent_q: Query<&GlobalTransform, Without<XrdsBodyLocked>>,
    anchor_root_q: Query<(), (With<XrdsPlayerAnchorRoot>, Without<XrdsBodyLocked>)>,
    player_root_q: Query<(), (With<XrdsPlayerRoot>, Without<XrdsBodyLocked>)>,
    active: Res<ActivePlayerAnchorEntity>,
) {
    let Some(cam_gt) = pick_head_camera(&camera_q, player_cam_q.iter().next()) else {
        return;
    };
    let cam_tf = cam_gt.compute_transform();
    let cam_pos = cam_tf.translation;
    let yaw = cam_tf.rotation.to_euler(EulerRot::YXZ).0;
    let body_rot = Quat::from_rotation_y(yaw);
    let body_mat = Mat4::from_scale_rotation_translation(Vec3::ONE, body_rot, cam_pos);

    for (mut tf, mut gt, anchor, child_of) in q.iter_mut() {
        let parent_ent = child_of.map(|co| co.0);
        let parent_anchor = parent_ent.filter(|&e| anchor_root_q.contains(e));

        if let Some(anchor_ent) = parent_anchor {
            if !is_active_anchor(anchor_ent, &anchor_root_q, &active) {
                // Inactive anchor: authored world position stays; just billboard
                // toward the active camera so the label is readable from outside.
                let world_pos = gt.translation();
                let world_rot = horiz_billboard(world_pos, cam_pos, body_rot);
                let world_mat =
                    Mat4::from_scale_rotation_translation(Vec3::ONE, world_rot, world_pos);
                write_world(&mut *tf, &mut *gt, world_mat, child_of, &parent_q);
                continue;
            }
        } else if parent_ent.is_some_and(|e| player_root_q.contains(e)) {
            continue;
        }
        let has_anchor_parent = parent_anchor.is_some();

        let world_pos = if has_anchor_parent {
            (body_mat * anchor.local_offset.translation.extend(1.0)).truncate()
        } else {
            (body_mat * Vec3::new(0.0, 0.7, -0.2).extend(1.0)).truncate()
        };
        let world_rot = horiz_billboard(world_pos, cam_pos, body_rot);
        let world_mat = Mat4::from_scale_rotation_translation(Vec3::ONE, world_rot, world_pos);
        write_world(&mut *tf, &mut *gt, world_mat, child_of, &parent_q);
    }
}

pub fn comfort_pinned_system(
    mut q: Query<(
        &mut Transform,
        &mut GlobalTransform,
        &XrdsComfortPinned,
        Option<&ChildOf>,
    )>,
    camera_q: Query<
        (&GlobalTransform, &Projection, &Camera),
        (With<Camera3d>, Without<XrdsComfortPinned>),
    >,
    player_cam_q: Query<&GlobalTransform, (With<XrdsPlayerCamera>, Without<XrdsComfortPinned>)>,
    parent_q: Query<&GlobalTransform, Without<XrdsComfortPinned>>,
    anchor_root_q: Query<(), (With<XrdsPlayerAnchorRoot>, Without<XrdsComfortPinned>)>,
    player_root_q: Query<(), (With<XrdsPlayerRoot>, Without<XrdsComfortPinned>)>,
    active: Res<ActivePlayerAnchorEntity>,
) {
    let Some(cam_gt) = pick_head_camera(&camera_q, player_cam_q.iter().next()) else {
        return;
    };
    let cam_tf = cam_gt.compute_transform();
    let cam_pos = cam_gt.translation();

    for (mut tf, mut gt, anchor, child_of) in q.iter_mut() {
        let parent_ent = child_of.map(|co| co.0);
        let parent_anchor = parent_ent.filter(|&e| anchor_root_q.contains(e));

        if let Some(anchor_ent) = parent_anchor {
            if !is_active_anchor(anchor_ent, &anchor_root_q, &active) {
                continue;
            }
        } else if parent_ent.is_some_and(|e| player_root_q.contains(e)) {
            continue;
        }
        let has_anchor_parent = parent_anchor.is_some();

        let depth = if anchor.depth_m < 0.05 {
            1.5
        } else {
            anchor.depth_m
        };
        let forward = cam_tf.rotation * Vec3::NEG_Z;
        let base_pos = cam_pos + forward * depth;
        let world_pos =
            if has_anchor_parent && anchor.local_offset.translation.length_squared() > 1e-6 {
                let lateral = Vec3::new(
                    anchor.local_offset.translation.x,
                    anchor.local_offset.translation.y,
                    0.0,
                );
                base_pos + cam_tf.rotation * lateral
            } else {
                base_pos
            };
        let world_rot = face_camera(cam_pos, world_pos) * anchor.local_offset.rotation;
        let world_mat =
            Mat4::from_scale_rotation_translation(anchor.local_offset.scale, world_rot, world_pos);
        write_world(&mut *tf, &mut *gt, world_mat, child_of, &parent_q);
    }
}

pub fn cylindrical_system(
    mut q: Query<(
        &mut Transform,
        &mut GlobalTransform,
        &XrdsCylindrical,
        Option<&ChildOf>,
    )>,
    camera_q: Query<
        (&GlobalTransform, &Projection, &Camera),
        (With<Camera3d>, Without<XrdsCylindrical>),
    >,
    player_cam_q: Query<&GlobalTransform, (With<XrdsPlayerCamera>, Without<XrdsCylindrical>)>,
    parent_q: Query<&GlobalTransform, Without<XrdsCylindrical>>,
    anchor_root_q: Query<(), (With<XrdsPlayerAnchorRoot>, Without<XrdsCylindrical>)>,
    player_root_q: Query<(), (With<XrdsPlayerRoot>, Without<XrdsCylindrical>)>,
    active: Res<ActivePlayerAnchorEntity>,
) {
    let Some(cam_gt) = pick_head_camera(&camera_q, player_cam_q.iter().next()) else {
        return;
    };
    let cam_tf = cam_gt.compute_transform();
    let yaw = cam_tf.rotation.to_euler(EulerRot::YXZ).0;
    let center = cam_tf.translation;

    for (mut tf, mut gt, anchor, child_of) in q.iter_mut() {
        let parent_ent = child_of.map(|co| co.0);
        let parent_anchor = parent_ent.filter(|&e| anchor_root_q.contains(e));

        if let Some(anchor_ent) = parent_anchor {
            if !is_active_anchor(anchor_ent, &anchor_root_q, &active) {
                continue;
            }
        } else if parent_ent.is_some_and(|e| player_root_q.contains(e)) {
            continue;
        }

        let angle = yaw + anchor.local_offset.translation.x;
        let height = center.y + anchor.local_offset.translation.y;
        let r = anchor.radius_m;
        let x = center.x + angle.sin() * r;
        let z = center.z - angle.cos() * r;
        let pos = Vec3::new(x, height, z);

        let outward = Vec3::new(x - center.x, 0.0, z - center.z).normalize_or_zero();
        let base_rot = if outward.length_squared() > 0.001 {
            Transform::IDENTITY.looking_to(outward, Vec3::Y).rotation
        } else {
            Quat::IDENTITY
        };
        let world_mat = Mat4::from_scale_rotation_translation(
            anchor.local_offset.scale,
            base_rot * anchor.local_offset.rotation,
            pos,
        );
        write_world(&mut *tf, &mut *gt, world_mat, child_of, &parent_q);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `entity` is a `XrdsPlayerAnchorRoot` that should currently
/// process anchor systems.  Respects `ActivePlayerAnchorEntity`:
/// - `None` → all anchor roots are active.
/// - `Some(e)` → only that specific entity is active.
#[inline]
fn is_active_anchor(
    entity: Entity,
    anchor_root_q: &Query<(), impl bevy::ecs::query::QueryFilter>,
    active: &ActivePlayerAnchorEntity,
) -> bool {
    if !anchor_root_q.contains(entity) {
        return false;
    }
    match active.0 {
        None => true,
        Some(active_ent) => entity == active_ent,
    }
}

/// Camera selection priority for anchor systems:
///
/// 1. Active XR eye camera (`Projection::Custom`) — HMD asymmetric optics.
/// 2. `player_cam_hint` — explicitly-marked player camera (`XrdsPlayerCamera`).
///    Avoids picking a static scene Camera node authored in the document, which
///    would place body-locked content at a fixed world position rather than
///    following the player.
/// 3. Any active `Camera3d` — editor play-mode pawn, single-camera desktop apps.
fn pick_head_camera<'a>(
    camera_q: &'a Query<
        (&GlobalTransform, &Projection, &Camera),
        impl bevy::ecs::query::QueryFilter,
    >,
    player_cam_hint: Option<&'a GlobalTransform>,
) -> Option<&'a GlobalTransform> {
    // XR eye cameras take absolute priority (custom asymmetric projection from HMD optics).
    if let Some((gt, _, _)) = camera_q
        .iter()
        .find(|(_, p, c)| c.is_active && matches!(p, Projection::Custom(_)))
    {
        return Some(gt);
    }
    // Deployed-runtime player camera — prevents scene Camera nodes (authored in the
    // document and imported as real Bevy Camera3d entities) from being picked instead
    // of the moving AppCamera.
    if let Some(gt) = player_cam_hint {
        return Some(gt);
    }
    // Fallback: any active camera (editor pawn, single-camera desktop apps without
    // an explicit XrdsPlayerCamera marker).
    camera_q
        .iter()
        .find(|(_, _, c)| c.is_active)
        .map(|(gt, _, _)| gt)
}

/// Horizontal billboard: local +Z faces toward `cam_pos` projected onto the XZ
/// plane.  Stays upright (Vec3::Y up) regardless of vertical offset.
/// Falls back to body-forward when text and camera share the same XZ position
/// (text directly above/below camera).
fn horiz_billboard(world_pos: Vec3, cam_pos: Vec3, body_rot: Quat) -> Quat {
    let to_cam_h = Vec3::new(cam_pos.x - world_pos.x, 0.0, cam_pos.z - world_pos.z);
    if to_cam_h.length_squared() > 1e-4 {
        // looking_to(dir) sets local -Z = dir; local +Z = -dir = toward camera.
        Transform::IDENTITY
            .looking_to(-to_cam_h.normalize(), Vec3::Y)
            .rotation
    } else {
        // Same XZ: face the body forward direction so text is readable from
        // in front of the player.
        // body_rot * Z  →  local -Z = body_backward  →  local +Z = body_forward.
        Transform::IDENTITY
            .looking_to(body_rot * Vec3::Z, Vec3::Y)
            .rotation
    }
}

/// Drive every `XrdsPlayerRoot` entity from the `XrdsPlayerCamera` entity.
///
/// The body always stands upright — pitch and roll are stripped so only yaw
/// is applied to the `XrdsPlayerRoot` transform.
///
/// Runs in PostUpdate BEFORE TransformPropagate so the updated player position
/// propagates down to all children in the same frame.
///
/// When `ActivePlayerAnchorEntity` is set, only the Player entity that owns the
/// active anchor is synced.  All other Player entities stay at their authored
/// positions (useful for multi-player scenes).
pub fn sync_player_root_system(
    camera_q: Query<&Transform, With<XrdsPlayerCamera>>,
    mut player_q: Query<
        (Entity, &mut Transform),
        (With<XrdsPlayerRoot>, Without<XrdsPlayerCamera>),
    >,
    active: Res<ActivePlayerAnchorEntity>,
    anchor_parent_q: Query<Option<&ChildOf>, With<XrdsPlayerAnchorRoot>>,
) {
    let Some(cam_tf) = camera_q.iter().next() else {
        return;
    };

    let yaw = cam_tf.rotation.to_euler(EulerRot::YXZ).0;
    let body_rot = Quat::from_rotation_y(yaw);

    let target_player: Option<Entity> = active.0.and_then(|anchor_ent| {
        anchor_parent_q
            .get(anchor_ent)
            .ok()
            .flatten()
            .map(|co| co.0)
    });

    for (player_entity, mut player_tf) in player_q.iter_mut() {
        if let Some(target) = target_player {
            if player_entity != target {
                continue;
            }
        }
        player_tf.translation = cam_tf.translation;
        player_tf.rotation = body_rot;
    }
}

/// Teleport the `XrdsPlayerCamera` entity when `ActivePlayerAnchorEntity` changes.
///
/// On departure: saves the camera's current pose into the departing anchor's
/// `PlayerAnchorCameraPose` so switching back restores where the player left off.
/// On arrival: restores the arriving anchor's stored pose and FOV.
///
/// Runs in PostUpdate AFTER TransformPropagate but BEFORE the anchor-mode systems,
/// so the updated camera position is seen by body/head-locked systems in the same frame.
pub fn teleport_on_anchor_switch_system(
    mut camera_q: Query<(&mut Transform, &mut Projection), With<XrdsPlayerCamera>>,
    active: Res<ActivePlayerAnchorEntity>,
    mut pose_q: Query<&mut PlayerAnchorCameraPose, With<XrdsPlayerAnchorRoot>>,
    mut last_anchor: Local<Option<Entity>>,
) {
    let current = active.0;
    if *last_anchor == current {
        return;
    }

    let Ok((mut cam_tf, mut proj)) = camera_q.single_mut() else {
        *last_anchor = current;
        return;
    };

    // Save current camera pose to the departing anchor.
    if let Some(departing) = *last_anchor {
        if let Ok(mut pose) = pose_q.get_mut(departing) {
            pose.translation = cam_tf.translation;
            pose.rotation = cam_tf.rotation;
        }
    }

    *last_anchor = current;

    // Teleport to the arriving anchor's stored pose.
    if let Some(arriving) = current {
        if let Ok(pose) = pose_q.get(arriving) {
            info!(
                "[xrds-runtime] teleport → anchor {arriving:?}: pos={:?}, fov={}°",
                pose.translation, pose.fov_deg
            );
            cam_tf.translation = pose.translation;
            cam_tf.rotation = pose.rotation;
            if let Projection::Perspective(ref mut persp) = *proj {
                persp.fov = pose.fov_deg.to_radians();
            }
        } else {
            warn!("[xrds-runtime] teleport → anchor {arriving:?}: no PlayerAnchorCameraPose found");
        }
    }
}

/// Update the `XrdsPlayerCamera` entity's FOV when the active anchor changes.
///
/// Fires once per anchor transition — a `Local` tracks the previous anchor so
/// it only runs when `ActivePlayerAnchorEntity` actually changes.
pub fn apply_anchor_fov_system(
    mut camera_q: Query<&mut Projection, With<XrdsPlayerCamera>>,
    active: Res<ActivePlayerAnchorEntity>,
    fov_q: Query<&XrdsAnchorFov, With<XrdsPlayerAnchorRoot>>,
    mut last_anchor: Local<Option<Entity>>,
) {
    let current = active.0;
    if *last_anchor == current {
        return;
    }
    *last_anchor = current;

    let Some(arriving) = current else {
        return;
    };
    let Ok(fov) = fov_q.get(arriving) else {
        return;
    };
    let Ok(mut proj) = camera_q.single_mut() else {
        return;
    };
    if let Projection::Perspective(ref mut persp) = *proj {
        persp.fov = fov.0.to_radians();
    }
}

/// Apply the arriving anchor's per-anchor exposure override.
///
/// Runs every frame in PostUpdate.  On anchor change (or when the active anchor's
/// `XrdsAnchorExposure` value differs from the cached override), this updates
/// `XrdsAnchorExposureOverride` and calls `sync_managed_scene_exposure_in_world`
/// so the camera's `Exposure` component reflects the new setting immediately.
pub fn apply_anchor_exposure_system(world: &mut World) {
    let current = world.resource::<ActivePlayerAnchorEntity>().0;
    let anchor_ev = current
        .and_then(|e| world.get::<XrdsAnchorExposure>(e))
        .and_then(|ae| ae.0);

    let prev = world
        .resource::<crate::xrds_api::environment::XrdsAnchorExposureOverride>()
        .0;
    if prev == anchor_ev {
        return;
    }

    world
        .resource_mut::<crate::xrds_api::environment::XrdsAnchorExposureOverride>()
        .0 = anchor_ev;
    crate::xrds_api::environment::sync_managed_scene_exposure_in_world(world);
}

/// World-space rotation whose local +Z faces `cam_pos` from `world_pos`.
fn face_camera(cam_pos: Vec3, world_pos: Vec3) -> Quat {
    let to_cam = cam_pos - world_pos;
    if to_cam.length_squared() < 1e-6 {
        return Quat::IDENTITY;
    }
    let up = if to_cam.normalize().abs().dot(Vec3::Y) > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    // looking_to(dir) → local -Z = dir → local +Z = -dir = toward camera
    Transform::IDENTITY
        .looking_to(-to_cam.normalize(), up)
        .rotation
}

/// Write world-space mat as both GlobalTransform (current-frame render) and
/// parent-local Transform (so next-frame TransformPropagate stays consistent).
#[inline]
fn write_world(
    tf: &mut Transform,
    gt: &mut GlobalTransform,
    world_mat: Mat4,
    child_of: Option<&ChildOf>,
    parent_q: &Query<&GlobalTransform, impl bevy::ecs::query::QueryFilter>,
) {
    let world_tf = Transform::from_matrix(world_mat);
    // GlobalTransform must be world-space so the renderer sees the correct position
    // this frame (our system runs after TransformPropagate, so we set GT directly).
    *gt = GlobalTransform::from(world_tf);
    // Transform must be parent-local so the NEXT frame's TransformPropagate
    // reproduces the correct GlobalTransform without re-applying the parent offset.
    let local_tf = if let Some(parent_gt) = child_of.and_then(|co| parent_q.get(co.0).ok()) {
        let parent_inv = Mat4::from(parent_gt.affine().inverse());
        Transform::from_matrix(parent_inv * world_mat)
    } else {
        world_tf
    };
    *tf = local_tf;
}

// ---------------------------------------------------------------------------
// Visibility diagnostic — logs whether HUD entities appear in each camera's
// VisibleEntities after CheckVisibility runs. Sampling at 60-frame intervals
// to avoid logcat spam.
// ---------------------------------------------------------------------------
pub fn vis_diag_system(
    cameras: Query<(Entity, &VisibleEntities), With<Camera3d>>,
    hud_q: Query<Entity, With<XrdsHeadLocked>>,
    mesh_q: Query<(Entity, &Name), With<bevy::prelude::Mesh3d>>,
) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    if n % 60 != 0 {
        return;
    }

    let hud_ents: Vec<Entity> = hud_q.iter().collect();
    let mesh_ents: Vec<(Entity, String)> = mesh_q
        .iter()
        .map(|(e, name)| (e, name.to_string()))
        .collect();

    for (cam_ent, visible) in &cameras {
        let total: usize = visible.entities.values().map(|v| v.len()).sum();
        let has_hud = hud_ents
            .iter()
            .any(|&h| visible.entities.values().any(|ents| ents.contains(&h)));
        // Log which named Mesh3d entities are (and are not) in this camera's VisibleEntities.
        let mut in_visible: Vec<&str> = Vec::new();
        let mut missing: Vec<&str> = Vec::new();
        for (e, name) in &mesh_ents {
            if visible.entities.values().any(|ents| ents.contains(e)) {
                in_visible.push(name.as_str());
            } else {
                missing.push(name.as_str());
            }
        }
    }
}
