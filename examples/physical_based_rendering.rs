use bevy::camera::ScalingMode;
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "PhysicalBasedRendering".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems(environment_map_load_finish);
    }
}

#[derive(Component)]
struct EnvironmentMapLabel;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let sphere_mesh = meshes.add(Sphere::new(0.45));
    // add entities to the world
    for y in -2..=2 {
        for x in -5..=5 {
            let x01 = (x + 5) as f32 / 10.0;
            let y01 = (y + 2) as f32 / 4.0;
            // sphere
            commands.spawn((
                Mesh3d(sphere_mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Srgba::hex("#FF3333").unwrap().into(),
                    // vary key PBR parameters on a grid of spheres to show the effect
                    metallic: y01,
                    perceptual_roughness: x01,
                    ..default()
                })),
                Transform::from_xyz(x as f32, y as f32 + 0.5, 0.0),
            ));
        }
    }
    // unlit sphere
    commands.spawn((
        Mesh3d(sphere_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("#FF3333").unwrap().into(),
            // vary key PBR parameters on a grid of spheres to show the effect
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(-5.0, -2.5, 0.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 1_500.,
            ..default()
        },
        Transform::from_xyz(50.0, 50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // labels
    commands.spawn((
        Text::new("Perceptual Roughness"),
        TextFont {
            font_size: 30.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: px(20),
            left: px(100),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Metallic"),
        TextFont {
            font_size: 30.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: px(130),
            right: Val::ZERO,
            ..default()
        },
        UiTransform {
            rotation: Rot2::degrees(90.),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Loading Environment Map..."),
        TextFont {
            font_size: 30.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            bottom: px(20),
            right: px(20),
            ..default()
        },
        EnvironmentMapLabel,
    ));

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::default(), Vec3::Y),
        Projection::from(OrthographicProjection {
            scale: 0.01,
            scaling_mode: ScalingMode::WindowSize,
            ..OrthographicProjection::default_3d()
        }),
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/diffuse.ktx2"),
            specular_map: asset_server.load("environment_maps/specular.ktx2"),
            intensity: 900.0,
            ..default()
        },
    ));
}

fn environment_map_load_finish(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    environment_map: Single<&EnvironmentMapLight>,
    label_entity: Option<Single<Entity, With<EnvironmentMapLabel>>>,
) {
    if asset_server
        .load_state(&environment_map.diffuse_map)
        .is_loaded()
        && asset_server
            .load_state(&environment_map.specular_map)
            .is_loaded()
    {
        // Do not attempt to remove `label_entity` if it has already been removed.
        if let Some(label_entity) = label_entity {
            commands.entity(*label_entity).despawn();
        }
    }
}
