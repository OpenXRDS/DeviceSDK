//! Transform gizmo: draws world-space axis handles on the selected scene node.
//!
//! The same `draw_axes` helper is also used by the viewport orientation widget
//! (see `panels/viewport.rs`) which projects the same three directions into 2D.

use bevy::gizmos::config::GizmoConfigGroup;
use bevy::reflect::Reflect;

use bevy_mod_outline::OutlineVolume;
use xrds::editor::{
    Camera, Children, Color, Commands, EguiContexts, Entity, GlobalTransform, Gizmos, Isometry3d,
    Local, Mesh3d, Query, Quat, Res, Transform, Vec3, With, Without, XrdsIdIndex,
};

/// Separate gizmo config group for the floor grid so it uses the default depth
/// test instead of the always-on-top `depth_bias = -1.0` of `DefaultGizmoConfigGroup`.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct GridGizmoGroup;
use xrds::scene_graph::{
    XrdsInteractionZoneShape, XrdsSceneCameraProjection, XrdsSceneNodePayload,
    XrdsSceneTextAlignment,
};

use crate::camera::EditorCameraMarker;
use crate::state::{EditorSession, EditorState, GizmoAxis, GizmoMode};

/// Returns the world-space gizmo size so it appears constant on screen.
///
/// Both `gizmo.rs` (drawing) and `gizmo_interaction.rs` (hit detection) call
/// this with the same inputs so the drawn arrows and their hit targets always
/// match exactly.
pub fn gizmo_scale(cam_pos: Vec3, target: Vec3) -> f32 {
    (cam_pos.distance(target) * 0.18).max(0.4)
}

/// Draw three colour-coded arrows at `origin` oriented by `rotation`.
///
/// X → red, Y → green, Z → blue (toward viewer in Bevy's -Z forward convention).
/// This is the shared primitive reused by both the world-space gizmo system and
/// the 2D orientation indicator in the viewport corner.
pub fn draw_axes(gizmos: &mut Gizmos, origin: Vec3, rotation: Quat, length: f32) {
    let x = rotation * Vec3::X;
    let y = rotation * Vec3::Y;
    // Bevy is -Z forward; we show +Z as the "into screen" axis in the indicator
    let z = rotation * Vec3::Z;

    gizmos.arrow(
        origin,
        origin + x * length,
        xrds::editor::Color::srgb_u8(220, 50, 50),
    );
    gizmos.arrow(
        origin,
        origin + y * length,
        xrds::editor::Color::srgb_u8(50, 200, 50),
    );
    gizmos.arrow(
        origin,
        origin + z * length,
        xrds::editor::Color::srgb_u8(50, 80, 220),
    );
}

/// Bevy system — draws move handles on the currently selected scene node.
///
/// In Translate mode: three world-space axis arrows (always +X/+Y/+Z, never
/// rotated with the node — consistent with how the drag accumulates).
/// In Rotate mode: three colour-coded rings (one per world axis).
/// Both modes scale the gizmo to a constant screen size.
pub fn transform_gizmo_system(
    mut gizmos: Gizmos,
    editor_state: Res<EditorState>,
    session: Res<EditorSession>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
) {
    if editor_state.selection.is_empty() {
        return;
    }

    let doc = session.document();

    // Gizmo origin = centroid of all selected nodes' live positions.
    let ids = editor_state.selection.ids();
    let sum: Vec3 = ids.iter().map(|&id| {
        let t = editor_state.pending_translation_for(id)
            .or_else(|| doc.node(id).map(|n| n.transform.translation))
            .unwrap_or([0.0, 0.0, 0.0]);
        Vec3::from_array(t)
    }).fold(Vec3::ZERO, |a, b| a + b);
    let origin = sum / ids.len() as f32;

    let cam_pos = camera_q
        .single()
        .map(|(_, gt)| gt.translation())
        .unwrap_or(Vec3::ZERO);
    let scale = gizmo_scale(cam_pos, origin);

    // Camera frustum and rotation rings use the primary node's orientation.
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

/// Draw a wireframe frustum for a scene camera so it is visible in the editor viewport.
///
/// Far plane is capped at 8 world units regardless of the camera's actual far value
/// to keep the visualization compact.
fn draw_camera_frustum(
    gizmos: &mut Gizmos,
    pos: Vec3,
    rot: Quat,
    projection: &XrdsSceneCameraProjection,
) {
    const CAP_FAR: f32 = 8.0;
    const ASPECT: f32 = 16.0 / 9.0;
    let color = Color::srgb(0.9, 0.85, 0.2);

    let (fov_rad, near, capped_far, ortho_scale) = match *projection {
        XrdsSceneCameraProjection::Perspective { fov_deg, near, far, .. } => {
            let cap = far.map_or(CAP_FAR, |f| f.min(CAP_FAR));
            (fov_deg.to_radians(), near, cap, None)
        }
        XrdsSceneCameraProjection::Orthographic { scale, near, far, .. } => {
            (0.0_f32, near, far.min(CAP_FAR), Some(scale))
        }
    };

    // Bevy cameras look along -Z; near/far planes sit at z = -near and z = -far.
    let corners = |dist: f32| -> [Vec3; 4] {
        let (hw, hh) = if let Some(scale) = ortho_scale {
            (scale * ASPECT * 0.5, scale * 0.5)
        } else {
            let h = dist * (fov_rad * 0.5).tan();
            (h * ASPECT, h)
        };
        [
            pos + rot * Vec3::new(-hw, -hh, -dist),
            pos + rot * Vec3::new( hw, -hh, -dist),
            pos + rot * Vec3::new( hw,  hh, -dist),
            pos + rot * Vec3::new(-hw,  hh, -dist),
        ]
    };

    let near_c = corners(near);
    let far_c  = corners(capped_far);

    // Near rectangle
    for i in 0..4 { gizmos.line(near_c[i], near_c[(i + 1) % 4], color); }
    // Far rectangle
    for i in 0..4 { gizmos.line(far_c[i],  far_c[(i + 1) % 4],  color); }
    // Frustum edges
    for i in 0..4 { gizmos.line(near_c[i], far_c[i], color); }
    // Lines from apex to near corners (shows direction)
    for i in 0..4 { gizmos.line(pos, near_c[i], color); }
}

fn draw_rotation_rings(gizmos: &mut Gizmos, origin: Vec3, scale: f32, hover: Option<GizmoAxis>) {
    use std::f32::consts::FRAC_PI_2;

    let dim = |r: u8, g: u8, b: u8| Color::srgb_u8(r, g, b);
    let hi = Color::srgb(1.0, 1.0, 0.5);

    let cx = if hover == Some(GizmoAxis::X) { hi } else { dim(220, 50, 50) };
    let cy = if hover == Some(GizmoAxis::Y) { hi } else { dim(50, 200, 50) };
    let cz = if hover == Some(GizmoAxis::Z) { hi } else { dim(50, 80, 220) };

    gizmos.circle(Isometry3d::new(origin, Quat::from_rotation_y(FRAC_PI_2)), scale, cx);
    gizmos.circle(Isometry3d::new(origin, Quat::from_rotation_x(FRAC_PI_2)), scale, cy);
    gizmos.circle(Isometry3d::new(origin, Quat::IDENTITY), scale, cz);
}

/// Scale gizmo — same coloured axis shafts as Translate but with a small cube
/// at each tip instead of an arrow head, making the mode visually distinct.
fn draw_scale_handles(gizmos: &mut Gizmos, origin: Vec3, scale: f32, hover: Option<GizmoAxis>) {
    use crate::state::GizmoAxis;
    let hi = Color::srgb(1.0, 1.0, 0.5);
    let dim = |r: u8, g: u8, b: u8| Color::srgb_u8(r, g, b);

    let cx = if hover == Some(GizmoAxis::X) { hi } else { dim(220, 50, 50) };
    let cy = if hover == Some(GizmoAxis::Y) { hi } else { dim(50, 200, 50) };
    let cz = if hover == Some(GizmoAxis::Z) { hi } else { dim(50, 80, 220) };

    let cube_half = scale * 0.08;
    for (dir, col) in [(Vec3::X, cx), (Vec3::Y, cy), (Vec3::Z, cz)] {
        let tip = origin + dir * scale;
        // Shaft line.
        gizmos.line(origin, tip, col);
        // Small cube at the tip.
        gizmos.cuboid(
            Transform {
                translation: tip,
                scale: Vec3::splat(cube_half * 2.0),
                ..Default::default()
            },
            col,
        );
    }
}

/// Bevy system — adds/removes Bevy's built-in `Wireframe` component on the
/// mesh entities of the selected scene node so Bevy renders their edges in orange.
///
/// glTF nodes use `SceneRoot` on the XRDS entity; actual `Mesh3d` entities are
/// child entities spawned asynchronously by Bevy's scene loader. This system
/// walks the full subtree every frame while no mesh entities have been found yet,
/// so it correctly handles both instant and late-loading cases.
pub fn update_selection_outline(
    mut commands: Commands,
    editor_state: Res<EditorState>,
    id_index: Res<XrdsIdIndex>,
    children_query: Query<&Children>,
    mesh_query: Query<(), With<Mesh3d>>,
    mut prev_sel: Local<Vec<xrds::scene_graph::XrdsSceneNodeId>>,
    mut outlined: Local<Vec<Entity>>,
) {
    // Before a full reimport entities will be despawned — just drop the list.
    if editor_state.needs_full_reimport {
        outlined.clear();
        prev_sel.clear();
        return;
    }

    let current_ids: Vec<_> = editor_state.selection.ids().to_vec();

    // When the selection changes, remove outlines from previously outlined entities.
    if *prev_sel != current_ids {
        for e in outlined.drain(..) {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.remove::<OutlineVolume>();
            }
        }
        *prev_sel = current_ids.clone();
    }

    if current_ids.is_empty() {
        return;
    }

    // For each selected node: scan for mesh children not yet outlined.
    // Retried every frame until glTF children appear asynchronously.
    for &node_id in &current_ids {
        let Some(root) = id_index.entity_of(xrds::sdk::XrdsId::from(node_id)) else {
            continue;
        };
        let mut candidates = Vec::new();
        collect_mesh_entities(root, &children_query, &mesh_query, &mut candidates);
        for e in candidates {
            if !outlined.contains(&e) {
                if let Ok(mut ec) = commands.get_entity(e) {
                    ec.insert(OutlineVolume {
                        visible: true,
                        colour: Color::srgb(1.0, 0.55, 0.0),
                        width: 2.0,
                    });
                }
                outlined.push(e);
            }
        }
    }
}

// ── Light ray debug gizmos ────────────────────────────────────────────────────

/// Bevy system — draws light-shape overlays based on the two debug flags:
///
/// * `light_rays_all`      — draws for every visible light node in the scene.
/// * `light_rays_selected` — draws only for the currently selected light node.
///
/// The two modes are independent; enabling both shows all lights while the
/// selected one is still highlighted (same appearance; no double-draw because
/// "all" already includes it).
pub fn light_rays_system(
    mut gizmos: Gizmos,
    editor_state: Res<EditorState>,
    session: Res<EditorSession>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
) {
    if !editor_state.light_rays_all && !editor_state.light_rays_selected {
        return;
    }

    let cam_pos = camera_q
        .single()
        .map(|(_, gt)| gt.translation())
        .unwrap_or(Vec3::ZERO);

    let doc = session.document();
    for node in &doc.nodes {
        if !node.visible {
            continue;
        }

        let is_selected = editor_state.selection.contains(node.id);
        let should_draw = editor_state.light_rays_all
            || (editor_state.light_rays_selected && is_selected);
        if !should_draw {
            continue;
        }

        let [tx, ty, tz] = node.transform.translation;
        let [qx, qy, qz, qw] = node.transform.rotation_quat_xyzw;
        let pos = Vec3::new(tx, ty, tz);
        let rot = Quat::from_xyzw(qx, qy, qz, qw);

        match &node.payload {
            XrdsSceneNodePayload::PointLight(l) => {
                draw_point_light_rays(&mut gizmos, pos, l.range, l.color);
            }
            XrdsSceneNodePayload::SpotLight(l) => {
                draw_spot_light_rays(
                    &mut gizmos, pos, rot, l.range,
                    l.inner_angle, l.outer_angle, l.color,
                );
            }
            XrdsSceneNodePayload::DirectionalLight(l) => {
                let scale = gizmo_scale(cam_pos, pos);
                draw_dir_light_rays(&mut gizmos, pos, rot, l.color, scale);
            }
            _ => {}
        }
    }
}

/// PointLight — 6 axis-aligned rays up to `range`, plus 3 circles marking the range sphere.
fn draw_point_light_rays(gizmos: &mut Gizmos, pos: Vec3, range: f32, color: [f32; 4]) {
    use std::f32::consts::FRAC_PI_2;
    let [r, g, b, _] = color;
    let ray = Color::srgba(r, g, b, 0.7);
    let ring = Color::srgba(r, g, b, 0.3);

    for dir in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
        gizmos.line(pos, pos + dir * range, ray);
    }
    // Three circles that outline the range sphere.
    gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_x(FRAC_PI_2)), range, ring);
    gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_y(FRAC_PI_2)), range, ring);
    gizmos.circle(Isometry3d::new(pos, Quat::IDENTITY), range, ring);
}

/// SpotLight — axis ray, outer-cone edge lines, outer and inner cone circles.
///
/// `inner_angle` / `outer_angle` are half-angles from the cone axis (radians).
fn draw_spot_light_rays(
    gizmos: &mut Gizmos,
    pos: Vec3,
    rot: Quat,
    range: f32,
    inner_angle: f32,
    outer_angle: f32,
    color: [f32; 4],
) {
    let [r, g, b, _] = color;
    let col_axis  = Color::srgba(r, g, b, 0.85);
    let col_outer = Color::srgba(r, g, b, 0.65);
    let col_inner = Color::srgba(r, g, b, 0.30);

    // Bevy spotlights point along local -Z.
    let forward = rot * Vec3::NEG_Z;
    let apex = pos;
    let tip  = pos + forward * range;

    // Central axis ray.
    gizmos.line(apex, tip, col_axis);

    // Two vectors perpendicular to forward for sampling the cone rim.
    let (t1, t2) = light_tangent_basis(forward);

    // Draw outer cone: 8 edge lines + closing circle at range.
    let outer_r = range * outer_angle.tan();
    let circle_rot = circle_normal_rotation(forward);
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let rim = tip + outer_r * (t1 * angle.cos() + t2 * angle.sin());
        gizmos.line(apex, rim, col_outer);
    }
    gizmos.circle(Isometry3d::new(tip, circle_rot), outer_r, col_outer);

    // Draw inner cone circle at the same distance (smaller, dimmer).
    let inner_r = range * inner_angle.tan();
    gizmos.circle(Isometry3d::new(tip, circle_rot), inner_r, col_inner);
}

/// DirectionalLight — sun icon: disk perpendicular to the light direction, 8 radial
/// spokes from the disk edge, and a yellow arrow showing the light direction.
/// Size is camera-distance-aware via `scale`.
fn draw_dir_light_rays(gizmos: &mut Gizmos, pos: Vec3, rot: Quat, color: [f32; 4], scale: f32) {
    let [r, g, b, _] = color;
    let col_disk  = Color::srgba(r, g, b, 0.75);
    let col_spoke = Color::srgba(r, g, b, 0.55);
    let col_arrow = Color::srgba(1.0, 0.85, 0.2, 0.90); // yellow direction arrow

    // Bevy directional lights illuminate along local -Z.
    let forward = rot * Vec3::NEG_Z;

    let disk_r    = scale * 0.45;
    let spoke_len = scale * 0.30;
    let disk_rot  = circle_normal_rotation(forward);

    // Sun disk.
    gizmos.circle(Isometry3d::new(pos, disk_rot), disk_r, col_disk);

    // 8 spokes radiating outward from the disk edge.
    let (t1, t2) = light_tangent_basis(forward);
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let dir   = t1 * angle.cos() + t2 * angle.sin();
        let start = pos + dir * disk_r;
        let end   = pos + dir * (disk_r + spoke_len);
        gizmos.line(start, end, col_spoke);
    }

    // Direction arrow — from behind the sun toward the scene (yellow).
    let arrow_start = pos - forward * scale * 1.1;
    let arrow_end   = pos - forward * scale * 0.15;
    gizmos.arrow(arrow_start, arrow_end, col_arrow);
}

/// Returns two unit vectors orthogonal to `dir` that span its perpendicular plane.
fn light_tangent_basis(dir: Vec3) -> (Vec3, Vec3) {
    let up = if dir.dot(Vec3::Y).abs() < 0.9 { Vec3::Y } else { Vec3::Z };
    let t1 = dir.cross(up).normalize();
    let t2 = dir.cross(t1);
    (t1, t2)
}

/// Rotation that aligns the circle's local Z axis with `normal` (the desired circle normal).
/// Handles the degenerate case where normal ≈ -Z.
fn circle_normal_rotation(normal: Vec3) -> Quat {
    if (normal + Vec3::Z).length_squared() < 1e-5 {
        Quat::from_rotation_x(std::f32::consts::PI)
    } else {
        Quat::from_rotation_arc(Vec3::Z, normal)
    }
}

/// Bevy system — draws a floor grid on the XZ plane (Y = 0) when `show_grid` is enabled.
///
/// 1 m minor lines every metre, brighter major lines every 5 m.
/// Origin row/column tinted red (X) and blue (Z) to help orient the scene.
pub fn floor_grid_system(mut gizmos: Gizmos<GridGizmoGroup>, editor_state: Res<EditorState>) {
    if !editor_state.show_grid {
        return;
    }

    const EXTENT: i32 = 10; // grid runs from -EXTENT to +EXTENT metres
    let ef = EXTENT as f32;

    for i in -EXTENT..=EXTENT {
        let fi = i as f32;

        let (col_x, col_z) = if i == 0 {
            // Origin lines — faint red for X axis, faint blue for Z axis.
            (
                Color::srgba(0.7, 0.2, 0.2, 0.55),
                Color::srgba(0.2, 0.2, 0.7, 0.55),
            )
        } else if i % 5 == 0 {
            let c = Color::srgba(0.55, 0.55, 0.55, 0.45);
            (c, c)
        } else {
            let c = Color::srgba(0.4, 0.4, 0.4, 0.22);
            (c, c)
        };

        // Lines parallel to Z (vary X position)
        gizmos.line(Vec3::new(fi, 0.0, -ef), Vec3::new(fi, 0.0, ef), col_z);
        // Lines parallel to X (vary Z position)
        gizmos.line(Vec3::new(-ef, 0.0, fi), Vec3::new(ef, 0.0, fi), col_x);
    }
}

/// Draw wireframe outlines for all InteractionZone nodes in the scene.
/// Selected zones are drawn brighter; all others are drawn faintly.
pub fn interaction_zone_gizmo_system(
    mut gizmos: Gizmos,
    editor_state: Res<EditorState>,
    session: Res<EditorSession>,
) {
    let doc = session.document();
    for node in &doc.nodes {
        let XrdsSceneNodePayload::InteractionZone(zone) = &node.payload else { continue; };
        let is_selected = editor_state.selection.contains(node.id);
        let color = if is_selected {
            Color::srgba(0.2, 0.9, 0.4, 0.9)
        } else {
            Color::srgba(0.2, 0.7, 0.3, 0.35)
        };

        let pos = Vec3::from_array(node.transform.translation);
        let [qx, qy, qz, qw] = node.transform.rotation_quat_xyzw;
        let rot = Quat::from_xyzw(qx, qy, qz, qw);

        match zone.shape {
            XrdsInteractionZoneShape::Box { half_extents } => {
                let size = Vec3::from_array(half_extents) * 2.0;
                gizmos.cuboid(
                    Transform::from_translation(pos).with_rotation(rot).with_scale(size),
                    color,
                );
            }
            XrdsInteractionZoneShape::Sphere { radius } => {
                gizmos.sphere(
                    Isometry3d::new(pos, rot),
                    radius,
                    color,
                );
            }
        }
    }
}

fn collect_mesh_entities(
    entity: Entity,
    children_query: &Query<&Children>,
    mesh_query: &Query<(), With<Mesh3d>>,
    result: &mut Vec<Entity>,
) {
    if mesh_query.contains(entity) {
        result.push(entity);
    }
    if let Ok(children) = children_query.get(entity) {
        for &child in children.iter() {
            collect_mesh_entities(child, children_query, mesh_query, result);
        }
    }
}

