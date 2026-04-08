use bevy::prelude::*;
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp};

struct DirectBevyHandler;

#[derive(Component)]
struct AnimatedCube;

impl XrdsApp for DirectBevyHandler {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        api.add_startup_system(setup);
        api.add_update_system(animate_cube);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        PointLight {
            intensity: 1_200_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 4.0),
    ));

    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.0, 0.0),
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.insert_resource(AmbientLight {
        brightness: 60.0,
        ..default()
    });

    let cube_mesh = meshes.add(Mesh::from(Cuboid::new(2.0, 2.0, 2.0)));
    let cube_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),
        ..default()
    });

    commands.spawn((
        AnimatedCube,
        Mesh3d(cube_mesh),
        MeshMaterial3d(cube_material),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));
}

fn animate_cube(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cubes: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<AnimatedCube>>,
) {
    let t = time.elapsed_secs();
    let yaw_radians = (t * 45.0).to_radians();
    let r = 0.5 + 0.5 * t.sin();
    let g = 0.5 + 0.5 * (t * 1.7).sin();
    let b = 0.5 + 0.5 * (t * 2.3).sin();

    for (mut transform, material_handle) in &mut cubes {
        transform.translation = Vec3::new(0.0, 0.5, 0.0);
        transform.rotation = Quat::from_rotation_y(yaw_radians);
        transform.scale = Vec3::splat(2.0);

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgb(r, g, b);
        }
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(DirectBevyHandler)
        .expect("failed to run direct_bevy");
}
