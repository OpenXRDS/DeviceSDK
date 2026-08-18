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
#[cfg(target_os = "linux")]
use crate::wry_overlay::pump_gtk_events;
use crate::viewport_gizmo::{
    floor_grid_system, fov_overlay_system, interaction_zone_gizmo_system, light_rays_system,
    physics_collider_gizmo_system, player_spawn_gizmo_system, spawn_zone_gizmo_system,
    transform_gizmo_system,
    update_selection_outline, GridGizmoGroup,
};
use crate::viewport_gizmo_interaction::gizmo_interaction_system;
use crate::play_pointer::mouse_world_ui_input_system;
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

        // Play-mode mouse → world-UI pointer bridge. PreUpdate (after input processing)
        // so the runtime's Update-scheduled world-UI systems read fresh state.
        app.add_systems(bevy::app::PreUpdate,
            mouse_world_ui_input_system.after(bevy::input::InputSystems));
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
        // Pump the GLib/GTK event loop so webkit2gtk can paint — Linux only.
        #[cfg(target_os = "linux")]
        app.add_systems(Update,
            pump_gtk_events
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

            if let Some((mut doc, save_path)) = ctx.resource::<EditorSession>()
                .map(|s| (s.0.document().clone(), s.0.save_path().map(|p| p.to_path_buf())))
            {
                if let Some(scene_dir) = save_path.as_deref().and_then(|p| p.parent()) {
                    absolutize_doc_asset_uris(&mut doc, scene_dir);
                }
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

        // ── Trigger-action preview (PreviewFireTrigger from the Inspector) ────
        // No real ZoneEnter/Grabbed/etc event exists to wait for in a desktop
        // editor, so this is the only way an authored binding ever fires here.
        let pending_fire_trigger = ctx.resource::<EditorState>()
            .and_then(|s| s.pending_fire_trigger.clone());
        if let Some((node_id, kind, hand)) = pending_fire_trigger {
            let _ = ctx.fire_trigger(xrds_runtime::sdk::XrdsId::from(node_id), &kind, hand);
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_fire_trigger = None;
            }
        }


        // ── Track preview transport ───────────────────────────────────────────
        // Independent of play mode: previewing one Track is not running the
        // simulation. `advance_tracks` is not gated on `is_playing`, so a
        // spawned agent advances on its own — no play mode needed.
        let pending_preview = ctx.resource::<EditorState>()
            .and_then(|s| s.pending_track_preview.clone());
        if let Some(request) = pending_preview {
            use crate::editor_state::TrackPreviewRequest as Req;
            match request {
                Req::Play(name) => {
                    // Restore this Track's own nodes from the document *before*
                    // starting it fresh. Covers both restart cases: (a) it is
                    // still running and gets stopped-then-restarted inside
                    // `preview_play_track`, and (b) it already finished
                    // naturally, so no live agent exists to report what it
                    // touched — the document's own asset rows are what we fall
                    // back to in that case, since they name the same nodes.
                    // Play restores everything, effects included: a fresh run
                    // must not inherit a trail left running by the last one.
                    restore_track_nodes_from_document(ctx, &name, RestoreScope::Everything);
                    if ctx.preview_play_track(&name) {
                        if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                            state.track_preview_name = Some(name);
                        }
                    } else if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                        // Refused or empty. The runtime already logged why; the
                        // status message is what the author actually sees.
                        state.track_preview_name = None;
                        state.pending_status = Some(format!(
                            "Could not preview \"{name}\" — it has no events on a live node, or \
                             another Track already holds its assets."
                        ));
                    }
                }

                Req::Pause(paused) => {
                    ctx.preview_pause_track(paused);
                }

                Req::Stop => {
                    let name = ctx.resource::<EditorState>().and_then(|s| s.track_preview_name.clone());
                    ctx.preview_stop_track();
                    // The document is the authority on where a previewed
                    // Track's nodes belong; restore from it by name rather than
                    // from what the runtime reports it touched, so material
                    // (which the runtime's lock table has no notion of) is
                    // restored too, not just transform/visibility.
                    if let Some(name) = &name {
                        // Explicit Stop means "clean up", so effects stop too.
                        restore_track_nodes_from_document(ctx, name, RestoreScope::Everything);
                    }
                    if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                        state.track_preview_name = None;
                    }
                }
            }

            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_track_preview = None;
            }
        }

        // Mirror the live preview into EditorState each frame, so the snapshot
        // builder (which has no world access) can report it. This is what makes
        // the transport timecode and the playhead move.
        let preview_state = ctx.track_preview_state();
        // Same for the conflict readout, resolving entities to node names so the
        // message can say "crane_arm" rather than an opaque entity id.
        let conflict = ctx
            .resource::<xrds_runtime::xrds_api::trigger_action::XrdsTrackAssetLocks>()
            .and_then(|locks| locks.last_conflict.clone())
            .map(|c| {
                let names: Vec<String> = c
                    .contended
                    .iter()
                    .map(|e| format!("{e:?}"))
                    .collect();
                crate::bridge::TrackConflictDto {
                    blocked_track: c.blocked_track,
                    contended: names,
                }
            });
        // A preview that ran to its end leaves no agent behind, so
        // `track_preview_state()` goes None while `track_preview_name` is still
        // set. That is the "time is over" moment: honour each row's
        // `When Finished`, soft-stopping effects so live particles fade.
        //
        // This is a behaviour change: previously a finished Track's end state
        // persisted until the next Play (which restores first) or an explicit
        // Stop, so an author could inspect where it left things. That was
        // defensible while every action produced a *static* end state, but
        // PlayEffect on a Trail leaves emission running indefinitely — CPU spent
        // and the view spammed, with the cleanup button greyed out. Resetting on
        // completion also matches what every other timeline tool does. Pausing
        // near the end still lets an author inspect a final pose, so nothing is
        // actually lost.
        let finished_preview = if preview_state.is_none() {
            ctx.resource::<EditorState>()
                .and_then(|s| s.track_preview_name.clone())
        } else {
            None
        };
        if let Some(name) = finished_preview {
            restore_track_nodes_from_document(ctx, &name, RestoreScope::OnCompletion);
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.track_preview_name = None;
            }
        }

        if let Some(mut state) = ctx.resource_mut::<EditorState>() {
            state.track_preview = preview_state.map(|(name, elapsed, duration, playing)| {
                crate::bridge::TrackPreviewDto {
                    name,
                    elapsed_secs: elapsed,
                    duration_secs: duration,
                    playing,
                }
            });
            state.track_conflict = conflict;
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
            // Merge the dragged fields over the *authored* material rather than
            // building a fresh one: the DTO carries only these four, so a fresh
            // struct drops textures/opacity/unlit and the extra PBR fields for
            // the whole drag — they would visibly vanish and then pop back on
            // commit. Same reasoning as `CommitMaterial` in inspector.rs.
            let authored = ctx
                .resource::<EditorSession>()
                .and_then(|s| s.0.document().node_material(id).ok().cloned());
            let mut params: XrdsMaterialParams = authored
                .map(Into::into)
                .unwrap_or_else(|| XrdsMaterialParams {
                    base_color: XrdsColor { rgba: [1.0, 1.0, 1.0, 1.0] },
                    emissive: XrdsLinearRgba { rgba: [0.0, 0.0, 0.0, 1.0] },
                    opacity: 1.0,
                    unlit: false,
                    pbr: XrdsMaterialPbrParams::default(),
                    textures: Default::default(),
                });
            params.base_color = XrdsColor { rgba: dto.base_color };
            params.emissive =
                XrdsLinearRgba { rgba: [dto.emissive[0], dto.emissive[1], dto.emissive[2], 1.0] };
            params.pbr.metallic = dto.metallic;
            params.pbr.roughness = dto.roughness;
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
        let pending_cg = ctx.resource::<EditorState>().and_then(|s| s.pending_capsule_geometry);
        if let Some((id, radius, length)) = pending_cg {
            ctx.set_capsule_geometry_for_node(
                id.into(),
                xrds_components::CapsuleGeometryParams { radius, length },
            );
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_capsule_geometry = None;
            }
        }

        let pending_fx = ctx
            .resource::<EditorState>()
            .and_then(|s| s.pending_effect_params.clone());
        if let Some((id, fx)) = pending_fx {
            ctx.set_effect_params_for_node(id.into(), effect_params_from_scene(&fx));
            if let Some(mut state) = ctx.resource_mut::<EditorState>() {
                state.pending_effect_params = None;
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

/// Rewrite relative asset URIs in a document clone to absolute paths anchored
/// at the opened scene's directory, so the runtime loads the files that sit
/// next to the scene file rather than whatever happens to share a name under
/// the editor's own asset root.
///
/// Only used on the transient copy passed to reimport — the session document
/// (and anything saved to disk) keeps its portable relative URIs.
///
/// Search order per URI: `<scene_dir>/<uri>`, then `<scene_dir>/assets/<uri>`
/// (the layout produced by Export Application / dev-mode pushes).
/// Puts every `Node`-target asset row of Track `track_name` back to whatever
/// the document authors, covering transform, visibility, *and* material.
///
/// Deliberately keyed by the Track's own authored rows rather than by asking
/// the runtime what a live preview agent touched: that also works once the
/// agent is already gone (a Track that finished on its own, or was already
/// stopped), which is exactly the state the Sequencer's restart button needs
/// to recover from. `SelfNode`/`TriggerSource` rows are skipped — a preview
/// resolves those to a stand-in that is always also a `Node` row in the same
/// Track (see `preview_play_track_in_world`), so restoring the `Node` rows
/// covers them too.
/// Scene-format effect payload -> runtime `EffectParams`.
///
/// Shared by the Inspector's live preview and by preview-stop restoration, which
/// both need the same translation. Kept as one function so the two cannot drift.
fn effect_params_from_scene(fx: &xrds_scene_graph::XrdsSceneEffect) -> xrds_components::EffectParams {
    xrds_components::EffectParams {
        kind: match fx.kind {
            xrds_scene_graph::XrdsSceneEffectKind::Burst => {
                xrds_components::primitives::XrdsEffectKind::Burst
            }
            xrds_scene_graph::XrdsSceneEffectKind::Trail => {
                xrds_components::primitives::XrdsEffectKind::Trail
            }
        },
        auto_play: fx.auto_play,
        burst_count: fx.burst_count,
        spawn_rate: fx.spawn_rate,
        lifetime_secs: fx.lifetime_secs,
        size_min: fx.size_min,
        size_max: fx.size_max,
        color_start: xrds_components::XrdsColor { rgba: fx.color_start },
        color_end: xrds_components::XrdsColor { rgba: fx.color_end },
        speed_min: fx.speed_min,
        speed_max: fx.speed_max,
        omnidirectional: fx.omnidirectional,
        spread_deg: fx.spread_deg,
        gravity: fx.gravity,
        emission_radius: fx.emission_radius,
        blend: match fx.blend {
            xrds_scene_graph::XrdsSceneEffectBlend::Blend => {
                xrds_components::primitives::XrdsEffectBlend::Blend
            }
            xrds_scene_graph::XrdsSceneEffectBlend::Add => {
                xrds_components::primitives::XrdsEffectBlend::Add
            }
            xrds_scene_graph::XrdsSceneEffectBlend::Multiply => {
                xrds_components::primitives::XrdsEffectBlend::Multiply
            }
        },
        size_end: fx.size_end,
        drag: fx.drag,
        fade_edge: fx.fade_edge,
        fade_scene: fx.fade_scene,
    }
}

/// Why a Track's assets are being put back, which decides how much to undo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RestoreScope {
    /// Every row, unconditionally — the author pressed Stop, or a new run is
    /// starting and must not inherit the last one's leftovers. Effects are reset
    /// to their authored params, which also clears any live particles.
    Everything,
    /// The Track reached its end on its own. Honours each row's
    /// `XrdsWhenFinished`, and stops effects *softly* so particles already in
    /// flight fade instead of vanishing.
    OnCompletion,
}

/// Put a Track's assets back where the document says they belong.
///
/// Rows opting into `XrdsWhenFinished::Keep` are skipped on completion — that is
/// the author saying "leave what this Track did in place", modelled on Unreal
/// Sequencer's per-track `When Finished`. It is deliberately ignored for
/// `Everything`: Stop means reset, and a fresh run needs a clean slate.
///
/// Effects are handled apart from transform/visibility/material because "undo an
/// effect" has two very different meanings. On completion the right one is a soft
/// stop (cease emitting, let live particles fade) — re-applying params instead
/// would rebuild `ParticleSpawner`, and bevy_firework discards live particles on
/// any spawner change, so a burst fired near the end would blink out. This also
/// removes what used to be a hard-coded "never restore effects on completion"
/// exception: the behaviour is now the author's choice, and visible in the UI.
fn restore_track_nodes_from_document(
    ctx: &mut XrdsUpdateContext<'_>,
    track_name: &str,
    scope: RestoreScope,
) {
    struct Row {
        id: u64,
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
        visible: bool,
        material: Option<XrdsSceneMaterial>,
        effect: Option<xrds_scene_graph::XrdsSceneEffect>,
    }

    let rows: Vec<Row> = ctx
        .resource::<EditorSession>()
        .and_then(|session| {
            let doc = session.0.document();
            let named = doc.track(track_name)?;
            Some(
                named
                    .track
                    .assets
                    .iter()
                    .filter(|asset| {
                        scope == RestoreScope::Everything
                            || asset.when_finished == xrds_scene_graph::XrdsWhenFinished::Restore
                    })
                    .filter_map(|asset| match asset.target {
                        xrds_scene_graph::XrdsActionTarget::Node(id) => {
                            let node = doc.node(id)?;
                            Some(Row {
                                id: node.id.0,
                                translation: node.transform.translation,
                                rotation: node.transform.rotation_quat_xyzw,
                                scale: node.transform.scale,
                                visible: node.visible,
                                material: doc.node_material(id).ok().cloned(),
                                effect: match &node.payload {
                                    XrdsSceneNodePayload::Effect(fx) => Some(fx.clone()),
                                    _ => None,
                                },
                            })
                        }
                        _ => None,
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();

    for row in rows {
        let id = xrds_runtime::sdk::XrdsId(row.id);

        if let Some(fx) = &row.effect {
            match scope {
                RestoreScope::Everything => {
                    ctx.set_effect_params_for_node(id, effect_params_from_scene(fx));
                }
                RestoreScope::OnCompletion => {
                    ctx.stop_effect_for_node(id);
                }
            }
            // An effect node has no material, and its transform is restored below
            // like any other node's.
        }

        ctx.set_translation_for_node(id, row.translation);
        ctx.set_rotation_for_node(id, row.rotation);
        ctx.set_scale_for_node(id, row.scale);
        ctx.set_visible_for_node(id, row.visible);
        if let Some(material) = row.material {
            ctx.set_material_params_for_node(id, material.into());
        }
    }
}

fn absolutize_doc_asset_uris(doc: &mut XrdsSceneDocument, scene_dir: &std::path::Path) {
    let resolve = |uri: &str| -> Option<String> {
        if uri.is_empty() || std::path::Path::new(uri).is_absolute() {
            return None;
        }
        [scene_dir.join(uri), scene_dir.join("assets").join(uri)]
            .into_iter()
            .find(|c| c.is_file())
            .map(|c| c.to_string_lossy().replace('\\', "/"))
    };

    for asset in &mut doc.assets {
        if let Some(abs) = resolve(&asset.uri) {
            asset.uri = abs;
        }
    }
    for node in &mut doc.nodes {
        if let XrdsSceneNodePayload::GltfAsset(gltf) = &mut node.payload {
            if let Some(abs) = resolve(&gltf.asset_uri) {
                gltf.asset_uri = abs;
            }
        }
    }
}

pub fn run_bevy_viewport(bridge: Arc<EditorBridge>) {
    // CARGO_MANIFEST_DIR = apps/xrds-editor/src-tauri
    // Workspace assets live three levels up at <workspace_root>/assets.
    // Normalize away the `..` segments so the runtime can compare authored
    // absolute asset paths against this root textually.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let asset_path = manifest_dir
        .ancestors()
        .nth(3)
        .unwrap_or(&manifest_dir)
        .join("assets")
        .to_string_lossy()
        .into_owned();

    Runtime::new(RuntimeParameters {
        app_name: "XRDS Editor".to_owned(),
        asset_path: Some(asset_path),
        allow_unapproved_paths: true,
        window_resolution: Some((1920.0, 1080.0)),
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
        triggers: Vec::new(),
        watchers: Vec::new(),
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
