use std::f32::consts::PI;

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{
    camera::Exposure,
    color::palettes::css::{ANTIQUE_WHITE, BLUE, LIME, ORANGE_RED, RED},
    core_pipeline::tonemapping::Tonemapping,
    light::{NotShadowCaster, PointLightShadowMap, TransmittedShadowReceiver},
    post_process::bloom::Bloom,
    render::view::{ColorGrading, ColorGradingGlobal},
};
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "LightTransmission".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems((rotate_camera, flicker_system));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let icosphere_mesh = meshes.add(Sphere::new(0.9).mesh().ico(7).unwrap());
    let cube_mesh = meshes.add(Cuboid::new(0.7, 0.7, 0.7));
    let plane_mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0));
    let cylinder_mesh = meshes.add(Cylinder::new(0.5, 2.0).mesh().resolution(50));

    commands.insert_resource(ClearColor(Color::BLACK));
    commands.insert_resource(PointLightShadowMap { size: 2048 });
    commands.insert_resource(AmbientLight {
        brightness: 0.0,
        ..Default::default()
    });

    // Cube #1
    commands.spawn((
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
        Transform::from_xyz(0.25, 0.5, -2.0).with_rotation(Quat::from_euler(
            EulerRot::XYZ,
            1.4,
            3.7,
            21.3,
        )),
    ));

    // Cube #2
    commands.spawn((
        Mesh3d(cube_mesh),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
        Transform::from_xyz(-0.75, 0.7, -2.0).with_rotation(Quat::from_euler(
            EulerRot::XYZ,
            0.4,
            2.3,
            4.7,
        )),
    ));

    // Candle
    commands.spawn((
        Mesh3d(cylinder_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.2, 0.3),
            diffuse_transmission: 0.7,
            perceptual_roughness: 0.32,
            thickness: 0.2,
            ..default()
        })),
        Transform::from_xyz(-1.0, 0.0, 0.0),
    ));

    // Candle Flame
    let scaled_white = LinearRgba::from(ANTIQUE_WHITE) * 20.;
    let scaled_orange = LinearRgba::from(ORANGE_RED) * 4.;
    let emissive = LinearRgba {
        red: scaled_white.red + scaled_orange.red,
        green: scaled_white.green + scaled_orange.green,
        blue: scaled_white.blue + scaled_orange.blue,
        alpha: 1.0,
    };

    commands.spawn((
        Mesh3d(icosphere_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            emissive,
            diffuse_transmission: 1.0,
            ..default()
        })),
        Transform::from_xyz(-1.0, 1.15, 0.0).with_scale(Vec3::new(0.1, 0.2, 0.1)),
        Flicker,
        NotShadowCaster,
    ));

    // Glass Sphere
    commands.spawn((
        Mesh3d(icosphere_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            specular_transmission: 0.9,
            diffuse_transmission: 1.0,
            thickness: 1.8,
            ior: 1.5,
            perceptual_roughness: 0.12,
            ..default()
        })),
        Transform::from_xyz(1.0, 0.0, 0.0),
    ));

    // R Sphere
    commands.spawn((
        Mesh3d(icosphere_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: RED.into(),
            specular_transmission: 0.9,
            diffuse_transmission: 1.0,
            thickness: 1.8,
            ior: 1.5,
            perceptual_roughness: 0.12,
            ..default()
        })),
        Transform::from_xyz(1.0, -0.5, 2.0).with_scale(Vec3::splat(0.5)),
    ));

    // G Sphere
    commands.spawn((
        Mesh3d(icosphere_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: LIME.into(),
            specular_transmission: 0.9,
            diffuse_transmission: 1.0,
            thickness: 1.8,
            ior: 1.5,
            perceptual_roughness: 0.12,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.5, 2.0).with_scale(Vec3::splat(0.5)),
    ));

    // B Sphere
    commands.spawn((
        Mesh3d(icosphere_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: BLUE.into(),
            specular_transmission: 0.9,
            diffuse_transmission: 1.0,
            thickness: 1.8,
            ior: 1.5,
            perceptual_roughness: 0.12,
            ..default()
        })),
        Transform::from_xyz(-1.0, -0.5, 2.0).with_scale(Vec3::splat(0.5)),
    ));

    // Chessboard Plane
    let black_material = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        reflectance: 0.3,
        perceptual_roughness: 0.8,
        ..default()
    });

    let white_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        reflectance: 0.3,
        perceptual_roughness: 0.8,
        ..default()
    });

    for x in -3..4 {
        for z in -3..4 {
            commands.spawn((
                Mesh3d(plane_mesh.clone()),
                MeshMaterial3d(if (x + z) % 2 == 0 {
                    black_material.clone()
                } else {
                    white_material.clone()
                }),
                Transform::from_xyz(x as f32 * 2.0, -1.0, z as f32 * 2.0),
            ));
        }
    }

    // Paper
    commands.spawn((
        Mesh3d(plane_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            diffuse_transmission: 0.6,
            perceptual_roughness: 0.8,
            reflectance: 1.0,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.5, -3.0)
            .with_scale(Vec3::new(2.0, 1.0, 1.0))
            .with_rotation(Quat::from_euler(EulerRot::XYZ, PI / 2.0, 0.0, 0.0)),
        TransmittedShadowReceiver,
    ));

    // Candle Light
    commands.spawn((
        Transform::from_xyz(-1.0, 1.7, 0.0),
        PointLight {
            color: Color::from(
                LinearRgba::from(ANTIQUE_WHITE).mix(&LinearRgba::from(ORANGE_RED), 0.2),
            ),
            intensity: 4_000.0,
            radius: 0.2,
            range: 5.0,
            shadows_enabled: true,
            ..default()
        },
        Flicker,
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 1.8, 7.0).looking_at(Vec3::ZERO, Vec3::Y),
        ColorGrading {
            global: ColorGradingGlobal {
                post_saturation: 1.2,
                ..default()
            },
            ..default()
        },
        Tonemapping::TonyMcMapface,
        Exposure { ev100: 6.0 },
        Msaa::Off,
        TemporalAntiAliasing::default(),
        EnvironmentMapLight {
            intensity: 25.0,
            diffuse_map: asset_server.load("environment_maps/diffuse.ktx2"),
            specular_map: asset_server.load("environment_maps/specular.ktx2"),
            ..default()
        },
        Bloom::default(),
    ));
}

fn rotate_camera(time: Res<Time>, mut query: Query<&mut Transform, With<Camera3d>>) {
    for mut transform in &mut query {
        let radius = 7.0;
        let speed = 0.2;
        let angle = time.elapsed_secs() * speed;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        transform.translation = Vec3::new(x, 1.8, z);
        transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}

#[derive(Component)]
struct Flicker;

fn flicker_system(
    mut flame: Single<&mut Transform, (With<Flicker>, With<Mesh3d>)>,
    light: Single<(&mut PointLight, &mut Transform), (With<Flicker>, Without<Mesh3d>)>,
    time: Res<Time>,
) {
    let s = time.elapsed_secs();
    let a = ops::cos(s * 6.0) * 0.0125 + ops::cos(s * 4.0) * 0.025;
    let b = ops::cos(s * 5.0) * 0.0125 + ops::cos(s * 3.0) * 0.025;
    let c = ops::cos(s * 7.0) * 0.0125 + ops::cos(s * 2.0) * 0.025;
    let (mut light, mut light_transform) = light.into_inner();
    light.intensity = 4_000.0 + 3000.0 * (a + b + c);
    flame.translation = Vec3::new(-1.0, 1.23, 0.0);
    flame.look_at(Vec3::new(-1.0 - c, 1.7 - b, 0.0 - a), Vec3::X);
    flame.rotate(Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, PI / 2.0));
    light_transform.translation = Vec3::new(-1.0 - c, 1.7, 0.0 - a);
    flame.translation = Vec3::new(-1.0 - c, 1.23, 0.0 - a);
}
