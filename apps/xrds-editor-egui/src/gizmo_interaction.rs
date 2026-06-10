//! Gizmo interaction: viewport object picking + axis drag.
//!
//! Every frame this system:
//!  1. Checks if egui is consuming the pointer and bails out if so.
//!  2. Computes the cursor position in screen pixels from egui.
//!  3. In Translate mode: checks arrow-tip proximity for hover.
//!     In Rotate mode: samples ring points for hover.
//!  4. On left-click with an axis highlighted: starts an axis drag.
//!  5. While dragging (Translate): projects mouse delta onto the screen-space
//!     axis and accumulates a world-space offset → writes `pending_translation`.
//!     While dragging (Rotate): maps mouse delta to angle via screen-space ring
//!     radius → writes `pending_rotation`.
//!  6. On release: commits the final transform to the session.

use xrds::editor::{
    ButtonInput, Camera, EguiContexts, GlobalTransform, KeyCode, MessageReader, MouseButton,
    MouseMotion, Query, Quat, Res, ResMut, Vec2, Vec3, With,
};
use xrds::scene_graph::XrdsSceneTransform;

use crate::camera::EditorCameraMarker;
use crate::gizmo::gizmo_scale;
use crate::state::{EditorSession, EditorState, GizmoAxis, GizmoDrag, GizmoMode};

const GIZMO_HIT_PX: f32 = 20.0; // pixels within which a handle counts as a hit
const RING_SAMPLES: usize = 32;  // points sampled around each ring for hit-testing

pub fn gizmo_interaction_system(
    mut editor_state: ResMut<EditorState>,
    mut session: ResMut<EditorSession>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut egui_ctx: EguiContexts,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
) {
    // ── Egui guard ────────────────────────────────────────────────────────────
    let egui_wants = egui_ctx
        .ctx_mut()
        .map(|c| c.wants_pointer_input())
        .unwrap_or(false);
    if egui_wants {
        editor_state.gizmo_hover = None;
        return;
    }

    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    // Cursor position in screen pixels (from egui).
    let cursor: Option<Vec2> = egui_ctx
        .ctx_mut()
        .ok()
        .and_then(|c| c.pointer_latest_pos())
        .map(|p| Vec2::new(p.x, p.y));

    // Accumulate mouse motion delta this frame.
    let mut delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        delta += Vec2::new(ev.delta.x, ev.delta.y);
    }

    let gizmo_mode = editor_state.gizmo_mode;

    // ── Ongoing gizmo drag ────────────────────────────────────────────────────
    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(drag) = editor_state.gizmo_drag.clone() {
            if delta != Vec2::ZERO {
                match gizmo_mode {
                    GizmoMode::Translate => {
                        let ctrl = keyboard.pressed(KeyCode::ControlLeft)
                            || keyboard.pressed(KeyCode::ControlRight);
                        translate_drag(&mut editor_state, &drag, delta, camera, cam_gt, ctrl);
                    }
                    GizmoMode::Rotate => {
                        rotate_drag(&mut editor_state, &drag, delta, camera, cam_gt);
                    }
                    GizmoMode::Scale => {
                        let shift = keyboard.pressed(KeyCode::ShiftLeft)
                            || keyboard.pressed(KeyCode::ShiftRight);
                        scale_drag(&mut editor_state, &drag, delta, camera, cam_gt, shift);
                    }
                }
            }
            return; // skip hover and picking while dragging
        }
    }

    // ── Release: commit drag to session ──────────────────────────────────────
    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some(drag) = editor_state.gizmo_drag.take() {
            match gizmo_mode {
                GizmoMode::Translate => {
                    // Commit every pending translation from the drag.
                    let to_commit: Vec<_> = editor_state.pending_translations
                        .iter()
                        .filter(|(id, _)| drag.all_origins.iter().any(|(oid, _)| oid == id))
                        .map(|&(id, translation)| (id, translation))
                        .collect();
                    let _ = session.session.edit(|doc| {
                        for (id, translation) in &to_commit {
                            if let Some(n) = doc.node_mut(*id) {
                                n.transform.translation = *translation;
                            }
                        }
                    });
                    editor_state.clear_pending_translations();
                }
                GizmoMode::Rotate => {
                    let to_commit: Vec<_> = editor_state.pending_rotations
                        .iter()
                        .filter(|(id, _)| drag.all_origins_rotation.iter().any(|(oid, _)| oid == id))
                        .map(|&(id, rotation)| (id, rotation))
                        .collect();
                    let _ = session.session.edit(|doc| {
                        for (id, rotation) in &to_commit {
                            if let Some(n) = doc.node_mut(*id) {
                                n.transform.rotation_quat_xyzw = *rotation;
                            }
                        }
                    });
                    editor_state.clear_pending_rotations();
                }
                GizmoMode::Scale => {
                    if let Some((id, scale)) = editor_state.pending_scale {
                        if id == drag.node_id {
                            let current =
                                session.document().node(id).map(|n| n.transform.clone());
                            if let Some(t) = current {
                                let _ = session.session.set_node_transform(
                                    id,
                                    XrdsSceneTransform {
                                        translation: t.translation,
                                        rotation_quat_xyzw: t.rotation_quat_xyzw,
                                        scale,
                                    },
                                );
                            }
                            editor_state.pending_scale = None;
                        }
                    }
                }
            }
        }
        return;
    }

    // ── Gizmo axis hover ─────────────────────────────────────────────────────
    editor_state.gizmo_hover = None;
    if let (Some(cursor), Some(primary_id)) = (cursor, editor_state.selection.primary()) {
        let origin = gizmo_centroid(&editor_state, &session);
        let cam_pos = cam_gt.translation();
        let scale = gizmo_scale(cam_pos, origin);

        match gizmo_mode {
            GizmoMode::Translate | GizmoMode::Scale => {
                if let Ok(o_s) = camera.world_to_viewport(cam_gt, origin) {
                    let o_v2 = Vec2::new(o_s.x, o_s.y);
                    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
                        let tip = origin + axis_dir(axis) * scale;
                        if let Ok(t_s) = camera.world_to_viewport(cam_gt, tip) {
                            let t_v2 = Vec2::new(t_s.x, t_s.y);
                            if seg_dist(cursor, o_v2, t_v2) < GIZMO_HIT_PX {
                                editor_state.gizmo_hover = Some(axis);
                                break;
                            }
                        }
                    }
                }
            }
            GizmoMode::Rotate => {
                for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
                    if ring_hit(cursor, origin, axis_dir(axis), scale, camera, cam_gt) {
                        editor_state.gizmo_hover = Some(axis);
                        break;
                    }
                }
            }
        }
        let _ = primary_id; // used implicitly via selection
    }

    // ── Left-click handling ───────────────────────────────────────────────────
    if mouse_buttons.just_pressed(MouseButton::Left) {
        let Some(_) = cursor else {
            return;
        };

        // Priority 1: start a gizmo drag if an axis is hovered.
        if let (Some(axis), Some(primary_id)) = (editor_state.gizmo_hover, editor_state.selection.primary()) {
            let centroid = gizmo_centroid(&editor_state, &session);
            let (origin_rotation, origin_scale) = session
                .document()
                .node(primary_id)
                .map(|n| (n.transform.rotation_quat_xyzw, n.transform.scale))
                .unwrap_or(([0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]));
            // Collect start translations for all selected nodes (for multi-translate).
            let all_origins: Vec<(xrds::scene_graph::XrdsSceneNodeId, [f32; 3])> =
                editor_state.selection.ids().iter()
                    .filter_map(|&id| {
                        let t = editor_state.pending_translation_for(id)
                            .or_else(|| session.document().node(id).map(|n| n.transform.translation))?;
                        Some((id, t))
                    })
                    .collect();
            // Collect start rotations for all selected nodes (for multi-rotate).
            let all_origins_rotation: Vec<(xrds::scene_graph::XrdsSceneNodeId, [f32; 4])> =
                editor_state.selection.ids().iter()
                    .filter_map(|&id| {
                        let r = editor_state.pending_rotation_for(id)
                            .or_else(|| session.document().node(id).map(|n| n.transform.rotation_quat_xyzw))?;
                        Some((id, r))
                    })
                    .collect();
            editor_state.gizmo_drag = Some(GizmoDrag {
                node_id: primary_id,
                axis,
                origin: centroid.to_array(),
                origin_rotation,
                origin_scale,
                all_origins,
                all_origins_rotation,
                accumulated: 0.0,
            });
            return;
        }

        // Object picking is handled by `viewport_selection_system` via Bevy's
        // mesh raycasting (`Pointer<Click>` events).  Nothing to do here when no
        // axis handle is hovered.
    }
}

// ── Drag helpers ──────────────────────────────────────────────────────────────

fn translate_drag(
    editor_state: &mut EditorState,
    drag: &GizmoDrag,
    delta: Vec2,
    camera: &Camera,
    cam_gt: &GlobalTransform,
    snap: bool,
) {
    // Always project from drag.origin (fixed) so the screen-space axis direction
    // stays stable for the full duration of the drag, avoiding drift as the object
    // moves away from its starting position.
    let drag_start = Vec3::from_array(drag.origin);
    let world_axis = axis_dir(drag.axis);
    let scale = gizmo_scale(cam_gt.translation(), drag_start);
    let tip = drag_start + world_axis * scale;

    if let (Ok(o_s), Ok(t_s)) = (
        camera.world_to_viewport(cam_gt, drag_start),
        camera.world_to_viewport(cam_gt, tip),
    ) {
        let screen_axis = Vec2::new(t_s.x - o_s.x, t_s.y - o_s.y);
        let len = screen_axis.length();
        if len > 0.5 {
            let dir = screen_axis / len;
            let px = delta.dot(dir);
            let world_per_px = scale / len;
            let new_acc = drag.accumulated + px * world_per_px;

            let step = editor_state.snap_step;
            // Apply the same accumulated delta to every selected node.
            let new_pending: Vec<_> = drag.all_origins.iter().map(|&(id, [ox, oy, oz])| {
                let mut t = match drag.axis {
                    GizmoAxis::X => [ox + new_acc, oy, oz],
                    GizmoAxis::Y => [ox, oy + new_acc, oz],
                    GizmoAxis::Z => [ox, oy, oz + new_acc],
                };
                if snap {
                    t = t.map(|v| (v / step).round() * step);
                }
                (id, t)
            }).collect();
            editor_state.pending_translations = new_pending;
            if let Some(d) = editor_state.gizmo_drag.as_mut() {
                d.accumulated = new_acc;
            }
        }
    }
}

fn rotate_drag(
    editor_state: &mut EditorState,
    drag: &GizmoDrag,
    delta: Vec2,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) {
    let center = Vec3::from_array(drag.origin);
    let world_axis = axis_dir(drag.axis);

    // Project a reference tangent point to compute the screen-space ring radius.
    let scale = gizmo_scale(cam_gt.translation(), center);
    let (t1, _) = tangent_basis(world_axis);
    let tangent_world = center + t1 * scale;

    if let (Ok(c_s), Ok(t_s)) = (
        camera.world_to_viewport(cam_gt, center),
        camera.world_to_viewport(cam_gt, tangent_world),
    ) {
        let c_v2 = Vec2::new(c_s.x, c_s.y);
        let t_v2 = Vec2::new(t_s.x, t_s.y);
        let screen_radius = c_v2.distance(t_v2);

        if screen_radius > 0.5 {
            // Map horizontal mouse delta to angle: dragging one circumference worth
            // of pixels (2π * screen_radius) should produce 2π radians of rotation.
            let angle_delta = delta.x / screen_radius;
            let new_acc = drag.accumulated + angle_delta;

            // Apply the same accumulated angle to every selected node's origin rotation.
            let new_pending: Vec<_> = drag.all_origins_rotation.iter().map(|&(id, [ox, oy, oz, ow])| {
                let base = Quat::from_xyzw(ox, oy, oz, ow);
                let rot = Quat::from_axis_angle(world_axis, new_acc) * base;
                (id, [rot.x, rot.y, rot.z, rot.w])
            }).collect();
            editor_state.pending_rotations = new_pending;
            if let Some(d) = editor_state.gizmo_drag.as_mut() {
                d.accumulated = new_acc;
            }
        }
    }
}

fn scale_drag(
    editor_state: &mut EditorState,
    drag: &GizmoDrag,
    delta: Vec2,
    camera: &Camera,
    cam_gt: &GlobalTransform,
    uniform: bool,
) {
    let drag_start = Vec3::from_array(drag.origin);
    let world_axis = axis_dir(drag.axis);
    let gizmo_sz = gizmo_scale(cam_gt.translation(), drag_start);
    let tip = drag_start + world_axis * gizmo_sz;

    if let (Ok(o_s), Ok(t_s)) = (
        camera.world_to_viewport(cam_gt, drag_start),
        camera.world_to_viewport(cam_gt, tip),
    ) {
        let screen_axis = Vec2::new(t_s.x - o_s.x, t_s.y - o_s.y);
        let len = screen_axis.length();
        if len > 0.5 {
            let dir = screen_axis / len;
            let px = delta.dot(dir);
            let new_acc = drag.accumulated + px / len;

            let [sx, sy, sz] = drag.origin_scale;
            let factor = (1.0 + new_acc).max(0.001);
            let s = if uniform {
                [sx * factor, sy * factor, sz * factor]
            } else {
                match drag.axis {
                    GizmoAxis::X => [sx * factor, sy, sz],
                    GizmoAxis::Y => [sx, sy * factor, sz],
                    GizmoAxis::Z => [sx, sy, sz * factor],
                }
            };
            editor_state.pending_scale = Some((drag.node_id, s));
            if let Some(d) = editor_state.gizmo_drag.as_mut() {
                d.accumulated = new_acc;
            }
        }
    }
}

// ── General helpers ───────────────────────────────────────────────────────────

fn axis_dir(axis: GizmoAxis) -> Vec3 {
    match axis {
        GizmoAxis::X => Vec3::X,
        GizmoAxis::Y => Vec3::Y,
        GizmoAxis::Z => Vec3::Z,
    }
}

/// Two orthonormal tangent vectors spanning the plane perpendicular to `normal`.
fn tangent_basis(normal: Vec3) -> (Vec3, Vec3) {
    let up = if normal.dot(Vec3::Y).abs() < 0.9 { Vec3::Y } else { Vec3::Z };
    let t1 = normal.cross(up).normalize();
    let t2 = normal.cross(t1);
    (t1, t2)
}

/// Returns true if the cursor is within `GIZMO_HIT_PX` of any sampled point on
/// the ring defined by `center`, `normal`, and `radius`.
fn ring_hit(
    cursor: Vec2,
    center: Vec3,
    normal: Vec3,
    radius: f32,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) -> bool {
    let (t1, t2) = tangent_basis(normal);
    for i in 0..RING_SAMPLES {
        let angle = (i as f32) * std::f32::consts::TAU / RING_SAMPLES as f32;
        let p = center + radius * (t1 * angle.cos() + t2 * angle.sin());
        if let Ok(s) = camera.world_to_viewport(cam_gt, p) {
            if Vec2::new(s.x, s.y).distance(cursor) < GIZMO_HIT_PX {
                return true;
            }
        }
    }
    false
}

/// Current world-space origin of the selected node (live pending or document).
fn selected_origin(
    state: &EditorState,
    id: xrds::scene_graph::XrdsSceneNodeId,
    session: &EditorSession,
) -> Vec3 {
    let live = state.pending_translation_for(id);
    let fallback = session
        .document()
        .node(id)
        .map(|n| n.transform.translation)
        .unwrap_or([0.0, 0.0, 0.0]);
    Vec3::from_array(live.unwrap_or(fallback))
}

/// World-space centroid of all currently selected nodes.
/// Falls back to the primary node's position for single selection.
fn gizmo_centroid(state: &EditorState, session: &EditorSession) -> Vec3 {
    let ids = state.selection.ids();
    if ids.is_empty() {
        return Vec3::ZERO;
    }
    let sum: Vec3 = ids.iter().map(|&id| {
        let t = state.pending_translation_for(id)
            .or_else(|| session.document().node(id).map(|n| n.transform.translation))
            .unwrap_or([0.0, 0.0, 0.0]);
        Vec3::from_array(t)
    }).fold(Vec3::ZERO, |a, b| a + b);
    sum / ids.len() as f32
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.dot(ab);
    if len_sq < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}
