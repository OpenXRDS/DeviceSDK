// XRDS Interface Level: 3 (Expert Runtime Hooks)
// Purpose: Expose low-level runtime lifecycle/scheduling control on top of Bevy.
// Target: Engine integrators and advanced users needing custom system wiring and scheduling behavior.
// When To Use: Use when SDK application facade is insufficient and direct lifecycle hooks are required.
use crate::*;
use bevy::{
    asset::UnapprovedPathMode,
    ecs::system::ScheduleSystem,
    gizmos::{config::GizmoConfig, AppGizmoBuilder},
    log::{Level, LogPlugin},
    prelude::*,
    window::WindowResolution,
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
        // PostUpdate guarantees all Update deferred commands are flushed before
        // run_xrds_app_update executes, eliminating the race condition where
        // regular Update systems (e.g. bevy_fontmesh::update_text_meshes) queue
        // commands for entities that reimport_scene_in_world then despawns.
        self.0.add_systems(PostUpdate, systems);
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
    /// Override the directory Bevy's `AssetPlugin` uses to resolve asset paths.
    ///
    /// Defaults to `None`, which lets Bevy use its own default (typically the
    /// `assets/` directory next to the package's `Cargo.toml`).
    ///
    /// Set this when the executable's working directory differs from the asset
    /// root — for example, an editor app in a sub-directory of a workspace that
    /// shares the workspace-root `assets/` folder:
    ///
    /// ```rust,ignore
    /// RuntimeParameters {
    ///     asset_path: Some(
    ///         std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    ///             .join("../../assets")
    ///             .to_string_lossy()
    ///             .into_owned(),
    ///     ),
    ///     ..Default::default()
    /// }
    /// ```
    pub asset_path: Option<String>,
    /// Allow loading assets from paths outside the configured asset root.
    ///
    /// Required when the editor lets users pick arbitrary files from the filesystem
    /// (e.g. `C:/Users/.../Downloads/model.glb`). Defaults to `false`.
    pub allow_unapproved_paths: bool,
    /// Allow Bevy's winit event loop to start on a non-main thread.
    ///
    /// Required when another framework (e.g. Tauri) already owns the main thread's
    /// event loop. Has no effect on macOS where the main thread is always required.
    /// Defaults to `false`.
    pub run_on_any_thread: bool,
    /// Initial logical size of the primary window `(width, height)`.
    /// Defaults to `None`, which lets Bevy use its own default (800 × 600).
    pub window_resolution: Option<(f32, f32)>,
}

impl Default for RuntimeParameters {
    fn default() -> Self {
        Self {
            app_name: "OpenXRDS".to_owned(),
            enable_xr: false,
            asset_path: None,
            allow_unapproved_paths: false,
            run_on_any_thread: false,
            window_resolution: None,
        }
    }
}

pub(crate) fn build_bevy_app(params: &RuntimeParameters) -> App {
    let mut app = App::new();

    // Add log plugin first for logging in plugin build phase
    app.add_plugins(LogPlugin {
        level: Level::INFO,
        filter: "bevy=info,wgpu=warn,wgpu_hal=off,naga=off,symphonia_bundle_mp3=error,symphonia_core=error,bevy_gltf=error".to_owned(),
        ..Default::default()
    });

    let mut asset_plugin = bevy::asset::AssetPlugin::default();
    if let Some(ref path) = params.asset_path {
        asset_plugin.file_path = path.clone();
    }

    // Resolve the asset root to an absolute path and pre-configure bundled fonts so
    // bevy_rich_text3d can find them regardless of the working directory at runtime.
    // Only register paths that actually exist on the filesystem — cosmic-text panics
    // at first text render if no fonts are loaded at all, but silently skips missing
    // files at load time. On Android/APK mode (asset_path = None), fonts live inside
    // the APK and are not filesystem-accessible, so we only set LoadFonts when we have
    // a real directory we can verify contains the fonts.
    {
        use std::path::PathBuf;
        let asset_root_opt: Option<PathBuf> = params.asset_path
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|d| d.join("assets"))
            });
        if let Some(asset_root) = asset_root_opt {
            let font_names = [
                "NotoSans-Regular.ttf",
                "NotoSans-Bold.ttf",
                "NotoSans-Italic.ttf",
                "NotoSans-BoldItalic.ttf",
            ];
            let font_paths: Vec<String> = font_names
                .iter()
                .map(|f| asset_root.join("fonts").join(f))
                .filter(|p| p.exists())
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            if !font_paths.is_empty() {
                app.insert_resource(bevy_rich_text3d::LoadFonts {
                    font_paths,
                    ..Default::default()
                });
            }
        }
    }
    if params.allow_unapproved_paths {
        asset_plugin.unapproved_path_mode = UnapprovedPathMode::Allow;
    }

    let use_xr = params.enable_xr && {
        if xrds_openxr::is_openxr_available() {
            true
        } else {
            warn!("enable_xr=true but no OpenXR runtime found — falling back to desktop rendering");
            false
        }
    };

    let mut winit_plugin = bevy::winit::WinitPlugin::<bevy::winit::WakeUp>::default();
    winit_plugin.run_on_any_thread = params.run_on_any_thread;

    let window_plugin = WindowPlugin {
        primary_window: Some(Window {
            title: if params.app_name.is_empty() {
                "OpenXRDS".to_owned()
            } else {
                params.app_name.clone()
            },
            resolution: params.window_resolution
                .map(|(w, h)| WindowResolution::new(w as u32, h as u32))
                .unwrap_or_default(),
            ..Default::default()
        }),
        ..Default::default()
    };

    if use_xr {
        app.add_plugins(xrds_openxr::add_plugins(
            DefaultPlugins
                .build()
                .disable::<LogPlugin>()
                .set(asset_plugin)
                .set(winit_plugin)
                .set(window_plugin),
            params.app_name.clone(),
        ));
    } else {
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<LogPlugin>()
                .set(asset_plugin)
                .set(winit_plugin)
                .set(window_plugin),
        );
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
        let mut app = self
            .app
            .take()
            .expect("XrdsAppAdapter app was already consumed before on_construct");
        // configure fires first so the app can add plugins (e.g. EguiPlugin) before
        // XRDS resources are initialised and before app.run() calls finish/cleanup.
        app.configure(on_construct.app_mut());
        let mut api = XrdsAPI::attach(on_construct.app_mut());
        app.setup(&mut api);
        api.insert_resource(XrdsAppState { app });
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems(run_xrds_app_update::<A>.in_set(XrdsUpdateSystemSet));
    }
}
