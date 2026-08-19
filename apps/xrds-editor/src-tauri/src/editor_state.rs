use bevy::prelude::Resource;
use xrds_scene_graph::{XrdsSceneDocumentSession, XrdsSceneNodeId};
use crate::bridge::{ApkPrerequisite, MaterialParamsDto};

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct EditorSession(pub XrdsSceneDocumentSession);

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// Ordered multi-node selection. Last entry = primary (gizmo anchor, inspector target).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    nodes: Vec<XrdsSceneNodeId>,
}

impl Selection {
    pub fn primary(&self) -> Option<XrdsSceneNodeId> { self.nodes.last().copied() }
    pub fn contains(&self, id: XrdsSceneNodeId) -> bool { self.nodes.contains(&id) }
    pub fn set_single(&mut self, id: XrdsSceneNodeId) { self.nodes.clear(); self.nodes.push(id); }
    pub fn clear(&mut self) { self.nodes.clear(); }
    pub fn add(&mut self, id: XrdsSceneNodeId) { if !self.contains(id) { self.nodes.push(id); } }
    pub fn toggle(&mut self, id: XrdsSceneNodeId) {
        if let Some(p) = self.nodes.iter().position(|&x| x == id) { self.nodes.remove(p); }
        else { self.nodes.push(id); }
    }
    pub fn ids(&self) -> &[XrdsSceneNodeId] { &self.nodes }
    pub fn count(&self) -> usize { self.nodes.len() }
    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }
}

// ---------------------------------------------------------------------------
// Gizmo types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoAxis { X, Y, Z }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GizmoMode { #[default] Translate, Rotate, Scale }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraMode { #[default] Orbit, Fly }

#[derive(Clone, Debug)]
pub struct GizmoDrag {
    pub node_id: XrdsSceneNodeId,
    pub axis: GizmoAxis,
    pub origin: [f32; 3],
    pub origin_rotation: [f32; 4],
    pub origin_scale: [f32; 3],
    pub all_origins: Vec<(XrdsSceneNodeId, [f32; 3])>,
    pub all_origins_rotation: Vec<(XrdsSceneNodeId, [f32; 4])>,
    pub accumulated: f32,
}

// ---------------------------------------------------------------------------
// EditorState
// ---------------------------------------------------------------------------

/// What the frontend asked the Track preview to do. Drained once per frame.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackPreviewRequest {
    /// Start this Track in the editor world, replacing any current preview.
    Play(String),
    Pause(bool),
    /// Stop and restore every asset the preview moved back to its authored
    /// document values.
    Stop,
}

#[derive(Resource)]
pub struct EditorState {
    pub selection: Selection,

    // ── Transform live-preview (multi-node for gizmo, single for inspector) ──
    pub pending_translations: Vec<(XrdsSceneNodeId, [f32; 3])>,
    pub pending_rotations:    Vec<(XrdsSceneNodeId, [f32; 4])>,
    pub pending_scale:        Option<(XrdsSceneNodeId, [f32; 3])>,
    pub pending_visible:      Option<(XrdsSceneNodeId, bool)>,
    pub pending_grabbable:    Option<(XrdsSceneNodeId, bool)>,

    // ── Material / light live-preview ─────────────────────────────────────
    pub pending_material:          Option<(XrdsSceneNodeId, MaterialParamsDto)>,
    pub pending_point_light:       Option<(XrdsSceneNodeId, [f32; 4], f32, f32)>,
    pub pending_directional_light: Option<(XrdsSceneNodeId, [f32; 4], f32)>,
    pub pending_spot_light:        Option<(XrdsSceneNodeId, [f32; 4], f32, f32, f32, f32)>,
    pub pending_ambient_light:     Option<([f32; 4], f32)>,
    pub pending_extruded_color:    Option<(XrdsSceneNodeId, [f32; 4])>,
    pub pending_camera:            Option<(XrdsSceneNodeId, f32)>, // (id, fov_deg)
    pub pending_anchor_fov:        Option<(XrdsSceneNodeId, f32)>,
    pub pending_gravity_scale:     Option<(XrdsSceneNodeId, f32)>,
    pub pending_mass:              Option<(XrdsSceneNodeId, f32)>,
    pub pending_capsule_geometry:  Option<(XrdsSceneNodeId, f32, f32)>,
    /// Whole-struct rather than per-field, matching `SetEffectParams`: the
    /// frontend re-sends every value on each edit, so there is nothing to merge.
    pub pending_effect_params:     Option<(XrdsSceneNodeId, xrds_scene_graph::XrdsSceneEffect)>,
    /// Audition request from the Inspector: `(node, play)`. Deferred like
    /// `pending_effect_params` because playback needs the Bevy world, which the
    /// command handlers do not have.
    pub pending_audio_preview:     Option<(XrdsSceneNodeId, bool)>,
    /// Live falloff/volume edit, applied without respawning the clip so a slider
    /// drag does not cut off the preview being listened to.
    pub pending_audio_falloff:     Option<(XrdsSceneNodeId, xrds_scene_graph::XrdsSceneAudioClip)>,

    // ── Gizmo state ───────────────────────────────────────────────────────
    pub gizmo_mode:  GizmoMode,
    pub gizmo_hover: Option<GizmoAxis>,
    pub gizmo_drag:  Option<GizmoDrag>,

    // ── Camera / viewport ─────────────────────────────────────────────────
    pub camera_mode:             CameraMode,
    pub frame_selected_target:   Option<[f32; 3]>,
    /// None = render through editor orbit camera; Some(id) = render through scene camera node.
    pub active_camera_id:        Option<XrdsSceneNodeId>,
    /// None = all PlayerAnchors active; Some(id) = only that anchor runs camera-relative math.
    pub active_player_anchor_id: Option<XrdsSceneNodeId>,
    /// Set for one frame to teleport the editor camera to a PlayerAnchor's authored position.
    pub preview_anchor_target:   Option<XrdsSceneNodeId>,

    // ── Flags / toolbar ───────────────────────────────────────────────────
    pub show_grid:           bool,
    pub show_fov_overlay:    bool,
    pub light_rays_selected: bool,
    pub needs_full_reimport: bool,
    pub needs_env_sync: bool,
    pub pending_spawns:      Vec<XrdsSceneNodeId>,
    pub pending_status:      Option<String>,
    pub is_playing:          bool,
    pub snap_step:           f32,

    // ── GLTF animation ────────────────────────────────────────────────────
    pub pending_gltf_play:   Option<(XrdsSceneNodeId, usize, f32)>,
    pub pending_gltf_stop:   Option<XrdsSceneNodeId>,
    /// Clip names per GltfAsset node — refreshed each frame in update().
    pub gltf_clips: std::collections::HashMap<XrdsSceneNodeId, Vec<(usize, String)>>,

    // ── Trigger-action preview ───────────────────────────────────────────
    /// Set by `PreviewFireTrigger`, drained in `update()` where an
    /// `XrdsUpdateContext` (and therefore `fire_trigger`) is actually
    /// available — same pending/drain pattern as `pending_gltf_play`.
    pub pending_fire_trigger: Option<(
        XrdsSceneNodeId,
        xrds_scene_graph::XrdsTriggerKind,
        Option<xrds_components::XrGrabHand>,
    )>,

    // ── Track preview transport ──────────────────────────────────────────
    /// Set by the `PreviewPlayTrack`/`PreviewPauseTrack`/`PreviewStopTrack`
    /// commands, drained in `update()` where world access is available —
    /// same pending/drain pattern as `pending_fire_trigger`.
    ///
    /// Deliberately independent of `is_playing`: previewing one Track is not
    /// running the simulation.
    pub pending_track_preview: Option<TrackPreviewRequest>,
    /// The Track currently being previewed, if any. Its agent lives in the
    /// Bevy world; this is just the name, for the snapshot readout.
    pub track_preview_name: Option<String>,
    /// The live preview, mirrored out of the world once per frame so the
    /// snapshot builder — which has no world access — can report it. Drives the
    /// transport timecode and the playhead.
    pub track_preview: Option<crate::bridge::TrackPreviewDto>,
    /// The most recent asset-conflict refusal, mirrored out of the world with
    /// its entities resolved to node names. Without this a refused Track is a
    /// silent no-op — the whole weakness of the reject-the-newcomer policy.
    pub track_conflict: Option<crate::bridge::TrackConflictDto>,

    // ── Clipboard ─────────────────────────────────────────────────────────
    /// Flat list of cloned scene nodes (roots + descendants).
    /// Set by CopySelection / CutSelection; consumed (non-destructively) by PasteClipboard.
    pub clipboard: Option<Vec<xrds_scene_graph::XrdsSceneNode>>,

    // ── Play mode ─────────────────────────────────────────────────────────
    /// Snapshot of the scene document saved when play mode starts.
    /// Restored verbatim when play mode stops.
    pub play_snapshot:  Option<xrds_scene_graph::XrdsSceneDocument>,
    /// Entity ID of the spawned player pawn camera. `None` when not playing.
    pub pawn_entity:    Option<bevy::prelude::Entity>,
    /// True for one frame at play start — triggers GLB animation playback.
    pub play_started:   bool,

    // ── Export Application ────────────────────────────────────────────────
    /// Background export build job.  `None` when idle.
    pub export_job: Option<ExportJob>,

    // ── Android / Quest export ────────────────────────────────────────────
    /// Results of the last `CheckApkPrerequisites` run.  Consumed by the
    /// snapshot broadcaster after one frame (same pattern as `pending_status`).
    pub apk_prerequisites: Option<Vec<ApkPrerequisite>>,
    /// Background APK build job.  `None` when idle.
    pub apk_export_job: Option<ApkExportJob>,
}

/// A background export job started by `ExportApplication`.
///
/// The build thread writes its final status into `result` when done.
/// `update()` polls this every frame via `try_lock` to keep it non-blocking.
pub struct ExportJob {
    pub out_dir: String,
    /// `Some(Ok(message))` on success, `Some(Err(message))` on failure, `None` while running.
    pub result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
}

/// A background APK build job started by `ExportApk`.
/// Extends ExportJob with a streaming log buffer read by the snapshot broadcaster.
pub struct ApkExportJob {
    pub out_dir: String,
    /// Lines emitted by the build script (stdout + stderr), appended by reader threads.
    pub log: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    /// `Some(Ok(msg))` on success, `Some(Err(msg))` on failure, `None` while running.
    pub result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            pending_translations: Vec::new(),
            pending_rotations: Vec::new(),
            pending_scale: None,
            pending_visible: None,
            pending_grabbable: None,
            pending_material: None,
            pending_point_light: None,
            pending_directional_light: None,
            pending_spot_light: None,
            pending_ambient_light: None,
            pending_extruded_color: None,
            pending_camera: None,
            pending_anchor_fov: None,
            pending_gravity_scale: None,
            pending_mass: None,
            pending_capsule_geometry: None,
            pending_effect_params: None,
            pending_audio_preview: None,
            pending_audio_falloff: None,
            gizmo_mode: GizmoMode::Translate,
            gizmo_hover: None,
            gizmo_drag: None,
            camera_mode: CameraMode::Orbit,
            frame_selected_target: None,
            active_camera_id: None,
            active_player_anchor_id: None,
            preview_anchor_target: None,
            show_grid: true,
            show_fov_overlay: false,
            light_rays_selected: false,
            needs_full_reimport: false,
            needs_env_sync: false,
            pending_spawns: Vec::new(),
            pending_status: None,
            is_playing: false,
            snap_step: 0.25,
            clipboard: None,
            play_snapshot: None,
            pawn_entity: None,
            play_started: false,
            pending_gltf_play: None,
            pending_gltf_stop: None,
            gltf_clips: std::collections::HashMap::new(),
            pending_fire_trigger: None,
            pending_track_preview: None,
            track_preview_name: None,
            track_preview: None,
            track_conflict: None,
            export_job: None,
            apk_prerequisites: None,
            apk_export_job: None,
        }
    }
}

impl EditorState {
    // ── Selection helpers (used by webview bridge commands) ───────────────
    pub fn select_single(&mut self, id: XrdsSceneNodeId) { self.selection.set_single(id); }
    pub fn toggle_selection(&mut self, id: XrdsSceneNodeId) { self.selection.toggle(id); }
    pub fn deselect_all(&mut self) { self.selection.clear(); }
    pub fn is_selected(&self, id: XrdsSceneNodeId) -> bool { self.selection.contains(id); false }

    // ── Pending translation helpers (used by gizmo + inspector) ──────────
    pub fn pending_translation_for(&self, id: XrdsSceneNodeId) -> Option<[f32; 3]> {
        self.pending_translations.iter().find(|(i, _)| *i == id).map(|(_, v)| *v)
    }
    pub fn set_pending_translation(&mut self, id: XrdsSceneNodeId, v: [f32; 3]) {
        if let Some(entry) = self.pending_translations.iter_mut().find(|(i, _)| *i == id) {
            entry.1 = v;
        } else {
            self.pending_translations.push((id, v));
        }
    }
    pub fn clear_pending_translations(&mut self) { self.pending_translations.clear(); }

    pub fn pending_rotation_for(&self, id: XrdsSceneNodeId) -> Option<[f32; 4]> {
        self.pending_rotations.iter().find(|(i, _)| *i == id).map(|(_, v)| *v)
    }
    pub fn set_pending_rotation(&mut self, id: XrdsSceneNodeId, v: [f32; 4]) {
        if let Some(entry) = self.pending_rotations.iter_mut().find(|(i, _)| *i == id) {
            entry.1 = v;
        } else {
            self.pending_rotations.push((id, v));
        }
    }
    pub fn clear_pending_rotations(&mut self) { self.pending_rotations.clear(); }
}
