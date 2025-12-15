use bevy::{
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    post_process::bloom::Bloom,
};
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "2dBloom".to_owned(),
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
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Tonemapping::TonyMcMapface, // 1. Using a tonemapper that desaturates to white is recommended
        Bloom::default(),           // 2. Enable bloom for the camera
        DebandDither::Enabled,      // Optional: bloom causes gradients which cause banding
    ));

    // Circle mesh
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(100.))),
        // 3. Put something bright in a dark environment to see the effect
        MeshMaterial2d(materials.add(Color::srgb(7.5, 0.0, 7.5))),
        Transform::from_translation(Vec3::new(-200., 0., 0.)),
    ));

    // Hexagon mesh
    commands.spawn((
        Mesh2d(meshes.add(RegularPolygon::new(100., 6))),
        // 3. Put something bright in a dark environment to see the effect
        MeshMaterial2d(materials.add(Color::srgb(6.25, 9.4, 9.1))),
        Transform::from_translation(Vec3::new(200., 0., 0.)),
    ));
}
