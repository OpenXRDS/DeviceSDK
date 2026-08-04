use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use xrds_runtime::XrdsIdIndex;
use xrds_runtime::sdk::XrdsId;
use xrds_scene_graph::XrdsSceneNodePayload;

use crate::bridge::EditorCommand;
use crate::bevy_bridge::BevyBridgeResource;
use crate::editor_state::{EditorSession, EditorState};
use crate::viewport_camera::EditorCameraMarker;

/// Delete key in the Bevy viewport window removes the selected object(s).
/// Pushes DeleteSelection into the bridge queue so the drain system processes it.
pub fn viewport_delete_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    state:    Res<EditorState>,
    bridge:   Res<BevyBridgeResource>,
) {
    if state.selection.is_empty() { return; }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        bridge.0.inbound.lock().unwrap().push_back(EditorCommand::DeleteSelection);
    }
}

pub fn viewport_ray_selection(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard:      Res<ButtonInput<KeyCode>>,
    windows:       Query<&Window, With<PrimaryWindow>>,
    camera_q:      Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
    session:       Res<EditorSession>,
    id_index:      Res<XrdsIdIndex>,
    global_tf_q:   Query<&GlobalTransform>,
    mut state:     ResMut<EditorState>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) { return; }
    if state.gizmo_hover.is_some() || state.gizmo_drag.is_some() { return; }
    // `apply_camera_selection_system` deactivates the editor camera during
    // play mode (the player pawn camera renders instead), but its Transform
    // is left where it was — raycasting through it here would silently pick
    // against a camera pose that no longer matches what's on screen. Same
    // guard `orbit_camera_system` already applies for the same reason.
    if state.is_playing { return; }

    let Ok((camera, cam_gt)) = camera_q.single() else { return; };
    let Ok(window) = windows.single() else { return; };
    let Some(cursor) = window.cursor_position() else { return; };

    let Ok(ray) = camera.viewport_to_world(cam_gt, cursor) else { return; };
    let ray_origin = ray.origin;
    let ray_dir    = ray.direction.as_vec3();

    // Two-pass: prefer non-plane hits so ground planes don't block object selection.
    let mut best_t    = f32::MAX;
    let mut best_id   = None;
    let mut plane_t   = f32::MAX;
    let mut plane_id  = None;

    let doc = session.0.document();
    for node in doc.nodes.iter() {
        // Use the entity's GlobalTransform (world-space) for hit testing.
        // Falling back to local transform from the document only when the entity
        // hasn't been spawned yet (e.g. during the frame it was just added).
        let center = id_index.entity_of(XrdsId::from(node.id))
            .and_then(|e| global_tf_q.get(e).ok())
            .map(|gt| gt.translation())
            .unwrap_or_else(|| {
                let local = state.pending_translation_for(node.id)
                    .unwrap_or(node.transform.translation);
                Vec3::from_array(local)
            });
        let [qx, qy, qz, qw] = node.transform.rotation_quat_xyzw;
        let rotation = Quat::from_xyzw(qx, qy, qz, qw);

        if let XrdsSceneNodePayload::Plane3D(p) = &node.payload {
            // Exact ray-plane intersection — much more accurate than sphere approx.
            if let Some(t) = ray_plane(ray_origin, ray_dir, center, rotation, p.size, node.transform.scale) {
                if t > 0.001 && t < plane_t { plane_t = t; plane_id = Some(node.id); }
            }
        } else {
            let radius = pick_radius(&node.payload, &node.transform.scale);
            if let Some(t) = ray_sphere(ray_origin, ray_dir, center, radius) {
                if t > 0.001 && t < best_t { best_t = t; best_id = Some(node.id); }
            }
        }
    }
    // Only fall back to plane if nothing else was hit.
    let best_id = best_id.or(plane_id);

    let shift = keyboard.pressed(KeyCode::ShiftLeft)   || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl  = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    match best_id {
        Some(id) => {
            if ctrl       { state.selection.toggle(id); }
            else if shift { state.selection.add(id); }
            else          { state.selection.set_single(id); }
        }
        None => {
            if !shift && !ctrl {
                state.selection.clear();
                state.clear_pending_translations();
            }
        }
    }
}

fn pick_radius(payload: &XrdsSceneNodePayload, scale: &[f32; 3]) -> f32 {
    let s = scale[0].max(scale[1]).max(scale[2]);
    match payload {
        XrdsSceneNodePayload::Sphere(sp)  => sp.radius * s,
        XrdsSceneNodePayload::Cube(c)     => Vec3::from_array(c.size).length() * 0.5 * s,
        XrdsSceneNodePayload::Cylinder(c) => c.radius.max(c.height * 0.5) * s,
        // Plane3D is handled by ray_plane() — not via bounding sphere
        XrdsSceneNodePayload::PointLight(_) | XrdsSceneNodePayload::SpotLight(_)
        | XrdsSceneNodePayload::DirectionalLight(_) | XrdsSceneNodePayload::AmbientLight(_) => 0.35 * s,
        XrdsSceneNodePayload::Camera(_) => 0.4 * s,
        XrdsSceneNodePayload::Empty     => 0.25 * s,
        _                               => 0.5 * s,
    }
}

/// Exact ray-plane intersection for a Plane3D node.
/// Returns `t` along the ray where it hits the finite plane rectangle.
fn ray_plane(
    origin: Vec3,
    dir: Vec3,
    center: Vec3,
    rotation: Quat,
    size: [f32; 2],
    scale: [f32; 3],
) -> Option<f32> {
    let normal = rotation * Vec3::Y;
    let denom = dir.dot(normal);
    if denom.abs() < 1e-6 { return None; }   // ray parallel to plane
    let t = (center - origin).dot(normal) / denom;
    if t < 0.001 { return None; }
    // Project hit point into local plane space to check bounds.
    let hit_world = origin + dir * t;
    let local = rotation.inverse() * (hit_world - center);
    let half_w = size[0] * 0.5 * scale[0];
    let half_d = size[1] * 0.5 * scale[2];
    if local.x.abs() <= half_w && local.z.abs() <= half_d { Some(t) } else { None }
}

fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 { return None; }
    let t = -b - disc.sqrt();
    if t > 0.0 { Some(t) } else { Some(-b + disc.sqrt()).filter(|&t2| t2 > 0.0) }
}
