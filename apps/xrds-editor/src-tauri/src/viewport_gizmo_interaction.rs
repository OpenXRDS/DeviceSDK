use bevy::prelude::*;
use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseMotion;
use bevy::window::PrimaryWindow;
use xrds_scene_graph::XrdsSceneTransform;

use xrds_runtime::XrdsIdIndex;
use xrds_runtime::sdk::XrdsId;
use crate::editor_state::{EditorSession, EditorState, GizmoAxis, GizmoDrag, GizmoMode};
use crate::viewport_camera::EditorCameraMarker;
use crate::viewport_gizmo::{axis_dir, gizmo_scale};

const GIZMO_HIT_PX: f32 = 20.0;
const RING_SAMPLES: usize = 32;

pub fn gizmo_interaction_system(
    mut state:        ResMut<EditorState>,
    mut session:      ResMut<EditorSession>,
    mouse_buttons:    Res<ButtonInput<MouseButton>>,
    keyboard:         Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    windows:          Query<&Window, With<PrimaryWindow>>,
    camera_q:         Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
    id_index:         Res<XrdsIdIndex>,
    global_tf_q:      Query<&GlobalTransform>,
) {
    let Ok((camera, cam_gt)) = camera_q.single() else { return; };
    let cursor: Option<Vec2> = windows.single().ok().and_then(|w| w.cursor_position());

    let mut delta = Vec2::ZERO;
    for ev in mouse_motion.read() { delta += Vec2::new(ev.delta.x, ev.delta.y); }

    let gizmo_mode = state.gizmo_mode;

    // ── Ongoing drag ─────────────────────────────────────────────────────────
    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(drag) = state.gizmo_drag.clone() {
            if delta != Vec2::ZERO {
                match gizmo_mode {
                    GizmoMode::Translate => {
                        let snap = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
                        translate_drag(&mut state, &drag, delta, camera, cam_gt, snap);
                    }
                    GizmoMode::Rotate => rotate_drag(&mut state, &drag, delta, camera, cam_gt),
                    GizmoMode::Scale  => {
                        let uniform = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
                        scale_drag(&mut state, &drag, delta, camera, cam_gt, uniform);
                    }
                }
            }
            return;
        }
    }

    // ── Release: commit drag to session ──────────────────────────────────────
    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some(drag) = state.gizmo_drag.take() {
            match gizmo_mode {
                GizmoMode::Translate => {
                    let to_commit: Vec<_> = state.pending_translations.iter()
                        .filter(|(id, _)| drag.all_origins.iter().any(|(oid, _)| oid == id))
                        .map(|&(id, t)| (id, t)).collect();
                    let _ = session.0.edit(|doc| {
                        for (id, world_t) in &to_commit {
                            if let Some(n) = doc.node_mut(*id) {
                                // pending_translations stores world-space positions (from GlobalTransform).
                                // Document stores local (parent-relative) space, so convert here.
                                let local_t = n.parent_id
                                    .and_then(|pid| id_index.entity_of(XrdsId::from(pid)))
                                    .and_then(|e| global_tf_q.get(e).ok())
                                    .map(|parent_gt| {
                                        parent_gt.affine().inverse()
                                            .transform_point3(Vec3::from_array(*world_t))
                                            .to_array()
                                    })
                                    .unwrap_or(*world_t);
                                n.transform.translation = local_t;
                            }
                        }
                    });
                    state.clear_pending_translations();
                }
                GizmoMode::Rotate => {
                    let to_commit: Vec<_> = state.pending_rotations.iter()
                        .filter(|(id, _)| drag.all_origins_rotation.iter().any(|(oid, _)| oid == id))
                        .map(|&(id, r)| (id, r)).collect();
                    let _ = session.0.edit(|doc| {
                        for (id, r) in &to_commit {
                            if let Some(n) = doc.node_mut(*id) { n.transform.rotation_quat_xyzw = *r; }
                        }
                    });
                    state.clear_pending_rotations();
                }
                GizmoMode::Scale => {
                    if let Some((id, scale)) = state.pending_scale.take() {
                        if id == drag.node_id {
                            if let Some(current) = session.0.document().node(id).map(|n| n.transform.clone()) {
                                let _ = session.0.edit(|doc| {
                                    if let Some(n) = doc.node_mut(id) {
                                        n.transform = XrdsSceneTransform {
                                            translation: current.translation,
                                            rotation_quat_xyzw: current.rotation_quat_xyzw,
                                            scale,
                                        };
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
        return;
    }

    // ── Hover detection ───────────────────────────────────────────────────────
    state.gizmo_hover = None;
    if let (Some(cursor), Some(primary_id)) = (cursor, state.selection.primary()) {
        let origin = gizmo_centroid(&state, &session, &id_index, &global_tf_q);
        let scale  = gizmo_scale(cam_gt.translation(), origin);

        match gizmo_mode {
            GizmoMode::Translate | GizmoMode::Scale => {
                if let Ok(o_s) = camera.world_to_viewport(cam_gt, origin) {
                    let o_v2 = Vec2::new(o_s.x, o_s.y);
                    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
                        let tip = origin + axis_dir(axis) * scale;
                        if let Ok(t_s) = camera.world_to_viewport(cam_gt, tip) {
                            if seg_dist(cursor, o_v2, Vec2::new(t_s.x, t_s.y)) < GIZMO_HIT_PX {
                                state.gizmo_hover = Some(axis); break;
                            }
                        }
                    }
                }
            }
            GizmoMode::Rotate => {
                for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
                    if ring_hit(cursor, origin, axis_dir(axis), scale, camera, cam_gt) {
                        state.gizmo_hover = Some(axis); break;
                    }
                }
            }
        }
        let _ = primary_id;
    }

    // ── Left-click: start drag ────────────────────────────────────────────────
    if mouse_buttons.just_pressed(MouseButton::Left) {
        if cursor.is_none() { return; }
        if let (Some(axis), Some(primary_id)) = (state.gizmo_hover, state.selection.primary()) {
            let centroid = gizmo_centroid(&state, &session, &id_index, &global_tf_q);
            let (origin_rotation, origin_scale) = session.0.document().node(primary_id)
                .map(|n| (n.transform.rotation_quat_xyzw, n.transform.scale))
                .unwrap_or(([0.0,0.0,0.0,1.0], [1.0,1.0,1.0]));
            let all_origins: Vec<_> = state.selection.ids().iter().filter_map(|&id| {
                // Use world-space position so drag works correctly for child nodes.
                let world = id_index.entity_of(XrdsId::from(id))
                    .and_then(|e| global_tf_q.get(e).ok())
                    .map(|gt| gt.translation().to_array())
                    .or_else(|| state.pending_translation_for(id))
                    .or_else(|| session.0.document().node(id).map(|n| n.transform.translation))?;
                Some((id, world))
            }).collect();
            let all_origins_rotation: Vec<_> = state.selection.ids().iter().filter_map(|&id| {
                let r = state.pending_rotation_for(id)
                    .or_else(|| session.0.document().node(id).map(|n| n.transform.rotation_quat_xyzw))?;
                Some((id, r))
            }).collect();
            state.gizmo_drag = Some(GizmoDrag {
                node_id: primary_id, axis, origin: centroid.to_array(),
                origin_rotation, origin_scale, all_origins, all_origins_rotation, accumulated: 0.0,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Drag helpers
// ---------------------------------------------------------------------------

fn translate_drag(state: &mut EditorState, drag: &GizmoDrag, delta: Vec2, camera: &Camera, cam_gt: &GlobalTransform, snap: bool) {
    let start = Vec3::from_array(drag.origin);
    let world_axis = axis_dir(drag.axis);
    let scale = gizmo_scale(cam_gt.translation(), start);
    let tip = start + world_axis * scale;
    if let (Ok(o_s), Ok(t_s)) = (camera.world_to_viewport(cam_gt, start), camera.world_to_viewport(cam_gt, tip)) {
        let screen_axis = Vec2::new(t_s.x - o_s.x, t_s.y - o_s.y);
        let len = screen_axis.length();
        if len > 0.5 {
            let new_acc = drag.accumulated + delta.dot(screen_axis / len) * (scale / len);
            let step = state.snap_step;
            state.pending_translations = drag.all_origins.iter().map(|&(id, [ox,oy,oz])| {
                let mut t = match drag.axis {
                    GizmoAxis::X => [ox+new_acc, oy, oz],
                    GizmoAxis::Y => [ox, oy+new_acc, oz],
                    GizmoAxis::Z => [ox, oy, oz+new_acc],
                };
                if snap { t = t.map(|v| (v/step).round()*step); }
                (id, t)
            }).collect();
            if let Some(d) = state.gizmo_drag.as_mut() { d.accumulated = new_acc; }
        }
    }
}

fn rotate_drag(state: &mut EditorState, drag: &GizmoDrag, delta: Vec2, camera: &Camera, cam_gt: &GlobalTransform) {
    let center = Vec3::from_array(drag.origin);
    let world_axis = axis_dir(drag.axis);
    let scale = gizmo_scale(cam_gt.translation(), center);
    let (t1, _) = tangent_basis(world_axis);
    if let (Ok(c_s), Ok(t_s)) = (camera.world_to_viewport(cam_gt, center), camera.world_to_viewport(cam_gt, center + t1*scale)) {
        let screen_r = Vec2::new(c_s.x, c_s.y).distance(Vec2::new(t_s.x, t_s.y));
        if screen_r > 0.5 {
            let new_acc = drag.accumulated + delta.x / screen_r;
            state.pending_rotations = drag.all_origins_rotation.iter().map(|&(id, [ox,oy,oz,ow])| {
                let rot = Quat::from_axis_angle(world_axis, new_acc) * Quat::from_xyzw(ox,oy,oz,ow);
                (id, [rot.x, rot.y, rot.z, rot.w])
            }).collect();
            if let Some(d) = state.gizmo_drag.as_mut() { d.accumulated = new_acc; }
        }
    }
}

fn scale_drag(state: &mut EditorState, drag: &GizmoDrag, delta: Vec2, camera: &Camera, cam_gt: &GlobalTransform, uniform: bool) {
    let start = Vec3::from_array(drag.origin);
    let world_axis = axis_dir(drag.axis);
    let gsz = gizmo_scale(cam_gt.translation(), start);
    let tip = start + world_axis * gsz;
    if let (Ok(o_s), Ok(t_s)) = (camera.world_to_viewport(cam_gt, start), camera.world_to_viewport(cam_gt, tip)) {
        let screen_axis = Vec2::new(t_s.x-o_s.x, t_s.y-o_s.y);
        let len = screen_axis.length();
        if len > 0.5 {
            let new_acc = drag.accumulated + delta.dot(screen_axis/len) / len;
            let [sx,sy,sz] = drag.origin_scale;
            let f = (1.0 + new_acc).max(0.001);
            let s = if uniform { [sx*f, sy*f, sz*f] } else {
                match drag.axis { GizmoAxis::X=>[sx*f,sy,sz], GizmoAxis::Y=>[sx,sy*f,sz], GizmoAxis::Z=>[sx,sy,sz*f] }
            };
            state.pending_scale = Some((drag.node_id, s));
            if let Some(d) = state.gizmo_drag.as_mut() { d.accumulated = new_acc; }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn gizmo_centroid(
    state: &EditorState,
    session: &EditorSession,
    id_index: &XrdsIdIndex,
    global_tf_q: &Query<&GlobalTransform>,
) -> Vec3 {
    let ids = state.selection.ids();
    if ids.is_empty() { return Vec3::ZERO; }
    let sum: Vec3 = ids.iter().map(|&id| {
        id_index.entity_of(XrdsId::from(id))
            .and_then(|e| global_tf_q.get(e).ok())
            .map(|gt| gt.translation())
            .unwrap_or_else(|| {
                let local = state.pending_translation_for(id)
                    .or_else(|| session.0.document().node(id).map(|n| n.transform.translation))
                    .unwrap_or([0.0,0.0,0.0]);
                Vec3::from_array(local)
            })
    }).fold(Vec3::ZERO, |a,b| a+b);
    sum / ids.len() as f32
}

fn tangent_basis(normal: Vec3) -> (Vec3, Vec3) {
    let up = if normal.dot(Vec3::Y).abs() < 0.9 { Vec3::Y } else { Vec3::Z };
    let t1 = normal.cross(up).normalize();
    (t1, normal.cross(t1))
}

fn ring_hit(cursor: Vec2, center: Vec3, normal: Vec3, radius: f32, camera: &Camera, cam_gt: &GlobalTransform) -> bool {
    let (t1, t2) = tangent_basis(normal);
    for i in 0..RING_SAMPLES {
        let a = (i as f32) * std::f32::consts::TAU / RING_SAMPLES as f32;
        let p = center + radius * (t1*a.cos() + t2*a.sin());
        if let Ok(s) = camera.world_to_viewport(cam_gt, p) {
            if Vec2::new(s.x, s.y).distance(cursor) < GIZMO_HIT_PX { return true; }
        }
    }
    false
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a; let len_sq = ab.dot(ab);
    if len_sq < 1e-6 { return p.distance(a); }
    p.distance(a + ab * ((p-a).dot(ab)/len_sq).clamp(0.0, 1.0))
}
