use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "AnimatedMaterial".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems(animate_materials);
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
        Transform::from_xyz(3.0, 1.0, 3.0).looking_at(Vec3::new(0.0, -0.5, 0.0), Vec3::Y),
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/diffuse.ktx2"),
            specular_map: asset_server.load("environment_maps/specular.ktx2"),
            intensity: 2000.0,
            ..default()
        },
    ));
    let cube = meshes.add(Cuboid::new(0.5, 0.5, 0.5));

    const GOLDEN_ANGLE: f32 = 137.507_77;

    let mut hsla = Hsla::hsl(0.0, 1.0, 0.5);
    for x in -1..2 {
        for z in -1..2 {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(materials.add(Color::from(hsla))),
                Transform::from_translation(Vec3::new(x as f32, 0.0, z as f32)),
            ));
            hsla = hsla.rotate_hue(GOLDEN_ANGLE);
        }
    }
}

fn animate_materials(
    material_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for material_handle in material_handles.iter() {
        if let Some(material) = materials.get_mut(material_handle) {
            if let Color::Hsla(ref mut hsla) = material.base_color {
                *hsla = hsla.rotate_hue(time.delta_secs() * 100.0);
            }
        }
    }
}
