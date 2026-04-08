// XRDS Interface Level: 3 (Expert Runtime Hooks)
// Purpose: Expose low-level runtime lifecycle/scheduling control on top of Bevy.
// Target: Engine integrators and advanced users needing custom system wiring and scheduling behavior.
// When To Use: Use when SDK application facade is insufficient and direct lifecycle hooks are required.
use crate::*;
use bevy::{
    ecs::system::ScheduleSystem,
    gizmos::{config::GizmoConfig, AppGizmoBuilder},
    log::{Level, LogPlugin},
    prelude::*,
};

use error::RuntimeError;

struct XrdsAppAdapter<A> {
    app: Option<A>,
}

impl<A> XrdsAppAdapter<A> {
    fn new(app: A) -> Self {
        Self { app: Some(app) }
    }
}

pub struct OnConstruct<'a>(&'a mut App);

impl OnConstruct<'_> {
    pub fn app_mut(&mut self) -> &mut App {
        self.0
    }

    pub fn add_systems<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
        self.0.add_systems(Startup, systems);
    }

    pub fn init_gizmo_group<Config>(&mut self)
    where
        Config: bevy::gizmos::config::GizmoConfigGroup,
    {
        self.0.init_gizmo_group::<Config>();
    }

    pub fn insert_gizmo_config<Config>(&mut self, group: Config, config: GizmoConfig)
    where
        Config: bevy::gizmos::config::GizmoConfigGroup,
    {
        self.0.insert_gizmo_config::<Config>(group, config);
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) {
        self.0.insert_resource(resource);
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
        self.0.add_systems(Update, systems);
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

pub(crate) fn build_bevy_app(params: &RuntimeParameters) -> App {
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

    app
}

impl Runtime {
    pub fn new(params: RuntimeParameters) -> Self {
        Self {
            app: build_bevy_app(&params),
        }
    }

    /**
     * Directly run the Bevy app with the provided handler for lifecycle hooks.
     * This is a low-level API that gives you full control over the app lifecycle and scheduling.
     * Use this when you need to customize the app setup and update behavior beyond what the XrdsApp trait allows.
     * For most use cases, prefer using `run_xrds` with an XrdsApp implementation for better ergonomics and integration with the SDK.
     */
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

    /**
    * Run an XrdsApp implementation with automatic lifecycle management. This is the recommended way to run an SDK application.
     - The provided app will have its `setup` method called during the construct phase, where you can spawn entities and set up resources using the XrdsAPI.
     - The app's `update` method will be called every frame, allowing you to implement your game logic and interact with the API.
     - This method abstracts away the direct handling of Bevy's app lifecycle and provides a more ergonomic interface for typical SDK applications.
    */
    pub fn run_xrds<A>(self, app: A) -> Result<(), RuntimeError>
    where
        A: XrdsApp + Send + Sync + 'static,
    {
        self.run(XrdsAppAdapter::new(app))
    }
}

impl<A> RuntimeHandler for XrdsAppAdapter<A>
where
    A: XrdsApp + Send + Sync + 'static,
{
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        let mut api = XrdsAPI::attach(on_construct.app_mut());
        let mut app = self
            .app
            .take()
            .expect("XrdsAppAdapter app was already consumed before on_construct");
        app.setup(&mut api);
        api.insert_resource(XrdsAppState { app });
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems(run_xrds_app_update::<A>);
    }
}
