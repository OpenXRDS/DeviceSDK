use std::f32::consts::PI;

use bevy::{
    light::CascadeShadowConfigBuilder, post_process::effect_stack::ChromaticAberration,
    render::view::Hdr,
};
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "PostprocessingBuiltin".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        DistanceFog {
            color: Color::srgb_u8(43, 44, 47),
            falloff: FogFalloff::Linear {
                start: 1.0,
                end: 8.0,
            },
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/diffuse.ktx2"),
            specular_map: asset_server.load("environment_maps/specular.ktx2"),
            intensity: 2000.0,
            ..default()
        },
        // Include the `ChromaticAberration` component.
        ChromaticAberration {
            intensity: 0.09,
            max_samples: 30,
            ..Default::default()
        },
    ));
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    commands.spawn((SceneRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/StainedGlassLamp/StainedGlassLamp.gltf"),
    )),));

    // Spawn the light.
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, PI * -0.15, PI * -0.15)),
        CascadeShadowConfigBuilder {
            maximum_distance: 3.0,
            first_cascade_far_bound: 0.9,
            ..default()
        }
        .build(),
    ));
}
