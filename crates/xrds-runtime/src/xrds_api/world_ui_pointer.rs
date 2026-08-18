use bevy::prelude::*;
use xrds_components::{
    XrGrabHand, XrdsId,
    XrdsWorldPointerCursors, XrdsWorldPointerHit, XrdsWorldPointerState,
    XrdsWorldSurface, XrWorldHoverEnterEvent, XrWorldHoverExitEvent,
};
use xrds_openxr::XrInput;
use super::anchor::XrdsPlayerCamera;
use super::state::XrdsIdIndex;

const POINTER_RANGE: f32 = 5.0;
const CURSOR_RADIUS: f32 = 0.008;

/// Spawns the two cursor sphere visuals (left/right hand) at startup.
///
/// Cursors are initially hidden; the pointer system moves and shows/hides them each frame.
pub(super) fn spawn_world_ui_cursors_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cursors: ResMut<XrdsWorldPointerCursors>,
) {
    let mesh = meshes.add(Sphere::new(CURSOR_RADIUS).mesh().uv(8, 8));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.85),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let left = commands
        .spawn((
            Name::new("WorldUiCursor_Left"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    let right = commands
        .spawn((
            Name::new("WorldUiCursor_Right"),
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    cursors.left  = Some(left);
    cursors.right = Some(right);
}

/// Per-frame world-UI pointer system (exclusive `Update`).
///
/// For each XR hand:
/// 1. Casts a ray from the controller aim pose.
/// 2. Finds the nearest [`XrdsWorldSurface`] hit by doing precise ray-vs-oriented-plane
///    intersection (not AABB), then checking the hit point is within the panel bounds.
/// 3. Converts the intersection to panel-local UV (0..1, 0..1).
/// 4. Fires [`XrWorldHoverEnterEvent`] / [`XrWorldHoverExitEvent`] on transitions.
/// 5. Moves the cursor visual to the hit point; hides it when no panel is hit.
pub(super) fn world_ui_pointer_system(world: &mut World) {
    let Some(xr) = world.get_resource::<XrInput>().cloned() else { return; };

    let player_root: Option<Transform> = {
        let mut q = world.query_filtered::<&Transform, With<XrdsPlayerCamera>>();
        q.iter(world).next().copied()
    };

    let left_aim  = xr.left.pose.map( |p| stage_to_world(player_root.as_ref(), &p));
    let right_aim = xr.right.pose.map(|p| stage_to_world(player_root.as_ref(), &p));

    let prev_state = world.resource::<XrdsWorldPointerState>().clone();

    let new_left = cast_hand(world, &left_aim);
    let new_right = cast_hand(world, &right_aim);

    // Fire enter/exit events on hover transitions.
    fire_hover_events(world, XrGrabHand::Left,  &prev_state.left,  &new_left);
    fire_hover_events(world, XrGrabHand::Right, &prev_state.right, &new_right);

    // Update pointer state resource.
    {
        let mut state = world.resource_mut::<XrdsWorldPointerState>();
        state.left  = new_left.clone();
        state.right = new_right.clone();
    }

    // Reposition cursor visuals. Copy the Entity ids (Copy type) so the borrow ends
    // before the mutable update_cursor calls.
    let left_cursor  = world.resource::<XrdsWorldPointerCursors>().left;
    let right_cursor = world.resource::<XrdsWorldPointerCursors>().right;
    update_cursor(world, left_cursor,  new_left.as_ref());
    update_cursor(world, right_cursor, new_right.as_ref());
}

fn cast_hand(world: &mut World, aim: &Option<Transform>) -> Option<XrdsWorldPointerHit> {
    let aim_tf = aim.as_ref()?;
    let origin = aim_tf.translation;
    let dir = (aim_tf.rotation * Vec3::NEG_Z).normalize_or_zero();
    if dir == Vec3::ZERO { return None; }
    nearest_panel_hit(world, origin, dir, POINTER_RANGE)
}

fn fire_hover_events(
    world: &mut World,
    hand: XrGrabHand,
    prev: &Option<XrdsWorldPointerHit>,
    new: &Option<XrdsWorldPointerHit>,
) {
    match (prev, new) {
        (None, Some(hit)) => {
            world.write_message(XrWorldHoverEnterEvent { panel_id: hit.panel_id, hand, uv: hit.uv });
        }
        (Some(prev_hit), None) => {
            world.write_message(XrWorldHoverExitEvent { panel_id: prev_hit.panel_id, hand });
        }
        (Some(prev_hit), Some(new_hit)) if prev_hit.panel_id != new_hit.panel_id => {
            world.write_message(XrWorldHoverExitEvent { panel_id: prev_hit.panel_id, hand });
            world.write_message(XrWorldHoverEnterEvent { panel_id: new_hit.panel_id, hand, uv: new_hit.uv });
        }
        _ => {}
    }
}

/// Find the nearest `XrdsWorldSurface` hit by the ray.
///
/// Collects surface data into a Vec first (releasing the query borrow) then resolves
/// XRDS ids — mirrors the same two-phase pattern used in `raycast_world`.
fn nearest_panel_hit(
    world: &mut World,
    origin: Vec3,
    dir: Vec3,
    max_distance: f32,
) -> Option<XrdsWorldPointerHit> {
    // Phase 1: collect surface geometry while holding the query borrow.
    let surfaces: Vec<(Entity, XrdsWorldSurface, GlobalTransform)> = {
        let mut q = world.query::<(Entity, &XrdsWorldSurface, &GlobalTransform)>();
        q.iter(world)
            .map(|(e, s, gt)| (e, s.clone(), *gt))
            .collect()
    };

    // Phase 2: intersect each surface (no active borrow on world).
    let mut best: Option<(f32, Entity, Vec2, Vec3)> = None;

    for (entity, surface, gt) in &surfaces {
        if !surface.enabled { continue; }

        let (_, panel_rot, panel_pos) = gt.to_scale_rotation_translation();
        let panel_normal = panel_rot * Vec3::Z;  // local +Z = front face

        let denom = dir.dot(panel_normal);
        // Only accept hits from the front face (denom must be negative: ray opposes normal).
        if denom >= -1e-6 { continue; }

        let t = (panel_pos - origin).dot(panel_normal) / denom;
        if t < 0.0 || t > max_distance { continue; }

        let world_hit = origin + dir * t;

        // Convert to panel-local space and bounds-check.
        let local_hit = gt.affine().inverse().transform_point3(world_hit);
        let half_w = surface.size.x * 0.5;
        let half_h = surface.size.y * 0.5;
        if local_hit.x.abs() > half_w || local_hit.y.abs() > half_h { continue; }

        let uv = Vec2::new(
            (local_hit.x / surface.size.x) + 0.5,
            (local_hit.y / surface.size.y) + 0.5,
        );

        if best.as_ref().map_or(true, |(bt, ..)| t < *bt) {
            best = Some((t, *entity, uv, world_hit));
        }
    }

    // Phase 3: resolve XRDS id for the winner.
    let (_, entity, uv, world_point) = best?;
    let panel_id = {
        let mut cur = entity;
        loop {
            if let Some(id) = world.resource::<XrdsIdIndex>().id_of(cur) {
                break Some(id);
            }
            let Some(parent) = world.get::<ChildOf>(cur).map(|co| co.0) else { break None; };
            cur = parent;
        }
    }?;
    Some(XrdsWorldPointerHit { entity, panel_id, uv, world_point })
}

fn update_cursor(world: &mut World, cursor_entity: Option<Entity>, hit: Option<&XrdsWorldPointerHit>) {
    let Some(entity) = cursor_entity else { return; };
    let Ok(mut e) = world.get_entity_mut(entity) else { return; };

    match hit {
        Some(h) => {
            if let Some(mut vis) = e.get_mut::<Visibility>() {
                *vis = Visibility::Visible;
            }
            if let Some(mut tf) = e.get_mut::<Transform>() {
                tf.translation = h.world_point;
            }
        }
        None => {
            if let Some(mut vis) = e.get_mut::<Visibility>() {
                *vis = Visibility::Hidden;
            }
        }
    }
}

/// Convert an XR stage-space aim pose to world space using the player's locomotion root.
/// Mirrors the same helper in `grab.rs`.
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
