use crate::*;
use bevy::{
    ecs::system::ScheduleSystem,
    log::{Level, LogPlugin},
    prelude::*,
};

use error::RuntimeError;

pub struct OnConstruct<'a>(&'a mut App);

impl OnConstruct<'_> {
    pub fn add_systems<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
        self.0.add_systems(Startup, systems);
    }
}

pub struct OnBegin<'a>(&'a mut App);

impl OnBegin<'_> {
    pub fn add_systems<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
        self.0.add_systems(PostStartup, systems);
    }
}

pub struct OnUpdate<'a>(&'a mut App);

impl OnUpdate<'_> {
    pub fn add_systems<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
        self.0.add_systems(FixedUpdate, systems);
    }
}

#[allow(unused_variables)]
pub trait RuntimeHandler {
    fn on_construct(&mut self, on_construct: OnConstruct) {}
    fn on_begin(&mut self, on_begin: OnBegin) {}
    fn on_resumed(&mut self) {}
    fn on_suspended(&mut self) {}
    fn on_end(&mut self) {}
    fn on_update(&mut self, on_update: OnUpdate) {}
    fn on_deconstruct(&mut self) {}
}

pub struct Runtime {
    app: App,
}

pub struct RuntimeParameters {
    pub app_name: String,
    pub enable_xr: bool,
}

impl Default for RuntimeParameters {
    fn default() -> Self {
        Self {
            app_name: "OpenXRDS".to_owned(),
            enable_xr: false,
        }
    }
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

        Self { app }
    }

    pub fn run<H>(mut self, mut handler: H) -> Result<(), RuntimeError>
    where
        H: RuntimeHandler + Send + Sync,
    {
        handler.on_construct(OnConstruct(&mut self.app));
        handler.on_begin(OnBegin(&mut self.app));
        handler.on_update(OnUpdate(&mut self.app));

        self.app.run();

        handler.on_end();

        Ok(())
    }
}
