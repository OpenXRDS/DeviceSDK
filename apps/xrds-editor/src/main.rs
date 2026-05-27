mod camera;
mod camera_icon;
mod gizmo;
mod gizmo_interaction;
mod icon;
mod io;
mod light_icon;
mod panels;
mod player;
mod selection;
mod state;
mod templates;

use bevy::asset::Assets;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::image::Image;
use bevy::mesh::Mesh;
use bevy::prelude::{
    Added, AppGizmoBuilder, Commands, Entity, PerspectiveProjection, Projection, Resource, Time,
};

/// Three-stage pipeline that mirrors the wait-for-loaded pattern in `glb_runtime_add.rs`.
///
/// Stage 1 `pending`       — (node_id, Gltf handle) waiting for the Gltf parent asset
///                           (and all sub-assets) to fully load.  Driven by
///                           `render_preloaded_gltf_system`.
///
/// Stage 2 `ready_to_spawn`— Gltf is loaded; `XrdsApp::update` calls
///                           `spawn_document_node` this frame, which inserts `SceneRoot`.
///                           Entry moves to `spawned` immediately after.
///
/// Stage 3 `spawned`       — `SceneRoot` inserted; waiting for `gltf_load_status == Loaded`
///                           (i.e. scene spawner finished creating all bone/mesh entities).
///                           `XrdsApp::update` polls each frame.  Only when fully loaded
///                           does the handle move to `alive`.
///
/// `alive`                 — Handle kept alive indefinitely so Bevy does not evict the asset.
#[derive(Default, Resource)]
struct GltfPreloadStore {
    pending: Vec<(xrds::scene_graph::XrdsSceneNodeId, bevy::asset::Handle<bevy::gltf::Gltf>)>,
    ready_to_spawn: Vec<(xrds::scene_graph::XrdsSceneNodeId, bevy::asset::Handle<bevy::gltf::Gltf>)>,
    spawned: Vec<(xrds::scene_graph::XrdsSceneNodeId, bevy::asset::Handle<bevy::gltf::Gltf>)>,
    alive: Vec<bevy::asset::Handle<bevy::gltf::Gltf>>,
}

use xrds::editor::{
    App, ButtonInput, Camera, Camera3d, DefaultGizmoConfigGroup, EguiContexts, EguiPlugin,
    EguiPrimaryContextPass, EulerRot, GlobalTransform, GizmoConfigStore, KeyCode, MessageReader,
    Mesh3d, MouseButton, MouseMotion, Quat, Query, Res, ResMut, Result, Startup, Transform,
    Update, Vec2, Vec3, With, Without,
};


use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneDirectionalLight, XrdsSceneDocument, XrdsSceneEnvironment,
    XrdsSceneMaterial, XrdsSceneMaterialPbrParams, XrdsSceneMetadata, XrdsSceneNode,
    XrdsPlayerLocomotionMode, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsSceneSphere,
    XrdsSceneTransform,
};
use xrds::sdk::world::XrdsGltfAsset; // needed for handle_of::<XrdsGltfAsset> turbofish
use xrds::{
    Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsGltfAnimationPlaybackOptions,
    XrdsGltfAnimationSelector, XrdsUpdateContext,
};

use crate::state::{EditorSession, EditorState, GltfClipInfo};
use camera::{orbit_camera_system, spawn_editor_camera, EditorCameraMarker, EditorCameraState};
use player::{PawnLocomotionMode, PawnVerticalState, PlayHudMarker, PlayerPawnMarker};
use camera_icon::setup_camera_icons;
use gizmo::{
    floor_grid_system, interaction_zone_gizmo_system, light_rays_system, text3d_label_system,
    transform_gizmo_system, update_selection_outline, GridGizmoGroup,
};
use gizmo_interaction::gizmo_interaction_system;
use light_icon::setup_light_icons;
use panels::{
    hierarchy_panel, inspector_panel, menubar_panel, palette_panel, start_play, stop_play,
    template_picker_panel, toolbar_panel, viewport_panel,
};
use selection::setup_viewport_selection;

/// Every frame: for each pending GLB handle that has fully loaded, move it to
/// `alive` to keep the asset in memory.  No spawning — just load-state polling.
fn render_preloaded_gltf_system(
    mut store: ResMut<GltfPreloadStore>,
    asset_server: Res<bevy::prelude::AssetServer>,
) {
    let pending = std::mem::take(&mut store.pending);
    let mut still_pending = vec![];

    for (node_id, handle) in pending {
        let loaded = matches!(
            asset_server.load_state(handle.id()),
            bevy::asset::LoadState::Loaded
        ) && matches!(
            asset_server.recursive_dependency_load_state(handle.id()),
            bevy::asset::RecursiveDependencyLoadState::Loaded
        );
        if loaded {
            store.ready_to_spawn.push((node_id, handle));
        } else {
            still_pending.push((node_id, handle));
        }
    }
    store.pending = still_pending;
}

fn main() {
    // Point the asset server at the workspace-root `assets/` directory so the
    // editor finds the XRDS runtime shaders regardless of which sub-directory
    // it lives in.
    let asset_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .to_string_lossy()
        .into_owned();

    Runtime::new(RuntimeParameters {
        app_name: "XRDS Editor".to_owned(),
        asset_path: Some(asset_path),
        allow_unapproved_paths: true,
        ..Default::default()
    })
    .run_xrds(XrdsEditorApp)
    .expect("failed to run xrds-editor");
}

struct XrdsEditorApp;

impl XrdsApp for XrdsEditorApp {
    fn configure(&mut self, app: &mut App) {
        app.add_plugins(EguiPlugin::default());
        app.add_plugins(bevy_mod_outline::OutlinePlugin);
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        app.init_resource::<EditorState>();
        app.init_resource::<EditorCameraState>();
        app.init_resource::<GltfPreloadStore>();
        app.add_systems(Update, render_preloaded_gltf_system);

        // Viewport camera — spawned once at startup.
        app.add_systems(Startup, spawn_editor_camera);
        // Draw gizmos in front of all scene geometry (depth_bias = -1 = always on top).
        app.add_systems(Startup, configure_gizmos);
        // Register separate gizmo group for the floor grid (normal depth test).
        app.init_gizmo_group::<GridGizmoGroup>();

        // Keep any scene Camera nodes inactive so they don't override the editor viewport.
        app.add_systems(Update, deactivate_scene_cameras);
        // Attach the camera body mesh to every scene camera node.
        setup_camera_icons(app);
        // Attach the flashlight mesh to every scene light node (except AmbientLight).
        setup_light_icons(app);
        // Orbit / pan / zoom — runs every frame before rendering.
        app.add_systems(Update, orbit_camera_system);
        // Floor grid overlay — XZ plane reference grid.
        app.add_systems(Update, floor_grid_system);
        // Light-shape debug overlay — rays / cones / range indicators.
        app.add_systems(Update, light_rays_system);
        // Interaction zone outlines — wireframe box/sphere for every InteractionZone node.
        app.add_systems(Update, interaction_zone_gizmo_system);
        // Text3D overlay — egui labels projected from world-space positions (Camera3d has no Text2d pipeline).
        app.add_systems(Update, text3d_label_system);
        // Transform gizmo — world-space axis arrows on selected node.
        app.add_systems(Update, transform_gizmo_system);
        // Selection outline — adds OutlineVolume to selected mesh entities.
        app.add_systems(Update, update_selection_outline);
        // Gizmo interaction — hover detection and axis drag.
        app.add_systems(Update, gizmo_interaction_system);
        // Performance stats — collect FPS, mesh, and texture data into EditorState.
        app.add_systems(Update, update_perf_stats);
        // Disable frustum culling on every spawned mesh so animated/skinned GLBs
        // are never culled due to a zero or stale AABB at rest pose.
        app.add_systems(Update, disable_frustum_culling_on_new_meshes);
        // Parse GLB files to populate animation clip names; runs once per node.
        app.add_systems(Update, parse_gltf_clips_system);
        // Viewport picking — observer fires on Pointer<Click> for every mesh entity.
        setup_viewport_selection(app);

        // Player pawn — spawn/despawn driven by is_playing; locomotion in play mode.
        app.add_systems(Update, spawn_player_pawn_system);
        app.add_systems(Update, despawn_player_pawn_system);
        app.add_systems(Update, pawn_locomotion_system);

        // All panels share one egui context pass.  egui requires TopBottom →
        // Left → Right ordering within the same frame, so they must be in a
        // single system rather than three separate ones.
        app.add_systems(EguiPrimaryContextPass, editor_ui);
    }

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = default_scene();

        api.insert_resource(EditorSession::new(document.clone()));

        api.import_scene_document(&document)
            .expect("default scene should import cleanly");
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        // ── Poll background build job (Export as Application) ────────────────
        let build_finished = ctx
            .resource::<EditorState>()
            .map(|s| s.build_job.as_ref().map(|j| j.handle.is_finished()).unwrap_or(false))
            .unwrap_or(false);
        if build_finished {
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                if let Some(job) = state.build_job.take() {
                    match job.handle.join() {
                        Ok(Ok(())) => {
                            let dist = job.out_dir.join("target").join("release");
                            // Copy assets/ next to the binary so it runs without cargo.
                            let src_assets = job.out_dir.join("assets");
                            let dst_assets = dist.join("assets");
                            if let Err(e) = crate::io::copy_dir_recursive(&src_assets, &dst_assets) {
                                state.status_message = Some(format!("Build ok, but asset copy failed: {e}"));
                            } else {
                                let _ = crate::io::reveal_in_explorer(&dist);
                                state.status_message = Some(format!(
                                    "Build complete!  Runnable package in: {}",
                                    dist.display()
                                ));
                            }
                        }
                        Ok(Err(e)) => {
                            state.status_message = Some(format!("Build failed: {e}"));
                        }
                        Err(_) => {
                            state.status_message = Some("Build thread panicked".into());
                        }
                    }
                }
            }
        }

        // Read all pending transform/material previews first (immutable borrow),
        // then apply them (mutable calls).
        // ── Full reimport (new nodes added via palette) ──────────────────────
        // ── Step 2: Preload GLB assets (no SceneRoot, no spawning) ───────────
        // For each node queued by the palette, look up its GLB URI from the
        // document and tell Bevy's asset server to start loading it.  The handle
        // is kept alive in GltfPreloadStore.  No Bevy entity is spawned here.
        let pending_spawns: Vec<XrdsSceneNodeId> = ctx
            .resource::<EditorState>()
            .map(|s| s.pending_node_spawns.clone())
            .unwrap_or_default();
        if !pending_spawns.is_empty() {
            let gltf_paths: Vec<String> = {
                let doc_opt = ctx.resource::<EditorSession>().map(|s| s.document().clone());
                if let Some(doc) = doc_opt {
                    pending_spawns
                        .iter()
                        .filter_map(|id| {
                            doc.nodes.iter().find(|n| n.id == *id).and_then(|node| {
                                if let xrds::scene_graph::XrdsSceneNodePayload::GltfAsset(g) =
                                    &node.payload
                                {
                                    doc.assets
                                        .iter()
                                        .find(|a| Some(&a.id) == g.asset_id.as_ref())
                                        .map(|a| a.uri.clone())
                                } else {
                                    None
                                }
                            })
                        })
                        .collect()
                } else {
                    vec![]
                }
            };
            // pair each path back with its node_id so the render system can name entities
            let node_handle_pairs: Vec<(XrdsSceneNodeId, bevy::asset::Handle<bevy::gltf::Gltf>)> = {
                let server = ctx.resource::<bevy::prelude::AssetServer>();
                if let Some(server) = server {
                    pending_spawns
                        .iter()
                        .zip(gltf_paths.into_iter())
                        .map(|(id, p)| (*id, server.load::<bevy::gltf::Gltf>(p)))
                        .collect()
                } else {
                    vec![]
                }
            };
            if let Some(mut store) = ctx.resource_mut::<GltfPreloadStore>() {
                store.pending.extend(node_handle_pairs);
            }
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_node_spawns.clear();
            }
        }

        // ── Stage 2 → 3: call spawn_document_node for Gltf-loaded nodes ────────
        // `render_preloaded_gltf_system` (Bevy system) moves entries from `pending`
        // to `ready_to_spawn` once the Gltf parent asset and all sub-assets are
        // fully loaded.  Here we call `spawn_document_node` (which inserts SceneRoot)
        // and move entries to `spawned` for the next-stage poll.
        let ready: Vec<(xrds::scene_graph::XrdsSceneNodeId, bevy::asset::Handle<bevy::gltf::Gltf>)> = ctx
            .resource_mut::<GltfPreloadStore>()
            .map(|mut store| std::mem::take(&mut store.ready_to_spawn))
            .unwrap_or_default();
        if !ready.is_empty() {
            let doc = ctx
                .resource::<EditorSession>()
                .map(|s| s.document().clone());
            if let Some(doc) = doc {
                for (node_id, handle) in ready {
                    let xrds_id = xrds::sdk::XrdsId::from(node_id);
                    match ctx.spawn_document_node(xrds_id, &doc) {
                        Ok(_) => {
                            if let Some(mut store) = ctx.resource_mut::<GltfPreloadStore>() {
                                store.spawned.push((node_id, handle));
                            }
                        }
                        Err(e) => {
                            bevy::log::warn!("[editor] spawn_document_node failed for {node_id:?}: {e:?}");
                        }
                    }
                }
            }
        }

        // ── Stage 3 → alive: wait for SceneRoot scene spawner to finish ──────
        // Poll `gltf_load_status` for each spawned-but-not-yet-ready node.
        // Only after `Loaded` do we consider the entity truly ready and move
        // the handle to `alive`.  This mirrors the pattern in glb_runtime_add.rs.
        let spawned: Vec<(xrds::scene_graph::XrdsSceneNodeId, bevy::asset::Handle<bevy::gltf::Gltf>)> = ctx
            .resource_mut::<GltfPreloadStore>()
            .map(|mut store| std::mem::take(&mut store.spawned))
            .unwrap_or_default();
        if !spawned.is_empty() {
            let mut still_spawned = Vec::new();
            for (node_id, handle) in spawned {
                let xrds_id = xrds::sdk::XrdsId::from(node_id);
                let is_loaded = ctx
                    .handle_of::<XrdsGltfAsset>(xrds_id)
                    .as_ref()
                    .and_then(|h| ctx.gltf_load_status(h))
                    .map(|s| matches!(s, xrds::XrdsGltfLoadStatus::Loaded))
                    .unwrap_or(false);
                if is_loaded {
                    if let Some(mut store) = ctx.resource_mut::<GltfPreloadStore>() {
                        store.alive.push(handle);
                    }
                } else {
                    still_spawned.push((node_id, handle));
                }
            }
            if let Some(mut store) = ctx.resource_mut::<GltfPreloadStore>() {
                store.spawned.extend(still_spawned);
            }
        }

        // ── Full reimport (structural changes: delete, reparent, paste, load) ─
        let needs_reimport = ctx
            .resource::<EditorState>()
            .map(|s| s.needs_full_reimport)
            .unwrap_or(false);
        if needs_reimport {
            let doc = ctx
                .resource::<EditorSession>()
                .map(|s| s.document().clone());
            if let Some(doc) = doc {
                let _ = ctx.reimport_scene(&doc);
            }
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.needs_full_reimport = false;
                state.needs_runtime_sync = false; // reimport subsumes sync
            }
        }

        // ── Start GLB animations when play mode begins ────────────────────────
        let (play_started, _is_playing) = ctx
            .resource::<EditorState>()
            .map(|s| (s.play_started, s.is_playing))
            .unwrap_or_default();
        if play_started {
            let gltf_ids: Vec<_> = ctx
                .resource::<EditorSession>()
                .map(|s| {
                    s.document()
                        .nodes
                        .iter()
                        .filter_map(|n| match &n.payload {
                            XrdsSceneNodePayload::GltfAsset(_) => Some(n.id),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();

            for node_id in gltf_ids {
                let xrds_id = xrds::sdk::XrdsId::from(node_id);
                if let Some(handle) = ctx.handle_of::<XrdsGltfAsset>(xrds_id) {
                    let _ = ctx.play_gltf_animation(
                        &handle,
                        XrdsGltfAnimationSelector::Index(0),
                        XrdsGltfAnimationPlaybackOptions::default(),
                    );
                }
            }

            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.play_started = false;
            }
        }

        // ── Undo/redo full sync ───────────────────────────────────────────────
        // After an undo or redo the document is already reverted.  Push every
        // node's transform and visibility from the document to the live runtime
        // so the 3D viewport immediately reflects the change.
        let needs_sync = ctx
            .resource::<EditorState>()
            .map(|s| s.needs_runtime_sync)
            .unwrap_or(false);
        if needs_sync {
            if let Some(nodes) = ctx.resource::<EditorSession>().map(|s| {
                s.document()
                    .nodes
                    .iter()
                    .map(|n| {
                        (
                            n.id,
                            n.transform.translation,
                            n.transform.rotation_quat_xyzw,
                            n.transform.scale,
                            n.visible,
                        )
                    })
                    .collect::<Vec<_>>()
            }) {
                for (node_id, t, r, s, vis) in nodes {
                    let id = xrds::sdk::XrdsId::from(node_id);
                    ctx.set_translation_for_node(id, t);
                    ctx.set_rotation_for_node(id, r);
                    ctx.set_scale_for_node(id, s);
                    ctx.set_visible_for_node(id, vis);
                }
            }
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.needs_runtime_sync = false;
            }
        }

        // ── Per-frame inspector preview ───────────────────────────────────────
        let (translations, rotations, s, m, vis) = ctx
            .resource::<EditorState>()
            .map(|st| {
                (
                    st.pending_translations.clone(),
                    st.pending_rotations.clone(),
                    st.pending_scale,
                    st.pending_material.as_ref().map(|(id, p)| (*id, p.clone())),
                    st.pending_visible,
                )
            })
            .unwrap_or_default();

        for (id, v) in translations {
            ctx.set_translation_for_node(xrds::sdk::XrdsId::from(id), v);
        }
        for (id, v) in rotations {
            ctx.set_rotation_for_node(xrds::sdk::XrdsId::from(id), v);
        }
        if let Some((id, v)) = s {
            ctx.set_scale_for_node(xrds::sdk::XrdsId::from(id), v);
        }
        if let Some((id, v)) = m {
            ctx.set_material_params_for_node(xrds::sdk::XrdsId::from(id), v);
        }
        if let Some((id, v)) = vis {
            ctx.set_visible_for_node(xrds::sdk::XrdsId::from(id), v);
        }

        // ── Per-frame light previews ──────────────────────────────────────────
        let (pl, dl, sl, al) = ctx
            .resource::<EditorState>()
            .map(|st| {
                (
                    st.pending_point_light,
                    st.pending_directional_light,
                    st.pending_spot_light,
                    st.pending_ambient_light,
                )
            })
            .unwrap_or_default();

        if let Some((id, color, intensity, range)) = pl {
            ctx.set_point_light_params_for_node(
                xrds::sdk::XrdsId::from(id),
                color,
                intensity,
                range,
            );
        }
        if let Some((id, color, illuminance)) = dl {
            ctx.set_directional_light_params_for_node(
                xrds::sdk::XrdsId::from(id),
                color,
                illuminance,
            );
        }
        if let Some((id, color, intensity, range, inner, outer)) = sl {
            ctx.set_spot_light_params_for_node(
                xrds::sdk::XrdsId::from(id),
                color,
                intensity,
                range,
                inner,
                outer,
            );
        }
        if let Some((_id, color, brightness)) = al {
            ctx.set_ambient_light_params(color, brightness);
        }

        // ── GLB animation state refresh (from runtime) ───────────────────────
        // Clip LIST is populated by a separate Bevy system (parse_gltf_clips_system)
        // that reads the file directly.  Here we only update the per-frame
        // playback state for nodes that already have a handle.
        let gltf_node_ids: Vec<_> = ctx
            .resource::<EditorSession>()
            .map(|s| {
                s.document()
                    .nodes
                    .iter()
                    .filter_map(|n| match &n.payload {
                        XrdsSceneNodePayload::GltfAsset(_) => Some(n.id),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut state_map = std::collections::HashMap::new();
        for node_id in &gltf_node_ids {
            let xrds_id = xrds::sdk::XrdsId::from(*node_id);
            if let Some(handle) = ctx.handle_of::<xrds::sdk::world::XrdsGltfAsset>(xrds_id) {
                let anim_state = ctx.gltf_animation_state(&handle).ok().flatten();
                state_map.insert(*node_id, anim_state);
            }
        }
        if let Some(mut state) = ctx.resource_mut::<EditorState>() {
            state.gltf_anim_state = state_map;
        }

        // ── Pending GLB animation commands ────────────────────────────────────
        let (pending_play, pending_stop, pending_pause, pending_resume) = ctx
            .resource::<EditorState>()
            .map(|s| {
                (
                    s.pending_gltf_play,
                    s.pending_gltf_stop,
                    s.pending_gltf_pause,
                    s.pending_gltf_resume,
                )
            })
            .unwrap_or_default();

        if let Some((node_id, clip_index, speed, repeat)) = pending_play {
            let xrds_id = xrds::sdk::XrdsId::from(node_id);
            if let Some(handle) = ctx.handle_of::<xrds::sdk::world::XrdsGltfAsset>(xrds_id) {
                let _ = ctx.play_gltf_animation(
                    &handle,
                    XrdsGltfAnimationSelector::Index(clip_index),
                    XrdsGltfAnimationPlaybackOptions {
                        speed,
                        repeat,
                        start_paused: false,
                    },
                );
            }
        }
        if let Some(node_id) = pending_stop {
            let xrds_id = xrds::sdk::XrdsId::from(node_id);
            if let Some(handle) = ctx.handle_of::<xrds::sdk::world::XrdsGltfAsset>(xrds_id) {
                let _ = ctx.stop_gltf_animation(&handle);
            }
        }
        if let Some(node_id) = pending_pause {
            let xrds_id = xrds::sdk::XrdsId::from(node_id);
            if let Some(handle) = ctx.handle_of::<xrds::sdk::world::XrdsGltfAsset>(xrds_id) {
                let _ = ctx.pause_gltf_animation(&handle);
            }
        }
        if let Some(node_id) = pending_resume {
            let xrds_id = xrds::sdk::XrdsId::from(node_id);
            if let Some(handle) = ctx.handle_of::<xrds::sdk::world::XrdsGltfAsset>(xrds_id) {
                let _ = ctx.resume_gltf_animation(&handle);
            }
        }

        if pending_play.is_some()
            || pending_stop.is_some()
            || pending_pause.is_some()
            || pending_resume.is_some()
        {
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_gltf_play = None;
                state.pending_gltf_stop = None;
                state.pending_gltf_pause = None;
                state.pending_gltf_resume = None;
            }
        }


        // ── Scene environment sync ────────────────────────────────────────────
        // Push fog + exposure from the session document to the runtime every
        // frame so changes made in the inspector take effect immediately without
        // requiring a full reimport.
        let env: Option<XrdsSceneEnvironment> = ctx
            .resource::<EditorSession>()
            .and_then(|s| s.document().metadata.environment.clone());
        match env {
            Some(e) => {
                ctx.set_scene_environment(e);
            }
            None => {
                ctx.clear_scene_environment();
            }
        }
    }
}

// ── Combined egui pass ────────────────────────────────────────────────────────
// All panels must draw in one system so they share the same egui frame.
// egui requires: TopBottom → Left → Right → Central

fn editor_ui(
    mut contexts: EguiContexts,
    mut session: ResMut<EditorSession>,
    mut editor_state: ResMut<EditorState>,
    mut cam_state: ResMut<EditorCameraState>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    // Widen resize grab zones for all panel edges from the default 5 px to 8 px.
    // This makes it much easier to grab hierarchy/inspector/palette resize handles.
    ctx.style_mut(|s| s.interaction.resize_grab_radius_side = 8.0);

    let state = &mut *editor_state;
    // egui panel order: all TopBottom first, then Left/Right, then Central area.
    menubar_panel(ctx, &mut *session, state); // top strip — File / Edit / Window menus
    toolbar_panel(ctx, &mut *session, state, &mut cam_state); // top strip — quick-access buttons
    if !state.is_playing {
        if state.show_palette {
            palette_panel(ctx, &mut *session, state);
        }
        if state.show_hierarchy {
            hierarchy_panel(ctx, &mut *session, state);
        }
        if state.show_inspector {
            inspector_panel(ctx, &mut *session, state);
        }
    }
    viewport_panel(ctx, &mut cam_state, state); // orientation indicator / play-mode HUD
    template_picker_panel(ctx, &mut *session, state); // modal — shown over everything else
    if state.show_perf_stats {
        perf_stats_overlay(ctx, state);
    }
    Ok(())
}

// ── Performance stats overlay ─────────────────────────────────────────────────

fn perf_stats_overlay(ctx: &mut xrds::editor::egui::Context, state: &EditorState) {
    use xrds::editor::egui;
    let s = &state.perf_stats;
    egui::Window::new("Stats")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(4.0, 4.0))
        .show(ctx, |ui| {
            egui::Grid::new("stats_grid")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    ui.label("FPS");
                    ui.label(format!("{:.1}", s.fps));
                    ui.end_row();
                    ui.label("Frame");
                    ui.label(format!("{:.2} ms", s.frame_ms));
                    ui.end_row();
                    ui.label("Meshes");
                    ui.label(format!("{}", s.mesh_entity_count));
                    ui.end_row();
                    ui.label("Vertices");
                    ui.label(if s.vertex_count >= 1_000_000 {
                        format!("{:.1} M", s.vertex_count as f32 / 1_000_000.0)
                    } else {
                        format!("{} K", s.vertex_count / 1_000)
                    });
                    ui.end_row();
                    ui.label("Textures");
                    ui.label(format!("{} KB", s.texture_memory_kb));
                    ui.end_row();
                });
        });
}

// ── Frustum culling ───────────────────────────────────────────────────────────

fn disable_frustum_culling_on_new_meshes(
    mut commands: Commands,
    query: Query<Entity, Added<Mesh3d>>,
) {
    for entity in &query {
        commands.entity(entity).insert(NoFrustumCulling);
    }
}

// ── Performance stats system ──────────────────────────────────────────────────

fn update_perf_stats(
    diagnostics: Res<DiagnosticsStore>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    mesh_query: Query<(), With<Mesh3d>>,
    mut editor_state: ResMut<EditorState>,
) {
    if !editor_state.show_perf_stats {
        return;
    }

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.average())
        .unwrap_or(0.0) as f32;
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.average())
        .unwrap_or(0.0) as f32;

    let mesh_entity_count = mesh_query.iter().count() as u32;

    let mut vertex_count: u64 = 0;
    for (_, mesh) in meshes.iter() {
        vertex_count += mesh.count_vertices() as u64;
    }

    let mut texture_memory_kb: u64 = 0;
    for (_, image) in images.iter() {
        texture_memory_kb += image.data.as_deref().map(|d| d.len() as u64).unwrap_or(0);
    }
    texture_memory_kb /= 1024;

    let stats = &mut editor_state.perf_stats;
    stats.fps = fps;
    stats.frame_ms = frame_ms;
    stats.mesh_entity_count = mesh_entity_count;
    stats.vertex_count = vertex_count;
    stats.texture_memory_kb = texture_memory_kb;
}

// ── One-time startup configuration ───────────────────────────────────────────

/// Make all gizmos render in front of scene geometry so axis handles are always
/// visible and clickable regardless of what mesh is at the same position.
fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0; // -1 = always in front of everything
}

// ── Editor-only helpers ───────────────────────────────────────────────────────

/// Ensure Camera nodes added to the scene document never take over the viewport.
/// The editor's own camera keeps `EditorCameraMarker`; every other Camera entity
/// is kept inactive so Bevy always renders through the editor camera.
fn deactivate_scene_cameras(
    mut scene_cameras: Query<&mut Camera, (Without<EditorCameraMarker>, Without<PlayerPawnMarker>)>,
) {
    for mut cam in scene_cameras.iter_mut() {
        if cam.is_active {
            cam.is_active = false;
        }
    }
}

// ── GLB clip parser ───────────────────────────────────────────────────────────

/// Reads GLB files directly to populate `EditorState::gltf_clips`.
/// Runs every frame but does work only for nodes not yet in the cache.
fn parse_gltf_clips_system(session: Res<EditorSession>, mut editor_state: ResMut<EditorState>) {
    for node in &session.document().nodes {
        let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
            continue;
        };
        if editor_state.gltf_clips.contains_key(&node.id) {
            continue;
        }
        // Strip any Bevy sub-asset fragment (e.g. "#Scene0") before opening the file.
        let raw = asset
            .asset_uri
            .split('#')
            .next()
            .unwrap_or(&asset.asset_uri);
        let path = std::path::Path::new(raw);
        // For relative paths try multiple locations:
        // 1. The original path (for absolute paths or paths in the working directory)
        // 2. assets/ directory (matches how the runtime's validate_gltf_source resolves them)
        let resolved = if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            [
                path.to_path_buf(),
                std::path::Path::new("assets").join(path),
            ]
            .into_iter()
            .find(|p| p.is_file())
        };
        let Some(resolved) = resolved else { continue };
        let Ok(gltf) = gltf::Gltf::open(&resolved) else {
            continue;
        };
        let clips: Vec<GltfClipInfo> = gltf
            .document
            .animations()
            .map(|a| GltfClipInfo {
                index: a.index(),
                name: a.name().map(str::to_owned),
            })
            .collect();
        editor_state.gltf_clips.insert(node.id, clips);
    }
}

// ── Player pawn ───────────────────────────────────────────────────────────────

const PAWN_MOVE_SPEED: f32 = 5.0;
const PAWN_LOOK_SENSITIVITY: f32 = 0.003;
const GRAVITY: f32 = -9.8;
const JUMP_IMPULSE: f32 = 4.5;
/// Offset from the PlayerSpawn node's Y (feet/floor level) to eye level.
const EYE_HEIGHT: f32 = 1.6;

/// Spawn the player pawn when play mode starts (detected by `is_playing && pawn_entity.is_none()`).
fn spawn_player_pawn_system(
    mut commands: Commands,
    mut editor_state: ResMut<EditorState>,
    session: Res<EditorSession>,
    cam_state: Res<EditorCameraState>,
    mut editor_cameras: Query<&mut Camera, With<EditorCameraMarker>>,
) {
    if !editor_state.is_playing || editor_state.pawn_entity.is_some() {
        return;
    }

    let (spawn_pos, spawn_rot, fov_deg, locomotion_mode) = session
        .document()
        .nodes
        .iter()
        .find_map(|n| {
            if let XrdsSceneNodePayload::PlayerSpawn(s) = &n.payload {
                let t = &n.transform;
                Some((
                    Vec3::from(t.translation),
                    Quat::from_array(t.rotation_quat_xyzw),
                    s.fov_deg,
                    s.locomotion_mode,
                ))
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            let t = cam_state.to_transform();
            (t.translation, t.rotation, 90.0, XrdsPlayerLocomotionMode::Flying)
        });

    // Treat spawn_pos.y as floor/feet level; camera lives at eye height above it.
    let eye_pos = Vec3::new(spawn_pos.x, spawn_pos.y + EYE_HEIGHT, spawn_pos.z);

    let pawn = commands
        .spawn((
            PlayerPawnMarker,
            PawnLocomotionMode(locomotion_mode),
            PawnVerticalState {
                velocity: 0.0,
                is_grounded: true,
                ground_y: eye_pos.y,
            },
            Camera3d::default(),
            Camera { is_active: true, ..Default::default() },
            Projection::Perspective(PerspectiveProjection {
                fov: fov_deg.to_radians(),
                ..Default::default()
            }),
            Transform { translation: eye_pos, rotation: spawn_rot, ..Default::default() },
            GlobalTransform::default(),
        ))
        .id();

    editor_state.pawn_entity = Some(pawn);

    // Spawn Bevy UI HUD targeted at the pawn camera so it renders over the game view.
    commands.spawn((
        PlayHudMarker,
        bevy::ui::UiTargetCamera(pawn),
        bevy::ui::Node {
            width: bevy::ui::Val::Percent(100.0),
            height: bevy::ui::Val::Percent(100.0),
            ..Default::default()
        },
    )).with_children(|parent| {
        // Top-left: playing hint
        parent.spawn((
            PlayHudMarker,
            bevy::ui::widget::Text::new("▶ PLAYING   ESC to stop"),
            bevy::ui::Node {
                position_type: bevy::ui::PositionType::Absolute,
                top: bevy::ui::Val::Px(8.0),
                left: bevy::ui::Val::Px(8.0),
                ..Default::default()
            },
            bevy::text::TextFont {
                font_size: 14.0,
                ..Default::default()
            },
            bevy::text::TextColor(bevy::color::Color::srgba(1.0, 0.86, 0.31, 0.9)),
        ));
    });

    for mut cam in editor_cameras.iter_mut() {
        cam.is_active = false;
    }
}

/// Despawn the player pawn and HUD when play mode stops.
fn despawn_player_pawn_system(
    mut commands: Commands,
    mut editor_state: ResMut<EditorState>,
    mut editor_cameras: Query<&mut Camera, With<EditorCameraMarker>>,
    hud_q: Query<Entity, With<PlayHudMarker>>,
) {
    if editor_state.is_playing || editor_state.pawn_entity.is_none() {
        return;
    }

    if let Some(entity) = editor_state.pawn_entity.take() {
        commands.entity(entity).despawn();
    }

    for entity in hud_q.iter() {
        commands.entity(entity).despawn();
    }

    for mut cam in editor_cameras.iter_mut() {
        cam.is_active = true;
    }
}

/// Locomotion for the player pawn during play mode.
///
/// - `Flying`: free-fly WASD (Q/E up/down), RMB look — matches the editor fly cam.
/// - `Smooth` / `Teleport`: grounded WASD with kinematic gravity and Space to jump.
///   Horizontal movement is projected onto the XZ plane so looking up/down does
///   not cause the player to float.  Ground level is fixed at the spawn's Y.
fn pawn_locomotion_system(
    mut pawn_q: Query<(&mut Transform, &mut PawnVerticalState, &PawnLocomotionMode), With<PlayerPawnMarker>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    time: Res<Time>,
    editor_state: Res<EditorState>,
    mut contexts: EguiContexts,
) {
    let mut delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        delta += Vec2::new(ev.delta.x, ev.delta.y);
    }

    if !editor_state.is_playing {
        return;
    }

    let Some((mut transform, mut vs, loco_mode)) = pawn_q.iter_mut().next() else {
        return;
    };

    let egui_wants_input = contexts
        .ctx_mut()
        .map(|ctx| ctx.wants_pointer_input() || ctx.wants_keyboard_input())
        .unwrap_or(false);

    // RMB free-look — shared by all modes.
    if mouse_buttons.pressed(MouseButton::Right) && !egui_wants_input && delta != Vec2::ZERO {
        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        yaw   -= delta.x * PAWN_LOOK_SENSITIVITY;
        pitch  = (pitch - delta.y * PAWN_LOOK_SENSITIVITY)
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.02, std::f32::consts::FRAC_PI_2 - 0.02);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let dt    = time.delta_secs();

    match loco_mode.0 {
        XrdsPlayerLocomotionMode::Flying => {
            // Full 3-D free-fly: movement follows the look direction including pitch.
            if !egui_wants_input {
                let speed   = if shift { PAWN_MOVE_SPEED * 3.0 } else { PAWN_MOVE_SPEED };
                let forward = transform.rotation * -Vec3::Z;
                let right   = transform.rotation *  Vec3::X;

                if keyboard.pressed(KeyCode::KeyW) { transform.translation += forward * speed * dt; }
                if keyboard.pressed(KeyCode::KeyS) { transform.translation -= forward * speed * dt; }
                if keyboard.pressed(KeyCode::KeyA) { transform.translation -= right   * speed * dt; }
                if keyboard.pressed(KeyCode::KeyD) { transform.translation += right   * speed * dt; }
                if keyboard.pressed(KeyCode::KeyE) { transform.translation += Vec3::Y * speed * dt; }
                if keyboard.pressed(KeyCode::KeyQ) { transform.translation -= Vec3::Y * speed * dt; }
            }
        }

        XrdsPlayerLocomotionMode::Smooth | XrdsPlayerLocomotionMode::Teleport => {
            // Kinematic grounded locomotion.
            // Gravity.
            if !vs.is_grounded {
                vs.velocity += GRAVITY * dt;
            }
            transform.translation.y += vs.velocity * dt;

            // Ground snap.
            if transform.translation.y <= vs.ground_y {
                transform.translation.y = vs.ground_y;
                vs.velocity     = 0.0;
                vs.is_grounded  = true;
            }

            // Jump.
            if vs.is_grounded && !egui_wants_input && keyboard.just_pressed(KeyCode::Space) {
                vs.velocity    = JUMP_IMPULSE;
                vs.is_grounded = false;
            }

            // Horizontal WASD — projected onto XZ so pitch doesn't affect height.
            if !egui_wants_input {
                let speed = if shift { PAWN_MOVE_SPEED * 2.0 } else { PAWN_MOVE_SPEED };
                let fwd3  = transform.rotation * -Vec3::Z;
                let rgt3  = transform.rotation *  Vec3::X;
                let forward = Vec3::new(fwd3.x, 0.0, fwd3.z).normalize_or_zero();
                let right   = Vec3::new(rgt3.x, 0.0, rgt3.z).normalize_or_zero();

                if keyboard.pressed(KeyCode::KeyW) { transform.translation += forward * speed * dt; }
                if keyboard.pressed(KeyCode::KeyS) { transform.translation -= forward * speed * dt; }
                if keyboard.pressed(KeyCode::KeyA) { transform.translation -= right   * speed * dt; }
                if keyboard.pressed(KeyCode::KeyD) { transform.translation += right   * speed * dt; }
            }
        }
    }
}

// ── Default scene ─────────────────────────────────────────────────────────────

fn default_scene() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Untitled".to_string(),
            ..Default::default()
        },
        nodes: vec![
            // Root group
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Scene".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata::default(),
            },
            // Default sun — 45° yaw / 45° pitch so PBR materials are lit from the start
            XrdsSceneNode {
                id: XrdsSceneNodeId(2),
                parent_id: None,
                name: "Sun".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    rotation_quat_xyzw: {
                        let r = xrds::editor::Quat::from_euler(
                            xrds::editor::EulerRot::YXZ,
                            45_f32.to_radians(),
                            -45_f32.to_radians(),
                            0.0,
                        );
                        [r.x, r.y, r.z, r.w]
                    },
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight {
                    color: [1.0, 1.0, 1.0, 1.0],
                    illuminance: 10_000.0,
                    shadows: false,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            // Sphere
            XrdsSceneNode {
                id: XrdsSceneNodeId(3),
                parent_id: Some(XrdsSceneNodeId(1)),
                name: "Sphere".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                    radius: 1.0,
                    material: XrdsSceneMaterial {
                        base_color: [0.3, 0.6, 1.0, 1.0],
                        pbr: XrdsSceneMaterialPbrParams {
                            roughness: 0.4,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        assets: Default::default(),
        gltf_node_authoring: Default::default(),
        version: Default::default(),
    }
}
