use xrds::{bevy::light::CascadeShadowConfigBuilder, *};

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "LoadGltf".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // light
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: 1.6,
            ..Default::default()
        }
        .build(),
    ));
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.5, 0.8, 0.5).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
    ));
    commands.spawn((SceneRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/StainedGlassLamp/StainedGlassLamp.gltf"),
    )),));
}
