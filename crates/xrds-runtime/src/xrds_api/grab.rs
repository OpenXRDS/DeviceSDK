use std::collections::HashSet;
use avian3d::prelude::{LinearVelocity, RigidBody};
use bevy::prelude::*;
use bevy_mod_outline::OutlineVolume;
use xrds_components::{
    XrDropEvent, XrGrabEvent, XrGrabHand, XrGrabHandle, XrGrabHandleOnly, XrGrabbable,
    XrGrabbed,
};
use xrds_openxr::{XrHand, XrHapticRequest, XrInput};
use super::anchor::XrdsPlayerCamera;
use super::raycast::raycast_world_meshes;

/// Whether a ray hit on `mesh` is allowed to begin a grab of `node`.
///
/// Free-for-all by default: any mesh under a grabbable node arms it, which is
/// what props and multi-submesh GLTFs need. When `node` carries
/// [`XrGrabHandleOnly`] the hit must come through a designated handle instead —
/// see that component for why panels require it.
///
/// The walk stops at `node` rather than running to the root, so a handle on some
/// unrelated ancestor cannot silently arm a descendant.
pub(super) fn grab_may_start_from(world: &World, mesh: Entity, node: Entity) -> bool {
    if world.get::<XrGrabHandleOnly>(node).is_none() {
        return true;
    }
    let mut cursor = mesh;
    loop {
        if world.get::<XrGrabHandle>(cursor).is_some() {
            return true;
        }
        if cursor == node {
            return false;
        }
        match world.get::<ChildOf>(cursor) {
            Some(parent) => cursor = parent.0,
            None => return false,
        }
    }
}
use super::state::XrdsIdIndex;

const GRAB_RANGE: f32 = 3.0;
const MAX_THROW_SPEED: f32 = 25.0;

/// Tracks which entity each hand is currently holding or hovering, plus velocity for throws.
#[derive(Resource, Default, Clone)]
pub(super) struct XrGrabState {
    pub left:        Option<Entity>,
    pub right:       Option<Entity>,
    pub left_hover:  Option<Entity>,
    pub right_hover: Option<Entity>,
    /// World position from the previous frame — used to compute throw velocity.
    pub left_prev_world_pos:  Option<Vec3>,
    pub right_prev_world_pos: Option<Vec3>,
    /// Smoothed velocity accumulated while held.
    pub left_throw_vel:  Vec3,
    pub right_throw_vel: Vec3,
    /// True if the entity was Dynamic before we grabbed it (so we restore on drop).
    pub left_was_dynamic:  bool,
    pub right_was_dynamic: bool,
}

/// SDK grab system — exclusive `Update` system.
///
/// - Each frame: raycasts from each hand aim pose to find the nearest `XrGrabbable`.
///   Adds/removes an `OutlineVolume` highlight on the hovered entity's mesh descendants.
/// - Trigger just pressed: grabs the hovered entity, fires `XrGrabEvent` + haptic.
///   If the entity has `RigidBody::Dynamic`, switches to `RigidBody::Kinematic` while held.
/// - While held: moves the entity's `Transform` to follow the aim pose, tracks velocity.
/// - Trigger just released: removes `XrGrabbed`, fires `XrDropEvent`.
///   If the entity was Dynamic, restores `RigidBody::Dynamic` and applies `LinearVelocity`.
pub(super) fn grab_system(world: &mut World) {
    let Some(xr) = world.get_resource::<XrInput>().cloned() else { return; };
    let dt = world.resource::<Time>().delta_secs().max(0.001);

    let player_root: Option<Transform> = {
        let mut q = world.query_filtered::<&Transform, With<XrdsPlayerCamera>>();
        q.iter(world).next().copied()
    };

    let left_aim  = xr.left.pose.map( |p| stage_to_world(player_root.as_ref(), &p));
    let right_aim = xr.right.pose.map(|p| stage_to_world(player_root.as_ref(), &p));

    // --- Drops ---
    let state = world.resource::<XrGrabState>().clone();
    for (hand, grabbed_entity) in [
        (XrGrabHand::Left,  state.left),
        (XrGrabHand::Right, state.right),
    ] {
        let just_released = match hand {
            XrGrabHand::Left  => xr.left.select_just_released,
            XrGrabHand::Right => xr.right.select_just_released,
        };
        if !just_released { continue; }
        let Some(entity) = grabbed_entity else { continue; };

        let (throw_vel, was_dynamic) = match hand {
            XrGrabHand::Left  => (state.left_throw_vel,  state.left_was_dynamic),
            XrGrabHand::Right => (state.right_throw_vel, state.right_was_dynamic),
        };

        world.entity_mut(entity).remove::<XrGrabbed>();

        if was_dynamic {
            let clamped = throw_vel.clamp_length_max(MAX_THROW_SPEED);
            world.entity_mut(entity).insert((RigidBody::Dynamic, LinearVelocity(clamped)));
        }

        let id = world.resource::<XrdsIdIndex>().id_of(entity);
        if let Some(id) = id {
            world.write_message(XrDropEvent { id, hand });
        }
        match hand {
            XrGrabHand::Left => {
                let mut st = world.resource_mut::<XrGrabState>();
                st.left = None;
                st.left_prev_world_pos = None;
                st.left_throw_vel = Vec3::ZERO;
                st.left_was_dynamic = false;
            }
            XrGrabHand::Right => {
                let mut st = world.resource_mut::<XrGrabState>();
                st.right = None;
                st.right_prev_world_pos = None;
                st.right_throw_vel = Vec3::ZERO;
                st.right_was_dynamic = false;
            }
        }
    }

    // --- Hover: raycast every frame and highlight the nearest grabbable ---
    let state = world.resource::<XrGrabState>().clone();
    let mut new_left_hover  = None::<Entity>;
    let mut new_right_hover = None::<Entity>;

    for (hand, aim, new_hover) in [
        (XrGrabHand::Left,  &left_aim,  &mut new_left_hover),
        (XrGrabHand::Right, &right_aim, &mut new_right_hover),
    ] {
        let already_grabbed = match hand {
            XrGrabHand::Left  => state.left.is_some(),
            XrGrabHand::Right => state.right.is_some(),
        };
        if already_grabbed { continue; }

        let Some(aim_tf) = aim else { continue; };
        let hits = raycast_world_meshes(
            world, aim_tf.translation, aim_tf.rotation * Vec3::NEG_Z, GRAB_RANGE,
        );
        *new_hover = hits.iter().find_map(|(mesh, h)| {
            let entity = world.resource::<XrdsIdIndex>().entity_of(h.id)?;
            if world.get::<XrGrabbable>(entity).is_none() { return None; }
            if !grab_may_start_from(world, *mesh, entity) { return None; }
            Some(entity)
        });
    }

    // Diff old vs new hover sets and update outlines.
    let old_set: HashSet<Entity> = [state.left_hover, state.right_hover].into_iter().flatten().collect();
    let new_set: HashSet<Entity> = [new_left_hover,   new_right_hover  ].into_iter().flatten().collect();

    for &entity in old_set.difference(&new_set) {
        let meshes = collect_mesh_descendants(world, entity);
        for e in meshes {
            if let Ok(mut em) = world.get_entity_mut(e) {
                em.remove::<OutlineVolume>();
            }
        }
    }
    for &entity in new_set.difference(&old_set) {
        let meshes = collect_mesh_descendants(world, entity);
        for e in meshes {
            if let Ok(mut em) = world.get_entity_mut(e) {
                em.insert(OutlineVolume {
                    visible: true,
                    colour:  Color::srgb(0.0, 0.9, 1.0),
                    width:   3.0,
                });
            }
        }
    }

    {
        let mut st = world.resource_mut::<XrGrabState>();
        st.left_hover  = new_left_hover;
        st.right_hover = new_right_hover;
    }

    // --- New grabs ---
    for (hand, aim, hover_entity) in [
        (XrGrabHand::Left,  &left_aim,  new_left_hover),
        (XrGrabHand::Right, &right_aim, new_right_hover),
    ] {
        let already_grabbed = match hand {
            XrGrabHand::Left  => world.resource::<XrGrabState>().left.is_some(),
            XrGrabHand::Right => world.resource::<XrGrabState>().right.is_some(),
        };
        if already_grabbed { continue; }

        let just_pressed = match hand {
            XrGrabHand::Left  => xr.left.select_just_pressed,
            XrGrabHand::Right => xr.right.select_just_pressed,
        };
        if !just_pressed { continue; }

        let (Some(aim_tf), Some(entity)) = (aim, hover_entity) else { continue; };

        let (world_pos, world_rot) = {
            let Some(gt) = world.get::<GlobalTransform>(entity) else { continue; };
            let (_, rot, trans) = gt.to_scale_rotation_translation();
            (trans, rot)
        };
        let offset          = aim_tf.rotation.inverse() * (world_pos - aim_tf.translation);
        let rotation_offset = aim_tf.rotation.inverse() * world_rot;

        // Switch Dynamic → Kinematic so physics doesn't fight the grab movement.
        let was_dynamic = world.get::<RigidBody>(entity).copied() == Some(RigidBody::Dynamic);
        if was_dynamic {
            world.entity_mut(entity).insert(RigidBody::Kinematic);
        }

        world.entity_mut(entity).insert(XrGrabbed { hand, offset, rotation_offset });
        let id = world.resource::<XrdsIdIndex>().id_of(entity);
        if let Some(id) = id {
            world.write_message(XrGrabEvent { id, hand });
        }
        world.write_message(XrHapticRequest {
            hand:          match hand { XrGrabHand::Left => XrHand::Left, XrGrabHand::Right => XrHand::Right },
            amplitude:     0.6,
            duration_secs: 0.08,
            frequency:     0.0,
        });
        match hand {
            XrGrabHand::Left => {
                let mut st = world.resource_mut::<XrGrabState>();
                st.left = Some(entity);
                st.left_prev_world_pos = Some(world_pos);
                st.left_throw_vel = Vec3::ZERO;
                st.left_was_dynamic = was_dynamic;
            }
            XrGrabHand::Right => {
                let mut st = world.resource_mut::<XrGrabState>();
                st.right = Some(entity);
                st.right_prev_world_pos = Some(world_pos);
                st.right_throw_vel = Vec3::ZERO;
                st.right_was_dynamic = was_dynamic;
            }
        }
    }

    // --- Follow: update grabbed entity transforms and track velocity ---
    let state = world.resource::<XrGrabState>().clone();
    for (hand, aim) in [
        (XrGrabHand::Left,  &left_aim),
        (XrGrabHand::Right, &right_aim),
    ] {
        let (Some(entity), Some(aim_tf)) = (
            match hand { XrGrabHand::Left => state.left, XrGrabHand::Right => state.right },
            aim,
        ) else { continue; };

        let Some(grabbed) = world.get::<XrGrabbed>(entity).cloned() else { continue; };

        let new_world_pos = aim_tf.translation + aim_tf.rotation * grabbed.offset;
        let new_world_rot = aim_tf.rotation * grabbed.rotation_offset;

        // Velocity tracking: (current - prev) / dt, smoothed with EMA.
        let frame_vel = match hand {
            XrGrabHand::Left => state.left_prev_world_pos,
            XrGrabHand::Right => state.right_prev_world_pos,
        }.map(|prev| (new_world_pos - prev) / dt)
         .unwrap_or(Vec3::ZERO);

        {
            let mut st = world.resource_mut::<XrGrabState>();
            match hand {
                XrGrabHand::Left => {
                    // EMA smoothing: blend 70% new + 30% old to reduce spike throws
                    st.left_throw_vel = frame_vel * 0.7 + st.left_throw_vel * 0.3;
                    st.left_prev_world_pos = Some(new_world_pos);
                }
                XrGrabHand::Right => {
                    st.right_throw_vel = frame_vel * 0.7 + st.right_throw_vel * 0.3;
                    st.right_prev_world_pos = Some(new_world_pos);
                }
            }
        }

        let parent_gt: Option<GlobalTransform> = world
            .get::<ChildOf>(entity)
            .and_then(|co| world.get::<GlobalTransform>(co.0))
            .cloned();

        let (local_pos, local_rot) = match parent_gt {
            Some(pgt) => {
                let (_, parent_rot, _) = pgt.to_scale_rotation_translation();
                let inv       = pgt.affine().inverse();
                let local_pos = inv.transform_point3(new_world_pos);
                let local_rot = parent_rot.inverse() * new_world_rot;
                (local_pos, local_rot)
            }
            None => (new_world_pos, new_world_rot),
        };

        if let Some(mut tf) = world.get_mut::<Transform>(entity) {
            tf.translation = local_pos;
            tf.rotation    = local_rot;
        }
    }
}

/// Recursively collect all entities with a `Mesh3d` component under `root`.
fn collect_mesh_descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut out = Vec::new();
    collect_inner(world, root, &mut out);
    out
}

fn collect_inner(world: &World, e: Entity, out: &mut Vec<Entity>) {
    if world.get::<Mesh3d>(e).is_some() {
        out.push(e);
    }
    let children: Vec<Entity> = world
        .get::<Children>(e)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    for child in children {
        collect_inner(world, child, out);
    }
}

/// Convert an XR stage-space aim pose to world space using the player's locomotion root.
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
