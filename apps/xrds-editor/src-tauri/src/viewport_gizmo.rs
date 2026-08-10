use bevy::prelude::*;
use bevy::gizmos::config::GizmoConfigGroup;
use bevy_mod_outline::OutlineVolume;
use xrds_runtime::{XrdsAnchorFov, XrdsIdIndex, XrdsPlayerAnchorRoot};
use xrds_runtime::sdk::XrdsId;
use xrds_scene_graph::{XrdsInteractionZoneShape, XrdsPhysicsBody, XrdsSceneCameraProjection, XrdsSceneNodePayload};
use crate::editor_state::{EditorSession, EditorState, GizmoAxis, GizmoMode};
use crate::viewport_camera::EditorCameraMarker;

// ---------------------------------------------------------------------------
// Grid gizmo group (uses normal depth test, not always-on-top)
// ---------------------------------------------------------------------------

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct GridGizmoGroup;

// ---------------------------------------------------------------------------
// Public helpers used by gizmo_interaction too
// ---------------------------------------------------------------------------

pub fn gizmo_scale(cam_pos: Vec3, target: Vec3) -> f32 {
    (cam_pos.distance(target) * 0.18).max(0.4)
}

pub fn draw_axes(gizmos: &mut Gizmos, origin: Vec3, rotation: Quat, length: f32) {
    let x = rotation * Vec3::X;
    let y = rotation * Vec3::Y;
    let z = rotation * Vec3::Z;
    gizmos.arrow(origin, origin + x * length, Color::srgb_u8(220, 50, 50));
    gizmos.arrow(origin, origin + y * length, Color::srgb_u8(50, 200, 50));
    gizmos.arrow(origin, origin + z * length, Color::srgb_u8(50, 80, 220));
}

pub fn axis_dir(axis: GizmoAxis) -> Vec3 {
    match axis { GizmoAxis::X => Vec3::X, GizmoAxis::Y => Vec3::Y, GizmoAxis::Z => Vec3::Z }
}

// ---------------------------------------------------------------------------
// Transform gizmo
// ---------------------------------------------------------------------------

pub fn transform_gizmo_system(
    mut gizmos:       Gizmos,
    editor_state:     Res<EditorState>,
    session:          Res<EditorSession>,
    id_index:         Res<XrdsIdIndex>,
    global_tf_q:      Query<&GlobalTransform>,
    camera_q:         Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
) {
    if editor_state.selection.is_empty() { return; }
    let doc = session.0.document();
    let ids = editor_state.selection.ids();
    // Use the entity's GlobalTransform (world-space) so the gizmo is correct for
    // child nodes whose local transform ≠ world transform.
    let sum: Vec3 = ids.iter().map(|&id| {
        let world_pos = id_index.entity_of(XrdsId::from(id))
            .and_then(|e| global_tf_q.get(e).ok())
            .map(|gt| gt.translation());
        world_pos.unwrap_or_else(|| {
            let local = editor_state.pending_translation_for(id)
                .or_else(|| doc.node(id).map(|n| n.transform.translation))
                .unwrap_or([0.0, 0.0, 0.0]);
            Vec3::from_array(local)
        })
    }).fold(Vec3::ZERO, |a, b| a + b);
    let origin = sum / ids.len() as f32;
    let cam_pos = camera_q.single().map(|(_, gt)| gt.translation()).unwrap_or(Vec3::ZERO);
    let scale = gizmo_scale(cam_pos, origin);

    if let Some(primary_id) = editor_state.selection.primary() {
        if let Some(node) = doc.node(primary_id) {
            let [qx, qy, qz, qw] = node.transform.rotation_quat_xyzw;
            let rotation = Quat::from_xyzw(qx, qy, qz, qw);
            if editor_state.selection.count() == 1 {
                if let XrdsSceneNodePayload::Camera(cam) = &node.payload {
                    draw_camera_frustum(&mut gizmos, origin, rotation, &cam.projection);
                }
            }
            if editor_state.gizmo_mode == GizmoMode::Rotate {
                draw_rotation_rings(&mut gizmos, origin, scale, editor_state.gizmo_hover);
                return;
            }
        }
    }
    match editor_state.gizmo_mode {
        GizmoMode::Translate => draw_axes(&mut gizmos, origin, Quat::IDENTITY, scale),
        GizmoMode::Rotate    => draw_rotation_rings(&mut gizmos, origin, scale, editor_state.gizmo_hover),
        GizmoMode::Scale     => draw_scale_handles(&mut gizmos, origin, scale, editor_state.gizmo_hover),
    }
}

fn draw_camera_frustum(gizmos: &mut Gizmos, pos: Vec3, rot: Quat, projection: &XrdsSceneCameraProjection) {
    const CAP: f32 = 8.0; const ASPECT: f32 = 16.0 / 9.0;
    let color = Color::srgb(0.9, 0.85, 0.2);
    let (fov_rad, near, far, ortho) = match *projection {
        XrdsSceneCameraProjection::Perspective { fov_deg, near, far, .. } =>
            (fov_deg.to_radians(), near, far.map_or(CAP, |f| f.min(CAP)), None),
        XrdsSceneCameraProjection::Orthographic { scale, near, far, .. } =>
            (0.0_f32, near, far.min(CAP), Some(scale)),
    };
    let corners = |dist: f32| -> [Vec3; 4] {
        let (hw, hh) = if let Some(s) = ortho { (s * ASPECT * 0.5, s * 0.5) }
                       else { let h = dist*(fov_rad*0.5).tan(); (h*ASPECT, h) };
        [pos+rot*Vec3::new(-hw,-hh,-dist), pos+rot*Vec3::new(hw,-hh,-dist),
         pos+rot*Vec3::new(hw,hh,-dist),   pos+rot*Vec3::new(-hw,hh,-dist)]
    };
    let n = corners(near); let f = corners(far);
    for i in 0..4 { gizmos.line(n[i], n[(i+1)%4], color); gizmos.line(f[i], f[(i+1)%4], color); gizmos.line(n[i], f[i], color); }
    for i in 0..4 { gizmos.line(pos, n[i], color); }
}

fn draw_rotation_rings(gizmos: &mut Gizmos, origin: Vec3, scale: f32, hover: Option<GizmoAxis>) {
    use std::f32::consts::FRAC_PI_2;
    let hi = Color::srgb(1.0, 1.0, 0.5);
    let cx = if hover == Some(GizmoAxis::X) { hi } else { Color::srgb_u8(220, 50, 50) };
    let cy = if hover == Some(GizmoAxis::Y) { hi } else { Color::srgb_u8(50, 200, 50) };
    let cz = if hover == Some(GizmoAxis::Z) { hi } else { Color::srgb_u8(50, 80, 220) };
    gizmos.circle(Isometry3d::new(origin, Quat::from_rotation_y(FRAC_PI_2)), scale, cx);
    gizmos.circle(Isometry3d::new(origin, Quat::from_rotation_x(FRAC_PI_2)), scale, cy);
    gizmos.circle(Isometry3d::new(origin, Quat::IDENTITY), scale, cz);
}

fn draw_scale_handles(gizmos: &mut Gizmos, origin: Vec3, scale: f32, hover: Option<GizmoAxis>) {
    let hi = Color::srgb(1.0, 1.0, 0.5);
    let ch = scale * 0.08;
    for (dir, col) in [(Vec3::X, if hover==Some(GizmoAxis::X){hi}else{Color::srgb_u8(220,50,50)}),
                       (Vec3::Y, if hover==Some(GizmoAxis::Y){hi}else{Color::srgb_u8(50,200,50)}),
                       (Vec3::Z, if hover==Some(GizmoAxis::Z){hi}else{Color::srgb_u8(50,80,220)})] {
        let tip = origin + dir * scale;
        gizmos.line(origin, tip, col);
        gizmos.cuboid(Transform { translation: tip, scale: Vec3::splat(ch * 2.0), ..Default::default() }, col);
    }
}

// ---------------------------------------------------------------------------
// Selection outlines
// ---------------------------------------------------------------------------

pub fn update_selection_outline(
    mut commands:    Commands,
    editor_state:    Res<EditorState>,
    id_index:        Res<XrdsIdIndex>,
    children_query:  Query<&Children>,
    mesh_query:      Query<(), With<Mesh3d>>,
    mut prev_sel:    Local<Vec<xrds_scene_graph::XrdsSceneNodeId>>,
    mut outlined:    Local<Vec<Entity>>,
) {
    if editor_state.needs_full_reimport { outlined.clear(); prev_sel.clear(); return; }
    let current_ids: Vec<_> = editor_state.selection.ids().to_vec();
    if *prev_sel != current_ids {
        for e in outlined.drain(..) {
            if let Ok(mut ec) = commands.get_entity(e) { ec.remove::<OutlineVolume>(); }
        }
        *prev_sel = current_ids.clone();
    }
    if current_ids.is_empty() { return; }
    for &node_id in &current_ids {
        let Some(root) = id_index.entity_of(XrdsId::from(node_id)) else { continue; };
        let mut candidates = Vec::new();
        collect_mesh_entities(root, &children_query, &mesh_query, &mut candidates);
        for e in candidates {
            if !outlined.contains(&e) {
                if let Ok(mut ec) = commands.get_entity(e) {
                    ec.insert(OutlineVolume { visible: true, colour: Color::srgb(1.0, 0.55, 0.0), width: 2.0 });
                }
                outlined.push(e);
            }
        }
    }
}

fn collect_mesh_entities(e: Entity, cq: &Query<&Children>, mq: &Query<(), With<Mesh3d>>, out: &mut Vec<Entity>) {
    if mq.contains(e) { out.push(e); }
    if let Ok(children) = cq.get(e) { for &c in children { collect_mesh_entities(c, cq, mq, out); } }
}

// ---------------------------------------------------------------------------
// Floor grid
// ---------------------------------------------------------------------------

pub fn floor_grid_system(mut gizmos: Gizmos<GridGizmoGroup>, editor_state: Res<EditorState>) {
    if !editor_state.show_grid { return; }
    const E: i32 = 10;
    let ef = E as f32;
    for i in -E..=E {
        let fi = i as f32;
        let (cx, cz) = if i == 0 {
            (Color::srgba(0.7, 0.2, 0.2, 0.55), Color::srgba(0.2, 0.2, 0.7, 0.55))
        } else if i % 5 == 0 {
            let c = Color::srgba(0.55, 0.55, 0.55, 0.45); (c, c)
        } else {
            let c = Color::srgba(0.4, 0.4, 0.4, 0.22); (c, c)
        };
        gizmos.line(Vec3::new(fi, 0.0, -ef), Vec3::new(fi, 0.0, ef), cz);
        gizmos.line(Vec3::new(-ef, 0.0, fi), Vec3::new(ef, 0.0, fi), cx);
    }
}

// ---------------------------------------------------------------------------
// Light rays + interaction zone gizmos
// ---------------------------------------------------------------------------

pub fn light_rays_system(
    mut gizmos:   Gizmos,
    editor_state: Res<EditorState>,
    session:      Res<EditorSession>,
    camera_q:     Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
) {
    if !editor_state.light_rays_selected { return; }
    let cam_pos = camera_q.single().map(|(_, gt)| gt.translation()).unwrap_or(Vec3::ZERO);
    let doc = session.0.document();
    for node in &doc.nodes {
        if !node.visible || !editor_state.selection.contains(node.id) { continue; }
        let pos = Vec3::from_array(node.transform.translation);
        let [qx, qy, qz, qw] = node.transform.rotation_quat_xyzw;
        let rot = Quat::from_xyzw(qx, qy, qz, qw);
        match &node.payload {
            XrdsSceneNodePayload::PointLight(l) => {
                let [r,g,b,_] = l.color;
                let ray = Color::srgba(r, g, b, 0.7);
                let ring = Color::srgba(r, g, b, 0.3);
                use std::f32::consts::FRAC_PI_2;
                for dir in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
                    gizmos.line(pos, pos + dir * l.range, ray);
                }
                gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_x(FRAC_PI_2)), l.range, ring);
                gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_y(FRAC_PI_2)), l.range, ring);
                gizmos.circle(Isometry3d::new(pos, Quat::IDENTITY), l.range, ring);
            }
            XrdsSceneNodePayload::DirectionalLight(l) => {
                let [r,g,b,_] = l.color;
                let s = gizmo_scale(cam_pos, pos);
                let fwd = rot * Vec3::NEG_Z;
                gizmos.circle(Isometry3d::new(pos, circle_normal_rotation(fwd)), s*0.45, Color::srgba(r,g,b,0.75));
                gizmos.arrow(pos - fwd*s*1.1, pos - fwd*s*0.15, Color::srgba(1.0, 0.85, 0.2, 0.9));
            }
            XrdsSceneNodePayload::SpotLight(l) => {
                let [r,g,b,_] = l.color;
                let fwd = rot * Vec3::NEG_Z;
                let tip = pos + fwd * l.range;
                gizmos.line(pos, tip, Color::srgba(r,g,b,0.85));
                let cr = circle_normal_rotation(fwd);
                gizmos.circle(Isometry3d::new(tip, cr), l.range * l.outer_angle.tan(), Color::srgba(r,g,b,0.65));
            }
            _ => {}
        }
    }
}

pub fn interaction_zone_gizmo_system(
    mut gizmos:   Gizmos,
    editor_state: Res<EditorState>,
    session:      Res<EditorSession>,
) {
    let doc = session.0.document();
    for node in &doc.nodes {
        let XrdsSceneNodePayload::InteractionZone(zone) = &node.payload else { continue; };
        let color = if editor_state.selection.contains(node.id) { Color::srgba(0.2,0.9,0.4,0.9) }
                    else { Color::srgba(0.2,0.7,0.3,0.35) };
        let pos = Vec3::from_array(node.transform.translation);
        let [qx,qy,qz,qw] = node.transform.rotation_quat_xyzw;
        let rot = Quat::from_xyzw(qx, qy, qz, qw);
        match zone.shape {
            XrdsInteractionZoneShape::Box { half_extents } => {
                gizmos.cuboid(Transform::from_translation(pos).with_rotation(rot).with_scale(Vec3::from_array(half_extents)*2.0), color);
            }
            XrdsInteractionZoneShape::Sphere { radius } => {
                gizmos.sphere(Isometry3d::new(pos, rot), radius, color);
            }
        }
    }
}

/// Draw a stick-figure gizmo at each PlayerSpawn node position.
/// Selected spawns are drawn brighter; all others are faint green.
pub fn player_spawn_gizmo_system(
    mut gizmos:   Gizmos,
    editor_state: Res<EditorState>,
    session:      Res<EditorSession>,
) {
    let doc = session.0.document();
    for node in &doc.nodes {
        let xrds_scene_graph::XrdsSceneNodePayload::PlayerSpawn(_) = &node.payload else { continue; };
        let is_selected = editor_state.selection.contains(node.id);
        let col_body = if is_selected { Color::srgba(0.4, 1.0, 0.6, 0.95) }
                       else           { Color::srgba(0.3, 0.8, 0.4, 0.55) };
        let col_dim  = if is_selected { Color::srgba(0.4, 1.0, 0.6, 0.65) }
                       else           { Color::srgba(0.3, 0.8, 0.4, 0.30) };

        let base  = Vec3::from_array(node.transform.translation);
        let [qx,qy,qz,qw] = node.transform.rotation_quat_xyzw;
        let rot   = Quat::from_xyzw(qx,qy,qz,qw);
        let up    = rot * Vec3::Y;
        let right = rot * Vec3::X;
        let fwd   = rot * Vec3::NEG_Z;

        // Legs
        let hip   = base + up * 0.9;
        let lfoot = base + (-right * 0.15);
        let rfoot = base + ( right * 0.15);
        gizmos.line(hip, lfoot, col_body);
        gizmos.line(hip, rfoot, col_body);

        // Torso
        let shoulder = base + up * 1.6;
        gizmos.line(hip, shoulder, col_body);

        // Arms
        let lelbow = shoulder + (-right * 0.35) + up * -0.25;
        let relbow = shoulder + ( right * 0.35) + up * -0.25;
        gizmos.line(shoulder, lelbow, col_body);
        gizmos.line(shoulder, relbow, col_body);

        // Head (circle)
        let head = base + up * 1.75;
        gizmos.circle(Isometry3d::new(head, rot), 0.12, col_body);

        // Direction arrow (forward)
        gizmos.arrow(base + up * 0.5, base + up * 0.5 + fwd * 0.6, col_dim);

        // Ground ring
        gizmos.circle(Isometry3d::new(base, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.25, col_dim);
    }
}

/// Draw a wireframe box for each PlayerSpawnZone node.
/// Green when unselected, bright green when selected.
pub fn spawn_zone_gizmo_system(
    mut gizmos:   Gizmos,
    editor_state: Res<EditorState>,
    session:      Res<EditorSession>,
) {
    let doc = session.0.document();
    for node in &doc.nodes {
        let XrdsSceneNodePayload::PlayerSpawnZone(zone) = &node.payload else { continue; };
        let is_selected = editor_state.selection.contains(node.id);
        let color = if is_selected { Color::srgba(0.3, 1.0, 0.5, 0.9) }
                    else           { Color::srgba(0.2, 0.7, 0.35, 0.45) };
        let pos = Vec3::from_array(node.transform.translation);
        let [qx, qy, qz, qw] = node.transform.rotation_quat_xyzw;
        let rot = Quat::from_xyzw(qx, qy, qz, qw);
        let size = Vec3::new(zone.size[0], zone.size[1], zone.size[2]);
        gizmos.cuboid(Transform::from_translation(pos).with_rotation(rot).with_scale(size), color);
    }
}

// `world_panel_gizmo_system` drew a wireframe rectangle for each WorldPanel node,
// since that payload had no mesh of its own to select against. Retired with
// WorldPanel. A `Panel` node needs no equivalent: it now carries a real
// `Mesh3d` backdrop (see `apply_panel_backdrop_in_world`), so `update_selection_outline`
// below already outlines it through the same generic, mesh-based path every
// other primitive uses — nothing panel-specific to draw.

fn circle_normal_rotation(normal: Vec3) -> Quat {
    if (normal + Vec3::Z).length_squared() < 1e-5 { Quat::from_rotation_x(std::f32::consts::PI) }
    else { Quat::from_rotation_arc(Vec3::Z, normal) }
}

// ---------------------------------------------------------------------------
// FOV overlay
// ---------------------------------------------------------------------------

/// Draw a perspective frustum wireframe for every PlayerAnchor when the
/// FOV overlay toggle is on.  The pyramid shows the exact viewing cone the
/// anchor would produce at eye level — useful for checking sight-lines and
/// overlap between multiple anchor viewpoints.
///
/// Selected anchors are drawn bright cyan; non-selected are drawn faint.
pub fn fov_overlay_system(
    mut gizmos:   Gizmos,
    editor_state: Res<EditorState>,
    id_index:     Res<XrdsIdIndex>,
    anchor_q:     Query<(Entity, &GlobalTransform, &XrdsAnchorFov), With<XrdsPlayerAnchorRoot>>,
) {
    if !editor_state.show_fov_overlay { return; }

    const DEPTH: f32 = 4.0;
    const ASPECT: f32 = 16.0 / 9.0;

    for (entity, gt, fov_comp) in anchor_q.iter() {
        let is_selected = id_index.id_of(entity)
            .map(|xid| editor_state.selection.contains(xrds_scene_graph::XrdsSceneNodeId(xid.0)))
            .unwrap_or(false);

        let col_edge = if is_selected { Color::srgba(0.2, 0.9, 1.0, 0.92) }
                       else           { Color::srgba(0.2, 0.7, 0.85, 0.40) };
        let col_far  = if is_selected { Color::srgba(0.2, 0.9, 1.0, 0.70) }
                       else           { Color::srgba(0.2, 0.7, 0.85, 0.25) };

        let tf    = gt.compute_transform();
        let origin = tf.translation;
        let rot    = tf.rotation;
        let fwd    = rot * Vec3::NEG_Z;
        let up     = rot * Vec3::Y;
        let right  = rot * Vec3::X;

        let half_v = (fov_comp.0.to_radians() / 2.0).tan() * DEPTH;
        let half_h = half_v * ASPECT;

        let far_center = origin + fwd * DEPTH;
        let tl = far_center + up * half_v - right * half_h;
        let tr = far_center + up * half_v + right * half_h;
        let bl = far_center - up * half_v - right * half_h;
        let br = far_center - up * half_v + right * half_h;

        // 4 frustum edges from apex
        gizmos.line(origin, tl, col_edge);
        gizmos.line(origin, tr, col_edge);
        gizmos.line(origin, bl, col_edge);
        gizmos.line(origin, br, col_edge);

        // Far rectangle
        gizmos.line(tl, tr, col_far);
        gizmos.line(tr, br, col_far);
        gizmos.line(br, bl, col_far);
        gizmos.line(bl, tl, col_far);

        // Near cross-hair dot (small cross at apex so the anchor origin is clear)
        let t = 0.06;
        gizmos.line(origin - right * t, origin + right * t, col_edge);
        gizmos.line(origin - up    * t, origin + up    * t, col_edge);
    }
}

// ---------------------------------------------------------------------------
// Physics collider gizmos
// ---------------------------------------------------------------------------

/// Draw wireframe collider shapes for every primitive node that has a non-None
/// physics body.  Green = Static, orange = Dynamic.  Selected nodes are drawn
/// brighter and fully opaque; unselected are dimmed and semi-transparent.
pub fn physics_collider_gizmo_system(
    mut gizmos:   Gizmos,
    editor_state: Res<EditorState>,
    session:      Res<EditorSession>,
) {
    let doc = session.0.document();
    for node in &doc.nodes {
        let (pb, shape) = match &node.payload {
            XrdsSceneNodePayload::Cube(c)     if !c.physics_body.is_none() => (c.physics_body, ColliderShape::Cube(c.size)),
            XrdsSceneNodePayload::Sphere(c)   if !c.physics_body.is_none() => (c.physics_body, ColliderShape::Sphere(c.radius)),
            XrdsSceneNodePayload::Cylinder(c) if !c.physics_body.is_none() => (c.physics_body, ColliderShape::Cylinder(c.radius, c.height)),
            XrdsSceneNodePayload::Plane3D(c)  if !c.physics_body.is_none() => (c.physics_body, ColliderShape::Plane),
            _ => continue,
        };

        let is_selected = editor_state.selection.contains(node.id);
        let alpha = if is_selected { 0.92 } else { 0.45 };
        let color = match pb {
            XrdsPhysicsBody::Static  => Color::srgba(0.15, 0.95, 0.3,  alpha),
            XrdsPhysicsBody::Dynamic => Color::srgba(1.0,  0.62, 0.05, alpha),
            XrdsPhysicsBody::None    => continue,
        };

        let pos = Vec3::from_array(node.transform.translation);
        let [qx, qy, qz, qw] = node.transform.rotation_quat_xyzw;
        let rot = Quat::from_xyzw(qx, qy, qz, qw);
        let scale = Vec3::from_array(node.transform.scale);

        match shape {
            ColliderShape::Cube([sx, sy, sz]) => {
                gizmos.cuboid(
                    Transform::from_translation(pos)
                        .with_rotation(rot)
                        .with_scale(Vec3::new(sx, sy, sz) * scale),
                    color,
                );
            }
            ColliderShape::Sphere(radius) => {
                let r = radius * scale.x.max(scale.y).max(scale.z);
                gizmos.sphere(Isometry3d::new(pos, rot), r, color);
            }
            ColliderShape::Cylinder(radius, height) => {
                let r  = radius * scale.x.max(scale.z);
                let hh = height * scale.y * 0.5;
                let up = rot * Vec3::Y;
                let top = pos + up * hh;
                let bot = pos - up * hh;
                let ring_rot = rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
                gizmos.circle(Isometry3d::new(top, ring_rot), r, color);
                gizmos.circle(Isometry3d::new(bot, ring_rot), r, color);
                for angle_deg in [0.0f32, 90.0, 180.0, 270.0] {
                    let offset = rot * Quat::from_rotation_y(angle_deg.to_radians()) * (Vec3::X * r);
                    gizmos.line(bot + offset, top + offset, color);
                }
            }
            ColliderShape::Plane => {
                // Draw a 5×5 grid to represent the infinite half-space floor.
                let right  = rot * Vec3::X;
                let fwd    = rot * Vec3::Z;
                let normal = rot * Vec3::Y;
                let extent = 2.5_f32;
                let steps  = 5;
                for i in -steps..=steps {
                    let f = i as f32 * (extent / steps as f32);
                    gizmos.line(pos + right * f - fwd * extent, pos + right * f + fwd * extent, color);
                    gizmos.line(pos + fwd   * f - right * extent, pos + fwd * f + right * extent, color);
                }
                // Normal arrow shows which side is solid
                gizmos.arrow(pos, pos + normal * 0.5, color);
            }
        }
    }
}

enum ColliderShape {
    Cube([f32; 3]),
    Sphere(f32),
    Cylinder(f32, f32),
    Plane,
}
