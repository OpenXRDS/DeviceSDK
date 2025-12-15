use std::f32::consts::PI;

use bevy::{
    anti_alias::taa::TemporalAntiAliasing, core_pipeline::Skybox, pbr::ScreenSpaceAmbientOcclusion,
};
use wgpu::{wgt::TextureViewDescriptor, TextureViewDimension};
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "Skybox".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems((
            asset_loaded,
            update_camera_direction,
            update_light_direction,
        ));
    }
}

#[derive(Resource)]
struct Cubemap {
    is_loaded: bool,
    image_handle: Handle<Image>,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        DirectionalLight {
            illuminance: 32000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 2.0, 0.0).with_rotation(Quat::from_rotation_x(-PI / 4.)),
    ));

    let skybox_handle = asset_server.load("environment_maps/specular.ktx2");
    // camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
            ..default()
        },
    ));

    commands.insert_resource(AmbientLight {
        color: Color::srgb_u8(210, 220, 240),
        brightness: 1.0,
        ..default()
    });

    commands.insert_resource(Cubemap {
        is_loaded: false,
        image_handle: skybox_handle,
    });
}

fn asset_loaded(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cubemap: ResMut<Cubemap>,
    mut skyboxes: Query<&mut Skybox>,
) {
    if !cubemap.is_loaded && asset_server.load_state(&cubemap.image_handle).is_loaded() {
        let image = images.get_mut(&cubemap.image_handle).unwrap();
        // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
        // so they appear as one texture. The following code reconfigures the texture as necessary.
        if image.texture_descriptor.array_layer_count() == 1 {
            image.reinterpret_stacked_2d_as_array(image.height() / image.width());
            image.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        }

        for mut skybox in &mut skyboxes {
            skybox.image = cubemap.image_handle.clone();
        }

        cubemap.is_loaded = true;
    }
}

fn update_camera_direction(time: Res<Time>, mut camera: Query<&mut Transform, With<Camera3d>>) {
    for mut transform in &mut camera {
        transform.rotate_y(time.delta_secs() * 0.1);
    }
}

fn update_light_direction(
    time: Res<Time>,
    mut light: Query<&mut Transform, With<DirectionalLight>>,
) {
    for mut transform in &mut light {
        transform.rotate_y(time.delta_secs() * 0.5);
    }
}
