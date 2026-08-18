use std::path::Path;

use bevy::{
    asset::io::{
        memory::{Dir, MemoryAssetReader},
        AssetSourceBuilder, AssetSourceBuilders,
    },
    prelude::*,
};

use crate::openxr::{
    resources::OpenXrInstance,
    schedule::{openxr_in_state_focused, OpenXrRuntimeSystems, OpenXrSchedules},
    session::OpenXrSession,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Handles to the runtime-provided controller GLTF scenes. Available as a Bevy resource.
/// `is_ready` becomes `true` after the PostSessionCreate system runs — whether or not models
/// were actually loaded. Use it to distinguish "not yet initialized" from "unavailable".
#[derive(Resource, Default)]
pub struct XrControllerModelAssets {
    pub left:     Option<Handle<Scene>>,
    pub right:    Option<Handle<Scene>>,
    pub is_ready: bool,
}

/// The in-memory asset directory backing the `"controller"` asset source.
/// Keep this alive for the lifetime of the app so the source stays readable.
#[derive(Resource)]
pub struct ControllerModelDir(pub Dir);

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ControllerModelPlugin;

impl Plugin for ControllerModelPlugin {
    fn build(&self, app: &mut App) {
        // Register a custom "controller://" asset source backed by an in-memory Dir.
        // Must happen during build() — before AssetPlugin::finish() builds the sources.
        let dir = Dir::default();
        let dir_for_source = dir.clone();
        app.world_mut()
            .get_resource_or_init::<AssetSourceBuilders>()
            .insert(
                "controller",
                AssetSourceBuilder::default()
                    .with_reader(move || Box::new(MemoryAssetReader { root: dir_for_source.clone() })),
            );

        app.insert_resource(ControllerModelDir(dir));
        app.init_resource::<XrControllerModelAssets>();
        // Load on the first focused frame — not PostSessionCreate, because Meta's runtime
        // rejects xrEnumerateRenderModelPathsFB before xrBeginSession has been called.
        app.add_systems(
            OpenXrSchedules::Update,
            load_controller_models
                .in_set(OpenXrRuntimeSystems::PreFrameLoop)
                .run_if(openxr_in_state_focused),
        );
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn load_controller_models(
    mut ran:      Local<bool>,
    instance:     Res<OpenXrInstance>,
    session:      Res<OpenXrSession>,
    asset_server: Res<AssetServer>,
    dir:          Res<ControllerModelDir>,
    mut assets:   ResMut<XrControllerModelAssets>,
) {
    if *ran { return; }

    // Extension permanently unavailable — mark done, no retry needed.
    if instance.instance.exts().fb_render_model.is_none() {
        info!("XR_FB_render_model: extension not available");
        *ran = true;
        assets.is_ready = true;
        return;
    }

    // On Quest Link / Air Link (PCVR) this returns false — the PC runtime does not expose
    // controller meshes. The extension is listed but unusable. xrEnumerateRenderModelPathsFB
    // will return ERROR_VALIDATION_FAILURE in this configuration.
    match instance.instance.supports_render_model_loading(instance.system_id) {
        Ok(true) => {}
        Ok(false) => {
            info!("XR_FB_render_model: system does not support render model loading (Quest Link / PCVR)");
            *ran = true;
            assets.is_ready = true;
            return;
        }
        Err(e) => {
            info!("XR_FB_render_model: supports_render_model_loading query failed: {e:?}");
            *ran = true;
            assets.is_ready = true;
            return;
        }
    }

    let paths = match session.enumerate_render_model_paths_fb() {
        Ok(p) => p,
        Err(e) => {
            info!("XR_FB_render_model: enumerate failed ({e:?}) — not available on this runtime");
            *ran = true;
            assets.is_ready = true;
            return;
        }
    };

    // Log all available paths so we can see what the runtime provides.
    for p in &paths {
        let name = instance.instance.path_to_string(*p).unwrap_or_else(|_| "<unknown>".into());
        info!("render model path available: {name}");
    }

    // Load left and right independently to avoid split-borrowing through ResMut.
    if let Some(handle) = load_one_model(
        "/model_fb/controller/left", "left.glb",
        &instance, &session, &asset_server, &dir, &paths,
    ) {
        assets.left = Some(handle);
    }

    if let Some(handle) = load_one_model(
        "/model_fb/controller/right", "right.glb",
        &instance, &session, &asset_server, &dir, &paths,
    ) {
        assets.right = Some(handle);
    }

    *ran = true;
    assets.is_ready = true;
}

fn load_one_model(
    xr_path_str:  &str,
    filename:     &str,
    instance:     &OpenXrInstance,
    session:      &OpenXrSession,
    asset_server: &AssetServer,
    dir:          &ControllerModelDir,
    paths:        &[openxr::Path],
) -> Option<Handle<Scene>> {
    let xr_path = instance.instance.string_to_path(xr_path_str).ok()?;

    if !paths.contains(&xr_path) {
        info!("controller path {filename} not in enumerated paths");
        return None;
    }

    let props = session
        .get_render_model_properties_fb(xr_path, openxr::RenderModelFlagsFB::SUPPORTS_GLTF_2_0_SUBSET_2)
        .map_err(|e| warn!("get_render_model_properties_fb {filename}: {e:?}"))
        .ok()?;

    if props.model_key == openxr::sys::RenderModelKeyFB::NULL {
        info!("no render model available for {filename}");
        return None;
    }

    let bytes = session
        .load_render_model_fb(props.model_key)
        .map_err(|e| warn!("load_render_model_fb {filename}: {e:?}"))
        .ok()?;

    let byte_count = bytes.len();
    dir.0.insert_asset(Path::new(filename), bytes);
    let handle: Handle<Scene> = asset_server.load(format!("controller://{filename}#Scene0"));
    info!("controller render model loaded: {filename} ({byte_count} bytes)");
    Some(handle)
}
