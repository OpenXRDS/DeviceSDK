//! Attaches a visible GLB mesh to every scene Camera entity so that camera
//! nodes are identifiable in the 3D viewport.
//!
//! The mesh lives at `apps/xrds-editor/resource/cinema_camera.glb` and is
//! loaded via an absolute path (the editor runs with `allow_unapproved_paths`).
//! A `CameraIconMarker` child entity is spawned under each camera entity the
//! first time the system sees it; subsequent frames skip entities that already
//! have the marker child.

use xrds::editor::{
    AssetServer, Camera, Children, Commands, Component, Entity, Query, Res, SceneRoot, Visibility,
    With, Without, bevy_ecs,
};

use crate::camera::EditorCameraMarker;
use crate::player::PlayerPawnMarker;

/// Marks the child entity that holds the camera body mesh.
#[derive(Component)]
pub struct CameraIconMarker;

/// Absolute path (forward-slash normalised) to the camera GLB, resolved at
/// compile time from the editor crate's manifest directory.
const CAMERA_GLB_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resource/cinema_camera.glb#Scene0",
);

pub fn setup_camera_icons(app: &mut xrds::editor::App) {
    app.add_systems(xrds::editor::Update, attach_camera_icons);
}

/// Each frame: find scene Camera entities that do not yet have a
/// `CameraIconMarker` child and attach one.
fn attach_camera_icons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cam_q: Query<(Entity, Option<&Children>), (With<Camera>, Without<EditorCameraMarker>, Without<PlayerPawnMarker>)>,
    icon_q: Query<(), With<CameraIconMarker>>,
) {
    // Normalise backslashes once (compile-time constant may have `\` on Windows).
    let path = CAMERA_GLB_PATH.replace('\\', "/");

    for (entity, children) in cam_q.iter() {
        let has_icon = children
            .map(|ch| ch.iter().any(|&c| icon_q.contains(c)))
            .unwrap_or(false);

        if !has_icon {
            // Ensure the camera entity has visibility components so that the
            // child SceneRoot (which requires InheritedVisibility) doesn't
            // trigger Bevy warning B0004.
            commands.entity(entity).insert(Visibility::Inherited);
            let icon = commands
                .spawn((
                    CameraIconMarker,
                    SceneRoot(asset_server.load(path.clone())),
                    Visibility::Inherited,
                ))
                .id();
            commands.entity(entity).add_child(icon);
        }
    }
}
