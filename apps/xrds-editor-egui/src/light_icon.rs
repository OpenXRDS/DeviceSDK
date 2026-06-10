//! Attaches a visible GLB mesh to every scene light entity (PointLight,
//! SpotLight, DirectionalLight) so that light nodes are identifiable and
//! orientable in the 3D viewport.  AmbientLight is excluded because it is a
//! global resource with no meaningful world-space transform.
//!
//! The mesh lives at `apps/xrds-editor/resource/flashlight.glb` and is loaded
//! via an absolute path (the editor runs with `allow_unapproved_paths`).
//! A `LightIconMarker` child entity is spawned under each light entity the
//! first time the system sees it; subsequent frames are skipped.

use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::world::World;
use xrds::editor::{
    AssetServer, Children, Commands, Component, DirectionalLight, Entity, Or, PointLight, Query,
    Quat, Res, SceneRoot, SpotLight, Transform, Vec3, Visibility, With, Without, bevy_ecs,
};

use crate::camera::EditorCameraMarker;

/// Marks the child entity that holds the flashlight body mesh.
#[derive(Component)]
pub struct LightIconMarker;

const LIGHT_GLB_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resource/flashlight.glb#Scene0",
);

pub fn setup_light_icons(app: &mut xrds::editor::App) {
    app.add_systems(xrds::editor::Update, attach_light_icons);
}

fn attach_light_icons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    light_q: Query<
        (Entity, Option<&Children>),
        (
            Or<(With<PointLight>, With<SpotLight>, With<DirectionalLight>)>,
            Without<EditorCameraMarker>,
        ),
    >,
    icon_q: Query<(), With<LightIconMarker>>,
) {
    let path = LIGHT_GLB_PATH.replace('\\', "/");

    for (entity, children) in light_q.iter() {
        let has_icon = children
            .map(|ch| ch.iter().any(|&c| icon_q.contains(c)))
            .unwrap_or(false);

        if !has_icon {
            // The GLB model's head points along +Y in model space.  Bevy lights
            // illuminate along local -Z, so rotate -90° around X to map +Y → -Z.
            let corrective = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
            let icon = commands
                .spawn((
                    LightIconMarker,
                    SceneRoot(asset_server.load(path.clone())),
                    Transform {
                        rotation: corrective,
                        scale: Vec3::splat(3.0),
                        ..Default::default()
                    },
                    Visibility::Inherited,
                ))
                .id();
            commands.queue(move |world: &mut World| {
                if world.get_entity(entity).is_ok() {
                    world.entity_mut(icon).insert(ChildOf(entity));
                } else if let Ok(mut e) = world.get_entity_mut(icon) {
                    e.despawn();
                }
            });
        }
    }
}
