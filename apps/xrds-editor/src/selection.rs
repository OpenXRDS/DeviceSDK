//! Viewport selection via manual ray casting.
//!
//! `Pointer<Click>` / `EntityEvent` proved unreliable in this Bevy 0.17 setup
//! (both `event.entity` and `trigger.target()` returned entity `0v0` — the
//! window/fallback entity, not the sphere).  Manual raycasting is simpler,
//! version-independent, and works with every XRDS node type.
//!
//! Each frame on left-click this system:
//!  1. Gets the cursor position from egui.
//!  2. Converts it to a world-space `Ray3d` via `Camera::viewport_to_world`.
//!  3. Tests every document node using a bounding-sphere approximation.
//!  4. Selects the closest hit, or deselects if nothing is hit.

use crate::camera::EditorCameraMarker;
use crate::state::{EditorSession, EditorState};
use xrds::editor::Query;
use xrds::editor::{
    ButtonInput, Camera, EguiContexts, GlobalTransform, KeyCode, MouseButton, Res, ResMut, Vec2,
    Vec3, With,
};
use xrds::scene_graph::XrdsSceneNodePayload;

pub fn setup_viewport_selection(app: &mut xrds::editor::App) {
    app.add_systems(xrds::editor::Update, viewport_ray_selection);
}

fn viewport_ray_selection(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut egui_ctx: EguiContexts,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCameraMarker>>,
    session: Res<EditorSession>,
    mut editor_state: ResMut<EditorState>,
) {
    // Only on a fresh left-click press.
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    // Skip when egui is consuming the pointer (clicking on any panel).
    let egui_wants = egui_ctx
        .ctx_mut()
        .map(|c| c.wants_pointer_input())
        .unwrap_or(true);
    if egui_wants {
        return;
    }

    // Skip when a gizmo axis is about to be dragged.
    if editor_state.gizmo_hover.is_some() || editor_state.gizmo_drag.is_some() {
        return;
    }

    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    // Cursor position in screen pixels from egui.
    let Some(cursor) = egui_ctx.ctx_mut().ok().and_then(|c| c.pointer_latest_pos()) else {
        return;
    };

    // Build a world-space ray from the camera through the cursor.
    let Ok(ray) = camera.viewport_to_world(cam_gt, Vec2::new(cursor.x, cursor.y)) else {
        return;
    };

    let ray_origin = ray.origin;
    let ray_dir = ray.direction.as_vec3();

    // Test every document node and keep the closest hit.
    let mut best_t = f32::MAX;
    let mut best_id = None;

    let doc = session.document();
    for node in doc.nodes.iter() {
        // Live position (respects pending drag preview).
        let center = {
            let [x, y, z] = editor_state
                .pending_translation_for(node.id)
                .unwrap_or(node.transform.translation);
            Vec3::new(x, y, z)
        };

        let radius = pick_radius(&node.payload, &node.transform.scale);

        if let Some(t) = ray_sphere(ray_origin, ray_dir, center, radius) {
            if t > 0.001 && t < best_t {
                best_t = t;
                best_id = Some(node.id);
            }
        }
    }


    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl  = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    match best_id {
        Some(id) => {
            if ctrl {
                editor_state.selection.toggle(id);
            } else if shift {
                editor_state.selection.add(id);
            } else {
                editor_state.selection.set_single(id);
            }
        }
        None => {
            if !shift && !ctrl {
                editor_state.selection.clear();
                editor_state.clear_pending_translations();
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Estimate a bounding-sphere radius for each node type.
/// These radii are generous so clicking near (not just on) an object works.
fn pick_radius(payload: &XrdsSceneNodePayload, scale: &[f32; 3]) -> f32 {
    let s = scale[0].max(scale[1]).max(scale[2]);
    match payload {
        XrdsSceneNodePayload::Sphere(sphere) => sphere.radius * s,
        XrdsSceneNodePayload::Cube(cube) => Vec3::from_array(cube.size).length() * 0.5 * s,
        XrdsSceneNodePayload::Cylinder(cyl) => cyl.radius.max(cyl.height * 0.5) * s,
        XrdsSceneNodePayload::Plane3D(plane) => Vec2::from_array(plane.size).length() * 0.5 * s,
        XrdsSceneNodePayload::PointLight(_)
        | XrdsSceneNodePayload::SpotLight(_)
        | XrdsSceneNodePayload::DirectionalLight(_)
        | XrdsSceneNodePayload::AmbientLight(_) => 0.35 * s,
        XrdsSceneNodePayload::Camera(_) => 0.4 * s,
        XrdsSceneNodePayload::AudioClip(_) => 0.3 * s,
        XrdsSceneNodePayload::Empty => 0.25 * s,
        _ => 0.5 * s,
    }
}

/// Analytic ray–sphere intersection.  Returns the distance `t` along the ray
/// to the first intersection, or `None` if the ray misses.
fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    if t > 0.0 {
        Some(t)
    } else {
        Some(-b + disc.sqrt()).filter(|&t2| t2 > 0.0)
    }
}
