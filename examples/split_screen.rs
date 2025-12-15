use bevy::camera::Viewport;
use xrds::{bevy::window::WindowResized, *};

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "SplitScreen".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems(update_camera_viewport);
    }
}

#[derive(Component)]
struct CameraViewport {
    offset: Vec2,
    size: Vec2,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((SceneRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/StainedGlassLamp/StainedGlassLamp.gltf"),
    )),));
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    for (index, (camera_name, camera_pos)) in [
        ("LeftEyeCamera", Vec3::new(-1.0, 1.0, 2.0)),
        ("RightEyeCamera", Vec3::new(1.0, 1.0, 2.0)),
    ]
    .iter()
    .enumerate()
    {
        let camera = commands
            .spawn((
                Camera3d::default(),
                Transform::from_translation(*camera_pos).looking_at(Vec3::ZERO, Vec3::Y),
                Camera {
                    order: index as isize,
                    ..Default::default()
                },
                CameraViewport {
                    offset: Vec2::new(index as f32 * 0.5, 0.0),
                    size: Vec2::new(0.5, 1.0),
                },
            ))
            .id();

        commands.spawn((
            UiTargetCamera(camera),
            Node {
                width: percent(100),
                height: percent(100),
                ..Default::default()
            },
            children![(
                Text::new(*camera_name),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12),
                    left: px(12),
                    ..Default::default()
                }
            )],
        ));
    }
}

fn update_camera_viewport(
    windows: Query<&Window>,
    mut window_resized_reader: MessageReader<WindowResized>,
    mut query: Query<(&mut Camera, &CameraViewport)>,
) {
    for window_resized in window_resized_reader.read() {
        let window = windows.get(window_resized.window).unwrap();
        for (mut camera, viewport) in query.iter_mut() {
            camera.viewport = Some(Viewport {
                physical_position: UVec2::new(
                    (viewport.offset.x * window.physical_width() as f32) as u32,
                    (viewport.offset.y * window.physical_height() as f32) as u32,
                ),
                physical_size: UVec2::new(
                    (viewport.size.x * window.physical_width() as f32) as u32,
                    (viewport.size.y * window.physical_height() as f32) as u32,
                ),
                ..Default::default()
            });
        }
    }
}
