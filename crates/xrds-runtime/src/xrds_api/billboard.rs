use bevy::prelude::*;

/// Marker component inserted on `XrdsText` entities whose `XrdsTextAnchor` is `Billboard`.
///
/// The `billboard_system` reads this component every PostUpdate frame and overwrites the
/// entity's local rotation so that its `-Z` axis points toward the active `Camera3d`,
/// preserving world-up (`Vec3::Y`).
#[derive(Component)]
pub struct XrdsBillboard;

/// Per-frame system — rotates every `XrdsBillboard` entity to face the active `Camera3d`.
///
/// World-up (`Vec3::Y`) is preserved so the text stays upright regardless of the camera
/// elevation angle.  Parent rotation is accounted for so entities parented to a moving
/// character also work correctly.
pub fn billboard_system(
    mut billboard_q: Query<
        (&mut Transform, &GlobalTransform, Option<&ChildOf>),
        With<XrdsBillboard>,
    >,
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    parent_q: Query<&GlobalTransform>,
) {
    // Use the first Camera3d.  single() would fail in XR mode (2 eye cameras), so
    // we take the first — close enough for billboard rotation in both desktop and XR.
    let Some(cam_gt) = camera_q.iter().next() else { return; };
    let cam_pos = cam_gt.translation();

    for (mut transform, global_tf, child_of) in billboard_q.iter_mut() {
        let entity_pos = global_tf.translation();
        let to_cam = cam_pos - entity_pos;
        let len_sq = to_cam.length_squared();
        if len_sq < 1e-6 {
            continue; // camera is at the same position as the text — skip
        }
        let to_cam_norm = to_cam / len_sq.sqrt();

        // `looking_to(dir, up)` makes the entity's local -Z face `dir`.
        // bevy_rich_text3d renders text with its visible face on +Z (same as most
        // Bevy sprites/quads), so we pass `-to_cam_norm` to make +Z face the camera.
        let up = if to_cam_norm.dot(Vec3::Y).abs() > 0.999 {
            Vec3::Z  // fallback when camera is directly above/below
        } else {
            Vec3::Y
        };
        let world_rotation = Transform::IDENTITY.looking_to(-to_cam_norm, up).rotation;

        // Convert world rotation to local rotation (account for parent).
        let parent_world_rotation = child_of
            .and_then(|co| parent_q.get(co.0).ok())
            .map(|pgt| pgt.compute_transform().rotation)
            .unwrap_or(Quat::IDENTITY);

        transform.rotation = parent_world_rotation.inverse() * world_rotation;
    }
}
