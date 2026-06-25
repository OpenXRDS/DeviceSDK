use std::sync::Arc;
use bevy::prelude::{App, IntoScheduleConfigs, PostUpdate, Update, resource_exists};
use xrds_runtime::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext, XrdsUpdateSystemSet};
use xrds_scene_graph::{
    XrdsEditorMetadata, XrdsSceneAmbientLight,
    XrdsSceneCube, XrdsSceneDirectionalLight, XrdsSceneDocument, XrdsSceneDocumentSession,
    XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsScenePlane3D, XrdsSceneMaterial,
    XrdsSceneTransform,
};
use crate::bevy_bridge::{
    BevyBridgeResource, broadcast_editor_snapshot_system, drain_editor_commands_system,
    sync_active_anchor_system,
};
use crate::bridge::EditorBridge;
use crate::editor_state::{EditorSession, EditorState};
use crate::viewport_camera::{
    EditorCameraState, StereoPreviewState,
    apply_camera_selection_system, orbit_camera_system, spawn_editor_camera,
    update_stereo_preview_camera,
};
use crate::wry_overlay::{
    ViewportRect, WryEditorReady,
    try_attach_wry_editor, push_snapshot_to_webview, drain_responses_and_viewport,
    handle_editor_resize, focus_viewport_on_click, force_exit_on_close,
};
use crate::viewport_gizmo::{
    floor_grid_system, fov_overlay_system, interaction_zone_gizmo_system, light_rays_system,
    physics_collider_gizmo_system, player_spawn_gizmo_system, spawn_zone_gizmo_system,
    transform_gizmo_system,
    update_selection_outline, GridGizmoGroup,
};
use crate::viewport_gizmo_interaction::gizmo_interaction_system;
use crate::viewport_player::{
    despawn_player_pawn_system, init_anchor_poses_system, pawn_locomotion_system,
    player_anchor_key_system, spawn_player_pawn_system, switch_player_anchor_system,
    sync_player_root_system,
};
use crate::viewport_selection::{viewport_delete_system, viewport_ray_selection};
use crate::keyboard_shortcuts::{keyboard_shortcut_system, raycast_debug_system};
use xrds_runtime::sdk::{XrdsColor, XrdsLinearRgba, XrdsMaterialParams, XrdsMaterialPbrParams};

const IDENTITY_ROT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const SUN_ROT: [f32; 4] = [-0.4226, 0.0, 0.0, 0.9063];

pub struct XrdsEditorTauriApp {
    bridge: Arc<EditorBridge>,
    initial_doc: Option<XrdsSceneDocument>,
}

impl XrdsEditorTauriApp {
    pub fn new(bridge: Arc<EditorBridge>) -> Self {
        Self {
            bridge,
            initial_doc: Some(build_default_document()),
        }
    }
}

impl XrdsApp for XrdsEditorTauriApp {
    fn configure(&mut self, app: &mut App) {
        let doc = self.initial_doc.as_ref().unwrap().clone();
        let session = XrdsSceneDocumentSession::new(doc)
            .expect("default document failed validation");

        use bevy::prelude::Startup;
        use bevy::gizmos::AppGizmoBuilder;
        use bevy_mod_outline::OutlinePlugin;

        app.insert_resource(BevyBridgeResource(Arc::clone(&self.bridge)));
        app.insert_resource(EditorSession(session));
        app.insert_resource(EditorState::default());
        app.insert_resource(EditorCameraState::default());
        app.insert_resource(ViewportRect::default());
        app.insert_resource(StereoPreviewState::default());

        app.add_plugins(OutlinePlugin);
        app.init_gizmo_group::<GridGizmoGroup>();

        app.add_systems(Startup, spawn_editor_camera);

        // Exit before Bevy destroys the parent HWND to avoid WebView2 COM teardown deadlock.
        app.add_systems(Update, force_exit_on_close);

        // Relay keyboard shortcuts to the inbound command queue when Bevy has focus.
        app.add_systems(Update, keyboard_shortcut_system);
        app.add_systems(Update, raycast_debug_system);

        // Wry editor overlay — try_attach_wry_editor is exclusive-world and retries
        // every frame until WINIT_WINDOWS has the window, then creates the WebView once.
        // After WryEditorReady is inserted, the resize + response-drain systems run.
        app.add_systems(Update, try_attach_wry_editor);
        app.add_systems(Update,
            (drain_responses_and_viewport, handle_editor_resize, focus_viewport_on_click)
                .run_if(resource_exists::<WryEditorReady>)
                .after(try_attach_wry_editor),
        );

        // Update systems
        // Ordering chain for anchor switching:
        //   drain → [spawn_pawn, player_anchor_keys] → sync_active_anchor → switch_anchor
        // init_anchor_poses runs after spawn_pawn (waits for pawn_entity to exist).
        app.add_systems(Update, (
            drain_editor_commands_system,
            apply_camera_selection_system,
            spawn_player_pawn_system.after(drain_editor_commands_system),
            despawn_player_pawn_system.after(drain_editor_commands_system),
            init_anchor_poses_system.after(spawn_player_pawn_system),
            pawn_locomotion_system,
            player_anchor_key_system.after(drain_editor_commands_system),
            sync_active_anchor_system
                .after(drain_editor_commands_system)
                .after(player_anchor_key_system),
            switch_player_anchor_system.after(sync_active_anchor_system),
            orbit_camera_system.after(drain_editor_commands_system),
            gizmo_interaction_system.after(drain_editor_commands_system),
            viewport_ray_selection.after(gizmo_interaction_system),
            viewport_delete_system.after(drain_editor_commands_system),
        ));

        // Sync Player entity transform from pawn BEFORE TransformPropagate so the
        // Player → PlayerAnchor → child hierarchy follows the pawn this frame.
        app.add_systems(
            PostUpdate,
            sync_player_root_system
                .before(bevy::transform::TransformSystems::Propagate),
        );

        // PostUpdate rendering systems
        app.add_systems(
            PostUpdate, (
                transform_gizmo_system,
                floor_grid_system,
                light_rays_system,
                interaction_zone_gizmo_system,
                player_spawn_gizmo_system,
                spawn_zone_gizmo_system,
                physics_collider_gizmo_system,
                fov_overlay_system,
                update_selection_outline,
                broadcast_editor_snapshot_system.after(XrdsUpdateSystemSet),
                push_snapshot_to_webview
                    .after(broadcast_editor_snapshot_system)
                    .run_if(resource_exists::<WryEditorReady>),
            )
        );

        // Stereo preview — runs in PostUpdate so it sees the final ViewportRect
        // and left-camera Transform from the Update schedule.
        app.add_systems(PostUpdate, update_stereo_preview_camera
            .before(bevy::transform::TransformSystems::Propagate));
    }

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let doc = self.initial_doc.take().unwrap();
        api.import_scene_document(&doc)
            .expect("failed to import default scene document");
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        // Read all pending state in one borrow, then drop it before mutating.
        let (
            needs_reimport,
            pending_translations,
            pending_rotations,
            pending_scale,
            pending_visible,
            pending_grabbable,
            pending_material,
            pending_point,
            pending_dir,
            pending_spot,
            pending_ambient,
            pending_extruded_color,
            pending_anchor_fov,
        ) = ctx.resource::<EditorState>().map(|s| (
            s.needs_full_reimport,
            s.pending_translations.clone(),
            s.pending_rotations.clone(),
            s.pending_scale,
            s.pending_visible,
            s.pending_grabbable,
            s.pending_material.clone(),
            s.pending_point_light,
            s.pending_directional_light,
            s.pending_spot_light,
            s.pending_ambient_light,
            s.pending_extruded_color,
            s.pending_anchor_fov,
        )).unwrap_or((false, vec![], vec![], None, None, None, None, None, None, None, None, None, None));

        if needs_reimport {
            // Commit any pending transforms to the document before reimporting.
            // Also clear them so entities respawned from the document start fresh.
            // Without this, a structural change (e.g. text depth edit) would
            // reset the entity to its last-saved position, discarding live preview.
            if !pending_translations.is_empty() || !pending_rotations.is_empty() || pending_scale.is_some() {
                if let Some(mut session) = ctx.resource_mut::<EditorSession>() {
                    let pt = pending_translations.clone();
                    let pr = pending_rotations.clone();
                    let _ = session.0.edit(|doc| {
                        for (id, v) in &pt {
                            if let Some(n) = doc.node_mut(*id) { n.transform.translation = *v; }
                        }
                        for (id, v) in &pr {
                            if let Some(n) = doc.node_mut(*id) { n.transform.rotation_quat_xyzw = *v; }
                        }
                        if let Some((id, v)) = pending_scale {
                            if let Some(n) = doc.node_mut(id) { n.transform.scale = v; }
                        }
                    });
                }
            }

            if let Some(doc) = ctx.resource::<EditorSession>().map(|s| s.0.document().clone()) {
                bevy::log::info!("[update] reimporting {} nodes", doc.nodes.len());
                match ctx.reimport_scene(&doc) {
                    Ok(ids) => bevy::log::info!("[update] reimport ok ({} entities)", ids.len()),
                    Err(e) => bevy::log::error!("[update] reimport failed: {:?}", e),
                }
            } else {
                bevy::log::error!("[update] no EditorSession found for reimport");
            }
        }

        // ── Scene environment sync ────────────────────────────────────────────
        // Applied every frame so undo/redo and file-open both update the runtime.
        if let Some(env) = ctx.resource::<EditorSession>().map(|s| {
            s.0.document().metadata.environment.clone()
        }) {
            match env {
                Some(e) => { let _ = ctx.set_scene_environment(e); }
                None    => { let _ = ctx.clear_scene_environment(); }
            }
        }
        if let Some(mut state) = ctx.resource_mut::<EditorState>() {
            state.needs_env_sync = false;
        }

        // ── Export Application: poll background build result ─────────────────
        let export_result = ctx.resource::<EditorState>()
            .and_then(|s| s.export_job.as_ref())
            .and_then(|j| j.result.lock().ok()?.take());

        if let Some(result) = export_result {
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.export_job = None;
                state.pending_status = Some(match result {
                    Ok(msg) => { bevy::log::info!("[export] {}", msg); msg }
                    Err(e)  => { bevy::log::error!("[export] {}", e); format!("Export failed: {e}") }
                });
            }
        }

        // ── Refresh GLTF clip names for all GltfAsset nodes ──────────────────
        {
            use xrds_scene_graph::XrdsSceneNodePayload;
            let gltf_ids: Vec<_> = ctx.resource::<EditorSession>()
                .map(|s| s.0.document().nodes.iter()
                    .filter_map(|n| matches!(n.payload, XrdsSceneNodePayload::GltfAsset(_))
                        .then_some(n.id))
                    .collect())
                .unwrap_or_default();
            let mut new_clips = std::collections::HashMap::new();
            for node_id in gltf_ids {
                let clips = ctx.gltf_clip_names(xrds_runtime::sdk::XrdsId::from(node_id));
                if !clips.is_empty() {
                    new_clips.insert(node_id, clips);
                }
            }
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.gltf_clips = new_clips;
            }
        }

        // ── GLTF animation commands from inspector ───────────────────────────
        let (pending_gltf_play, pending_gltf_stop) = ctx.resource::<EditorState>()
            .map(|s| (s.pending_gltf_play, s.pending_gltf_stop))
            .unwrap_or((None, None));

        if let Some((node_id, clip_index, speed)) = pending_gltf_play {
            use xrds_runtime::sdk::world::XrdsGltfAsset;
            use xrds_runtime::{XrdsGltfAnimationSelector, XrdsGltfAnimationPlaybackOptions};
            if let Some(handle) = ctx.handle_of::<XrdsGltfAsset>(xrds_runtime::sdk::XrdsId::from(node_id)) {
                let mut opts = XrdsGltfAnimationPlaybackOptions::default();
                opts.speed = speed;
                let _ = ctx.play_gltf_animation(&handle, XrdsGltfAnimationSelector::Index(clip_index), opts);
            }
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_gltf_play = None;
            }
        }
        if let Some(node_id) = pending_gltf_stop {
            use xrds_runtime::sdk::world::XrdsGltfAsset;
            if let Some(handle) = ctx.handle_of::<XrdsGltfAsset>(xrds_runtime::sdk::XrdsId::from(node_id)) {
                let _ = ctx.stop_gltf_animation(&handle);
            }
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_gltf_stop = None;
            }
        }

        // ── Play mode: start GLB animations on the first play frame ─────────
        let play_started = ctx.resource::<EditorState>()
            .map(|s| s.play_started)
            .unwrap_or(false);

        if play_started {
            use xrds_runtime::sdk::world::XrdsGltfAsset;
            use xrds_runtime::{XrdsGltfAnimationSelector, XrdsGltfAnimationPlaybackOptions};
            let gltf_ids: Vec<_> = ctx.resource::<EditorSession>()
                .map(|s| s.0.document().nodes.iter()
                    .filter_map(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::GltfAsset(_))
                        .then_some(n.id))
                    .collect())
                .unwrap_or_default();
            for node_id in gltf_ids {
                if let Some(handle) = ctx.handle_of::<XrdsGltfAsset>(xrds_runtime::sdk::XrdsId::from(node_id)) {
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

        // ── Transform live preview ───────────────────────────────────────────
        for (id, t) in &pending_translations {
            ctx.set_translation_for_node((*id).into(), *t);
        }
        for (id, r) in &pending_rotations {
            ctx.set_rotation_for_node((*id).into(), *r);
        }
        if let Some((id, s)) = pending_scale {
            ctx.set_scale_for_node(id.into(), s);
        }
        if let Some((id, v)) = pending_visible {
            ctx.set_visible_for_node(id.into(), v);
        }
        if let Some((id, g)) = pending_grabbable {
            if g { ctx.make_grabbable(id.into()); } else { ctx.make_ungrabable(id.into()); }
        }

        // ── Material live preview ────────────────────────────────────────────
        if let Some((id, ref dto)) = pending_material {
            let params = XrdsMaterialParams {
                base_color: XrdsColor { rgba: dto.base_color },
                emissive: XrdsLinearRgba { rgba: [dto.emissive[0], dto.emissive[1], dto.emissive[2], 1.0] },
                opacity: 1.0,
                unlit: false,
                pbr: XrdsMaterialPbrParams {
                    metallic: dto.metallic,
                    roughness: dto.roughness,
                    ..Default::default()
                },
                textures: Default::default(),
            };
            ctx.set_material_params_for_node(id.into(), params);
        }

        // ── Light live preview ───────────────────────────────────────────────
        if let Some((id, color, intensity, range)) = pending_point {
            ctx.set_point_light_params_for_node(id.into(), color, intensity, range);
        }
        if let Some((id, color, illuminance)) = pending_dir {
            ctx.set_directional_light_params_for_node(id.into(), color, illuminance);
        }
        if let Some((id, color, intensity, range, inner, outer)) = pending_spot {
            ctx.set_spot_light_params_for_node(id.into(), color, intensity, range, inner, outer);
        }
        if let Some((color, brightness)) = pending_ambient {
            ctx.set_ambient_light_params(color, brightness);
        }

        // ExtrudedText color in-place (no reimport — StandardMaterial update only)
        if let Some((id, color)) = pending_extruded_color {
            ctx.set_extruded_text_color_for_node(id.into(), color);
        }

        // Camera FOV live preview
        let pending_cam = ctx.resource::<EditorState>().and_then(|s| s.pending_camera);
        if let Some((id, fov)) = pending_cam {
            ctx.set_camera_fov_for_node(id.into(), fov);
        }

        // PlayerAnchor FOV live preview (updates XrdsAnchorFov component for overlay)
        if let Some((id, fov)) = pending_anchor_fov {
            ctx.set_anchor_fov_for_node(id.into(), fov);
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_anchor_fov = None;
            }
        }

        // Physics live preview — gravity scale and mass update ECS components directly
        let pending_gs = ctx.resource::<EditorState>().and_then(|s| s.pending_gravity_scale);
        if let Some((id, scale)) = pending_gs {
            ctx.set_gravity_scale_for_node(id.into(), scale);
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_gravity_scale = None;
            }
        }
        let pending_m = ctx.resource::<EditorState>().and_then(|s| s.pending_mass);
        if let Some((id, mass)) = pending_m {
            ctx.set_mass_for_node(id.into(), mass);
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_mass = None;
            }
        }

        // Keep physics paused outside play mode so Dynamic objects stay at authored positions.
        let is_playing = ctx.resource::<EditorState>().map(|s| s.is_playing).unwrap_or(false);
        ctx.set_physics_paused(!is_playing);

        // Clear all pending state
        if let Some(mut state) = ctx.resource_mut::<EditorState>() {
            state.needs_full_reimport = false;
            // pending_translations / pending_rotations are NOT cleared here.
            // They persist until cleared explicitly by:
            //   - gizmo drag commit (clear_pending_translations/rotations)
            //   - CommitTransform command (per-node clear in inspector.rs)
            //   - Undo/Redo/NewScene/OpenScene (clear all in io.rs)
            //   - Full reimport (clear on reimport trigger)
            state.pending_scale = None;
            state.pending_visible = None;
            state.pending_grabbable = None;
            state.pending_material = None;
            // Light pending state is NOT cleared here — CommitLight reads it on the next frame.
            // (Same reasoning as pending_translations: drain runs in Update, update() in PostUpdate.)
            state.pending_extruded_color = None;
            // pending_camera is cleared by CommitCameraParams handler
        }
    }
}

pub fn run_bevy_viewport(bridge: Arc<EditorBridge>) {
    // CARGO_MANIFEST_DIR = apps/xrds-editor/src-tauri
    // Workspace assets live three levels up at <workspace_root>/assets.
    let asset_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets")
        .to_string_lossy()
        .into_owned();

    Runtime::new(RuntimeParameters {
        app_name: "XRDS Editor".to_owned(),
        asset_path: Some(asset_path),
        allow_unapproved_paths: true,
        window_resolution: Some((1600.0, 1000.0)),
        ..Default::default()
    })
    .run_xrds(XrdsEditorTauriApp::new(bridge))
    .expect("3D viewport error");
}

// ---------------------------------------------------------------------------
// Default scene document
// ---------------------------------------------------------------------------

fn scene_node(
    id: u64,
    parent_id: Option<u64>,
    name: &str,
    translation: [f32; 3],
    rotation: [f32; 4],
    payload: XrdsSceneNodePayload,
) -> XrdsSceneNode {
    XrdsSceneNode {
        id: XrdsSceneNodeId(id),
        parent_id: parent_id.map(XrdsSceneNodeId),
        name: name.to_string(),
        enabled: true,
        visible: true,
        grabbable: false,
        transform: XrdsSceneTransform {
            translation,
            rotation_quat_xyzw: rotation,
            scale: [1.0, 1.0, 1.0],
        },
        payload,
        editor: XrdsEditorMetadata::default(),
    }
}

pub(crate) fn build_default_document() -> XrdsSceneDocument {
    let mut doc = XrdsSceneDocument::default();
    doc.metadata.name = "Untitled Scene".to_string();

    doc.nodes.push(scene_node(
        1, None, "Ambient Light",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::AmbientLight(XrdsSceneAmbientLight {
            color: [1.0, 0.95, 0.9, 1.0],
            brightness: 200.0,
            ..Default::default()
        }),
    ));

    doc.nodes.push(scene_node(
        2, None, "Sun",
        [0.0, 5.0, -3.0], SUN_ROT,
        XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight {
            color: [1.0, 0.98, 0.92, 1.0],
            illuminance: 10_000.0,
            shadows: true,
        }),
    ));

    doc.nodes.push(scene_node(
        3, None, "Ground",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
            size: [20.0, 20.0],
            material: XrdsSceneMaterial {
                base_color: [0.55, 0.55, 0.55, 1.0],
                ..Default::default()
            },
            ..Default::default()
        }),
    ));

    doc.nodes.push(scene_node(
        4, None, "Cube",
        [0.0, 0.5, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube::default()),
    ));

    // Camera node intentionally omitted from the default document.
    // The editor camera (EditorCameraMarker) handles the viewport view.
    // Scene cameras can be added by the user via the palette — they are
    // kept inactive in the editor by deactivate_scene_cameras.

    doc
}
