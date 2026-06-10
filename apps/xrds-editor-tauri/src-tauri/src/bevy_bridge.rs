use std::sync::Arc;
use bevy::prelude::*;
use tauri::Emitter;
use crate::bridge::{CameraNodeDto, EditorBridge, EditorCommand, EditorSnapshot, PlayerAnchorNodeDto};
use xrds_scene_graph::{XrdsSceneDocument, XrdsSceneNodePayload};
use xrds_runtime::{ActivePlayerAnchorEntity, XrdsIdIndex};
use crate::editor_state::{EditorSession, EditorState};
use crate::environment::{apply_environment_command, build_environment_dto};
use crate::hierarchy::{apply_hierarchy_command, build_hierarchy};
use crate::inspector::{apply_inspector_command, build_node_inspector};
use crate::io::apply_io_command;
use crate::palette::{apply_palette_command, build_asset_catalog};
use crate::toolbar::apply_toolbar_command;

/// Bevy resource that holds the shared bridge channels.
#[derive(Resource)]
pub struct BevyBridgeResource(pub Arc<EditorBridge>);

// ---------------------------------------------------------------------------
// Inbound drain — runs each Update frame
// ---------------------------------------------------------------------------

pub fn drain_editor_commands_system(
    bridge: Res<BevyBridgeResource>,
    mut session: ResMut<EditorSession>,
    mut state: ResMut<EditorState>,
) {
    let commands: Vec<EditorCommand> = {
        let mut q = bridge.0.inbound.lock().unwrap();
        q.drain(..).collect()
    };

    for cmd in &commands {
        let needs_reimport =
            apply_hierarchy_command(cmd, &mut session, &mut state) ||
            apply_palette_command(cmd, &mut session, &mut state) ||
            apply_inspector_command(cmd, &mut session, &mut state) ||
            apply_io_command(cmd, &mut session, &mut state);
        let env_changed = apply_environment_command(cmd, &mut session, &mut state);
        if env_changed { state.needs_env_sync = true; }
        let toolbar_reimport = apply_toolbar_command(cmd, &mut session, &mut state);
        if toolbar_reimport { state.needs_full_reimport = true; }
        if needs_reimport {
            state.needs_full_reimport = true;
        }
        // Live-preview commands (SetTranslation, SetMaterial, SetPointLight, etc.) fire
        // every slider tick — logged at trace! so they don't flood the terminal.
        // Structural operations (spawn, delete, save, undo, etc.) are logged at info!
        // by their individual handlers in hierarchy.rs / io.rs / palette.rs.
        if is_structural_command(cmd) {
            info!("[bridge] {:?}", cmd);
        } else {
            bevy::log::trace!("[bridge] {:?}", cmd);
        }
    }
}

// ---------------------------------------------------------------------------
// Outbound broadcaster — runs each PostUpdate frame
// ---------------------------------------------------------------------------

pub fn broadcast_editor_snapshot_system(
    bridge: Res<BevyBridgeResource>,
    session: Res<EditorSession>,
    mut state: ResMut<EditorState>,
) {
    let doc = session.0.document();

    let selection_ids: Vec<u64> = state.selection.ids().iter().map(|id| id.0).collect();
    let snapshot = EditorSnapshot {
        hierarchy: build_hierarchy(doc),
        selection: selection_ids.clone(),
        selected_node: build_node_inspector(doc, state.selection.ids(), &state.gltf_clips),
        asset_catalog: build_asset_catalog(doc),
        undo_count: session.0.undo_count(),
        redo_count: session.0.redo_count(),
        is_dirty: session.0.is_dirty(),
        scene_name: doc.metadata.name.clone(),
        status_message: state.pending_status.take(),
        gizmo_mode: format!("{:?}", state.gizmo_mode),
        camera_mode: format!("{:?}", state.camera_mode),
        show_grid: state.show_grid,
        show_fov_overlay: state.show_fov_overlay,
        is_playing: state.is_playing,
        snap_step: state.snap_step,
        is_exporting: state.export_job.is_some(),
        has_clipboard: state.clipboard.is_some(),
        environment: build_environment_dto(&session),
        available_cameras: doc.nodes.iter()
            .filter(|n| matches!(n.payload, XrdsSceneNodePayload::Camera(_)))
            .map(|n| CameraNodeDto { id: n.id.0, name: n.name.clone() })
            .collect(),
        active_camera_id: state.active_camera_id.map(|n| n.0),
        player_anchors: build_player_anchor_list(doc),
        active_player_anchor_id: state.active_player_anchor_id.map(|id| id.0),
    };

    bridge.0.outbound.lock().unwrap().push_back(snapshot);
}

/// Returns `true` for commands that structurally change the scene or session —
/// these are logged at `info!`. Live-preview commands that fire every frame are
/// logged at `trace!` instead so they don't flood the terminal.
fn is_structural_command(cmd: &EditorCommand) -> bool {
    use EditorCommand::*;
    matches!(cmd,
        // Scene structure
        SpawnPrimitive{..} | SpawnAsset{..} | DeleteNode{..} | DeleteSelection |
        DuplicateNode{..} | DuplicateSelection | ReparentNode{..} |
        RenameNode{..} | CopySelection | CutSelection | PasteClipboard |
        // File I/O
        NewScene | OpenScene{..} | SaveScene | SaveSceneAs{..} |
        ImportAsset{..} | ExportGlb{..} | ExportApplication{..} |
        RemoveAsset{..} |
        // History
        Undo | Redo |
        // Play mode / camera / player
        SetPlayMode{..} | PlayGltfAnimation{..} | StopGltfAnimation{..} | SetActiveCamera{..} |
        SetActivePlayerAnchor{..} |
        // Text structural (requires reimport)
        SetTextContent{..} | SetExtrudedText{..}
    )
}

// ---------------------------------------------------------------------------
// Player anchor list builder
// ---------------------------------------------------------------------------

fn build_player_anchor_list(doc: &XrdsSceneDocument) -> Vec<PlayerAnchorNodeDto> {
    doc.nodes.iter()
        .filter(|n| matches!(n.payload, XrdsSceneNodePayload::PlayerAnchor(_)))
        .map(|n| {
            let player_name = n.parent_id
                .and_then(|pid| doc.node(pid))
                .filter(|p| matches!(p.payload, XrdsSceneNodePayload::Player(_)))
                .map(|p| p.name.clone())
                .unwrap_or_default();
            PlayerAnchorNodeDto { id: n.id.0, name: n.name.clone(), player_name }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Active anchor sync — translates EditorState node ID → Bevy Entity each frame
// ---------------------------------------------------------------------------

pub fn sync_active_anchor_system(
    state: Res<EditorState>,
    id_index: Res<XrdsIdIndex>,
    mut active: ResMut<ActivePlayerAnchorEntity>,
) {
    let new = state.active_player_anchor_id
        .and_then(|id| id_index.entity_of(id.into()));
    if active.0 != new {
        active.0 = new;
    }
}

// ---------------------------------------------------------------------------
// Tauri emitter — async task draining outbound channel → webview events
// ---------------------------------------------------------------------------

pub fn spawn_snapshot_emitter(app_handle: tauri::AppHandle, bridge: Arc<EditorBridge>) {
    let outbound = Arc::clone(&bridge.outbound);
    tauri::async_runtime::spawn(async move {
        loop {
            let latest = {
                let mut q = outbound.lock().unwrap();
                let mut latest = None;
                while let Some(snap) = q.pop_front() {
                    latest = Some(snap);
                }
                latest
            };

            if let Some(snapshot) = latest {
                let _ = app_handle.emit("editor_state", snapshot);
            }

            tokio::time::sleep(std::time::Duration::from_millis(16)).await;
        }
    });
}
