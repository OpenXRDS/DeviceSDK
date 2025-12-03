use crate::*;
use bevy::{
    log::{Level, LogPlugin},
    prelude::*,
};

use error::RuntimeError;
use xrds_openxr::OpenXrCamera;

pub trait RuntimeHandler {
    fn on_construct(&mut self) {}
    fn on_begin(&mut self) {}
    fn on_resumed(&mut self) {}
    fn on_suspended(&mut self) {}
    fn on_end(&mut self) {}
    fn on_update(&mut self) {}
    fn on_deconstruct(&mut self) {}
}

pub struct Runtime {
    app: App,
}

pub struct RuntimeParameters {
    pub app_name: String,
    pub enable_xr: bool,
}

impl Runtime {
    pub fn new(params: RuntimeParameters) -> Self {
        let mut app = App::new();

        // Add log plugin first for logging in plugin build phase
        app.add_plugins(LogPlugin {
            level: Level::INFO,
            filter: "bevy=info,wgpu=warn,naga=info".to_owned(),
            ..Default::default()
        });
        if params.enable_xr {
            app.add_plugins(xrds_openxr::add_plugins(
                DefaultPlugins.build().disable::<LogPlugin>(),
                if params.app_name.is_empty() {
                    "OpenXRDS".to_owned()
                } else {
                    params.app_name.clone()
                },
            ));
        } else {
            app.add_plugins(DefaultPlugins.build().disable::<LogPlugin>());
        }

        app.add_systems(Startup, test_setup);
        Self { app }
    }

    pub fn run<A>(mut self, mut app: A) -> Result<(), RuntimeError>
    where
        A: RuntimeHandler + Send + Sync,
    {
        app.on_begin();

        // Pseudo Code
        // app.on_begin(self.world);
        // app.world.build_startup(&mut self.app);

        self.app.run();

        app.on_end();

        Ok(())
    }
}

fn test_setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(128, 128, 255)),
            ..Default::default()
        },
        OpenXrCamera,
        Transform::default(),
    ));
}
