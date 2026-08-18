use xrds_scene_graph::XrdsSceneDocumentSession;
use crate::bridge::EditorCommand;
use crate::editor_state::{CameraMode, EditorSession, EditorState, GizmoMode};

/// Handle toolbar / viewport-settings commands. Returns true if a full reimport is needed.
pub fn apply_toolbar_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    state: &mut EditorState,
) -> bool {
    match cmd {
        EditorCommand::SetGizmoMode { mode } => {
            state.gizmo_mode = match mode.as_str() {
                "Rotate" => GizmoMode::Rotate,
                "Scale"  => GizmoMode::Scale,
                _        => GizmoMode::Translate,
            };
            false
        }
        EditorCommand::SetCameraMode { mode } => {
            state.camera_mode = match mode.as_str() {
                "Fly"  => CameraMode::Fly,
                _      => CameraMode::Orbit,
            };
            false
        }
        EditorCommand::ToggleGrid => {
            state.show_grid = !state.show_grid;
            false
        }
        EditorCommand::ToggleFovOverlay => {
            state.show_fov_overlay = !state.show_fov_overlay;
            false
        }
        EditorCommand::FrameSelected => {
            state.frame_selected_target = Some([0.0, 0.0, 0.0]);
            false
        }
        EditorCommand::SetPlayMode { playing } => {
            if *playing && !state.is_playing {
                start_play(session, state);
            } else if !*playing && state.is_playing {
                return stop_play(session, state); // returns true (needs reimport)
            }
            false
        }
        EditorCommand::SetActivePlayerAnchor { id } => {
            state.active_player_anchor_id = id.map(xrds_scene_graph::XrdsSceneNodeId);
            false
        }
        EditorCommand::PreviewFromAnchor { id } => {
            state.preview_anchor_target = Some(xrds_scene_graph::XrdsSceneNodeId(*id));
            false
        }
        _ => false,
    }
}

pub fn start_play(session: &EditorSession, state: &mut EditorState) {
    state.play_snapshot = Some(session.0.document().clone());
    state.is_playing    = true;
    state.play_started  = true;
    state.pending_status = Some("Playing — Esc to stop".into());
}

pub fn stop_play(session: &mut EditorSession, state: &mut EditorState) -> bool {
    let Some(snapshot) = state.play_snapshot.take() else {
        state.is_playing = false;
        return false;
    };
    match XrdsSceneDocumentSession::new(snapshot) {
        Ok(new_session) => {
            session.0 = new_session;
            state.is_playing = false;
            state.active_player_anchor_id = None;
            state.selection.clear();
            state.clear_pending_translations();
            state.clear_pending_rotations();
            state.pending_scale = None;
            state.gizmo_drag    = None;
            state.pending_status = Some("Stopped.".into());
            true // needs full reimport to restore scene
        }
        Err(e) => {
            state.is_playing = false;
            state.pending_status = Some(format!("Stop failed: {e:?}"));
            false
        }
    }
}
