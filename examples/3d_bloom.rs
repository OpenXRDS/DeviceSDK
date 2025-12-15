use bevy::{core_pipeline::tonemapping::Tonemapping, post_process::bloom::Bloom};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "3dBloom".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems(bounce_spheres);
    }
}

#[derive(Component)]
struct Bouncing;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Tonemapping::TonyMcMapface, // 1. Using a tonemapper that desaturates to white is recommended
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Bloom::NATURAL, // 2. Enable bloom for the camera
    ));

    let material_emissive1 = materials.add(StandardMaterial {
        emissive: LinearRgba::rgb(0.0, 0.0, 150.0), // 3. Put something bright in a dark environment to see the effect
        ..default()
    });
    let material_emissive2 = materials.add(StandardMaterial {
        emissive: LinearRgba::rgb(1000.0, 1000.0, 1000.0),
        ..default()
    });
    let material_emissive3 = materials.add(StandardMaterial {
        emissive: LinearRgba::rgb(50.0, 0.0, 0.0),
        ..default()
    });
    let material_non_emissive = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        ..default()
    });

    let mesh = meshes.add(Sphere::new(0.4).mesh().ico(5).unwrap());

    for x in -5..5 {
        for z in -5..5 {
            // This generates a pseudo-random integer between `[0, 6)`, but deterministically so
            // the same spheres are always the same colors.
            let mut hasher = DefaultHasher::new();
            (x, z).hash(&mut hasher);
            let rand = (hasher.finish() + 3) % 6;

            let (material, scale) = match rand {
                0 => (material_emissive1.clone(), 0.5),
                1 => (material_emissive2.clone(), 0.1),
                2 => (material_emissive3.clone(), 1.0),
                3..=5 => (material_non_emissive.clone(), 1.5),
                _ => unreachable!(),
            };

            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(x as f32 * 2.0, 0.0, z as f32 * 2.0)
                    .with_scale(Vec3::splat(scale)),
                Bouncing,
            ));
        }
    }
}

fn bounce_spheres(time: Res<Time>, mut query: Query<&mut Transform, With<Bouncing>>) {
    for mut transform in query.iter_mut() {
        transform.translation.y =
            ops::sin(transform.translation.x + transform.translation.z + time.elapsed_secs());
    }
}
