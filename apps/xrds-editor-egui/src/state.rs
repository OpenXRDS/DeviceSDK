use std::collections::HashMap;
use std::path::PathBuf;
use xrds::editor::{bevy_ecs, Entity, Resource};
use xrds::scene_graph::{
    XrdsSceneDocument, XrdsSceneDocumentSession, XrdsSceneMaterialTextureSlotKind,
    XrdsSceneNode, XrdsSceneNodeId,
};
use xrds::sdk::XrdsMaterialParams;
pub use xrds::{XrdsAnimationRepeatMode, XrdsGltfAnimationState};

// ── BuildJob ──────────────────────────────────────────────────────────────────

/// In-flight `cargo build --release` launched by "Export as Application".
pub struct BuildJob {
    pub handle: std::thread::JoinHandle<Result<(), String>>,
    pub out_dir: PathBuf,
}

// ── PendingFileDialog ─────────────────────────────────────────────────────────

/// All `rfd::FileDialog` calls from within Bevy ECS systems use a background
/// thread + channel to avoid the `dispatch_sync` deadlock on macOS (the dialog
/// posts work to the main queue, which is blocked waiting for ECS to finish).
///
/// One dialog may be in-flight at a time.  The `op` field records what to do
/// when the path arrives; all processing happens in `XrdsEditorApp::update`.

#[derive(Debug)]
pub enum PendingFileOpKind {
    ImportAsset,
    OpenScene,
    SaveSceneAs,
    ExportGlb,
    ExportGlbSelectionCopy { source: PathBuf },
    ExportGlbSelectionExport { node_id: XrdsSceneNodeId },
    /// Inspector material panel — "…" pick-texture button.
    PickTexture { node_id: XrdsSceneNodeId, slot_kind: XrdsSceneMaterialTextureSlotKind },
}

pub struct PendingFileDialog {
    /// Receiver is `Send` but not `Sync`; Mutex satisfies `Resource: Sync`.
    pub rx: std::sync::Mutex<std::sync::mpsc::Receiver<Option<PathBuf>>>,
    pub op: PendingFileOpKind,
}

// ── ExportAppPending ──────────────────────────────────────────────────────────

/// Holds the folder-picker channel and a snapshot of the scene taken at click
/// time.  Separate from `PendingFileDialog` because `pick_folder` is a
/// folder-only dialog and carries additional state (doc + save_path).
pub struct ExportAppPending {
    /// Receiver is `Send` but not `Sync`; the Mutex satisfies `Resource: Sync`.
    pub rx: std::sync::Mutex<std::sync::mpsc::Receiver<Option<PathBuf>>>,
    pub doc: XrdsSceneDocument,
    pub save_path: Option<PathBuf>,
}

// ── GltfClipInfo ──────────────────────────────────────────────────────────────

/// Lightweight animation clip descriptor parsed from the GLB file.
#[derive(Clone, Debug)]
pub struct GltfClipInfo {
    pub index: usize,
    pub name: Option<String>,
}

// ── PerfStats ─────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct PerfStats {
    pub fps: f32,
    pub frame_ms: f32,
    pub mesh_entity_count: u32,
    pub vertex_count: u64,
    pub texture_memory_kb: u64,
}

// ── EditorSession ─────────────────────────────────────────────────────────────

/// Authoritative document state.  All committed edits go through the session.
/// The runtime is re-synced from the session on explicit import.
#[derive(Resource)]
pub struct EditorSession {
    pub session: XrdsSceneDocumentSession,
}

impl EditorSession {
    pub fn new(document: XrdsSceneDocument) -> Self {
        Self {
            session: XrdsSceneDocumentSession::new(document)
                .expect("default editor document should be valid"),
        }
    }

    pub fn document(&self) -> &XrdsSceneDocument {
        self.session.document()
    }
}

// ── Selection ─────────────────────────────────────────────────────────────────

/// Ordered multi-node selection.  Nodes are stored in selection-time order;
/// the last entry is the "primary" node (gizmo anchor, inspector target).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    nodes: Vec<XrdsSceneNodeId>,
}

impl Selection {
    pub fn primary(&self) -> Option<XrdsSceneNodeId> {
        self.nodes.last().copied()
    }

    pub fn contains(&self, id: XrdsSceneNodeId) -> bool {
        self.nodes.contains(&id)
    }

    pub fn set_single(&mut self, id: XrdsSceneNodeId) {
        self.nodes.clear();
        self.nodes.push(id);
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// Add `id` if not already present (Shift+Click behaviour).
    pub fn add(&mut self, id: XrdsSceneNodeId) {
        if !self.contains(id) {
            self.nodes.push(id);
        }
    }

    /// Add if absent, remove if present (Ctrl+Click behaviour).
    pub fn toggle(&mut self, id: XrdsSceneNodeId) {
        if let Some(pos) = self.nodes.iter().position(|&x| x == id) {
            self.nodes.remove(pos);
        } else {
            self.nodes.push(id);
        }
    }

    pub fn ids(&self) -> &[XrdsSceneNodeId] {
        &self.nodes
    }

    pub fn count(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

// ── GizmoAxis / GizmoMode ─────────────────────────────────────────────────────

/// Which gizmo axis handle is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoAxis { X, Y, Z }

/// Whether the transform gizmo operates in translation, rotation, or scale mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GizmoMode { #[default] Translate, Rotate, Scale }

/// Camera navigation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraMode { #[default] Orbit, Fly }

/// Active gizmo drag state (set when the user begins dragging an axis handle).
#[derive(Clone, Debug)]
pub struct GizmoDrag {
    /// Primary node being dragged (used for rotate / scale commit).
    pub node_id: XrdsSceneNodeId,
    pub axis: GizmoAxis,
    /// World-space centroid of the selection at drag start (used for screen-space axis projection).
    pub origin: [f32; 3],
    /// Rotation quaternion [x,y,z,w] at drag start (rotate mode, primary node).
    pub origin_rotation: [f32; 4],
    /// Per-axis scale at drag start (scale mode, primary node).
    pub origin_scale: [f32; 3],
    /// Per-node start translations for every selected node (translate mode).
    pub all_origins: Vec<(XrdsSceneNodeId, [f32; 3])>,
    /// Per-node start rotations for every selected node (rotate mode).
    pub all_origins_rotation: Vec<(XrdsSceneNodeId, [f32; 4])>,
    /// Cumulative drag offset: world units (translate), radians (rotate), or scale factor (scale).
    pub accumulated: f32,
}

// ── EditorState ───────────────────────────────────────────────────────────────

/// UI-only editor state.
#[derive(Resource)]
pub struct EditorState {
    pub selection: Selection,
    pub hovered_id: Option<XrdsSceneNodeId>,

    /// Pending translations — one entry per node (gizmo drag or inspector T field).
    pub pending_translations: Vec<(XrdsSceneNodeId, [f32; 3])>,
    /// Pending rotations — one entry per node (gizmo drag: all selected; inspector: primary only).
    pub pending_rotations: Vec<(XrdsSceneNodeId, [f32; 4])>,
    /// Pending scale (inspector S fields, primary node only).
    pub pending_scale: Option<(XrdsSceneNodeId, [f32; 3])>,
    /// Pending material params (inspector color/slider edits).
    pub pending_material: Option<(XrdsSceneNodeId, XrdsMaterialParams)>,
    /// Pending visibility change — applied to the runtime entity in update().
    pub pending_visible: Option<(XrdsSceneNodeId, bool)>,
    /// Pending point light: (id, color [f32;4], intensity, range).
    pub pending_point_light: Option<(XrdsSceneNodeId, [f32; 4], f32, f32)>,
    /// Pending directional light: (id, color [f32;4], illuminance).
    pub pending_directional_light: Option<(XrdsSceneNodeId, [f32; 4], f32)>,
    /// Pending spot light: (id, color [f32;4], intensity, range, inner_angle, outer_angle).
    pub pending_spot_light: Option<(XrdsSceneNodeId, [f32; 4], f32, f32, f32, f32)>,
    /// Pending ambient light: (id, color [f32;4], brightness).
    pub pending_ambient_light: Option<(XrdsSceneNodeId, [f32; 4], f32)>,
    /// Pending extruded-text color update: (id, color [f32;4]).  Applied in-place
    /// without a full reimport to avoid the bevy_fontmesh update_text_meshes race.
    pub pending_extruded_color: Option<(XrdsSceneNodeId, [f32; 4])>,
    /// Floor grid overlay toggle.
    pub show_grid: bool,
    /// Debug: draw light shapes for every visible light node.
    pub light_rays_all: bool,
    /// Debug: draw light shapes only for the currently selected light node.
    pub light_rays_selected: bool,
    /// Full scene reimport — despawn all XRDS entities and re-spawn from the
    /// session document.  Set when new nodes are added via the palette so they
    /// appear in the 3D viewport immediately.
    pub needs_full_reimport: bool,
    /// Node IDs queued for incremental spawn (palette placement, no full reimport).
    /// Each ID names a node that is already in the session document but not yet
    /// in the Bevy world.  Processed by `XrdsUpdateContext::spawn_document_node`.
    pub pending_node_spawns: Vec<XrdsSceneNodeId>,
    /// Set by undo/redo to trigger a full runtime sync in `XrdsApp::update`.
    pub needs_runtime_sync: bool,
    /// One-line status message shown in the toolbar.
    pub status_message: Option<String>,
    /// Active tab in the palette panel (0 = Primitives, 1 = Project Assets).
    pub palette_tab: u8,
    /// Search text for the project-assets tab filter.
    pub asset_search: String,
    // ── Panel visibility ──────────────────────────────────────────────────────
    pub gizmo_mode: GizmoMode,
    pub show_help: bool,
    pub show_hierarchy: bool,
    pub show_inspector: bool,
    pub show_palette: bool,
    /// Persistent name-edit buffer in the inspector.
    pub editing_name: Option<(XrdsSceneNodeId, String)>,
    /// Node currently being renamed inline in the hierarchy panel.
    pub renaming_id: Option<XrdsSceneNodeId>,

    /// Which axis handle the cursor is near (updated by gizmo_interaction_system).
    pub gizmo_hover: Option<GizmoAxis>,
    /// Active drag — set on mouse-down on an axis handle, cleared on release.
    pub gizmo_drag: Option<GizmoDrag>,
    /// Set by F key to re-center the orbit camera on a node; consumed by orbit_camera_system.
    pub frame_selected_target: Option<[f32; 3]>,

    // ── Copy / paste ──────────────────────────────────────────────────────────
    /// Subtrees copied by Ctrl+C.  Each inner Vec is one subtree (root first).
    pub clipboard: Option<Vec<Vec<XrdsSceneNode>>>,

    // ── Play mode ─────────────────────────────────────────────────────────────
    pub play_snapshot: Option<XrdsSceneDocument>,
    pub is_playing: bool,
    /// The spawned player pawn entity — `Some` while playing, `None` otherwise.
    pub pawn_entity: Option<Entity>,
    /// Set to true for one frame when play mode starts; consumed by `XrdsApp::update`
    /// to kick off GLB animations.
    pub play_started: bool,

    // ── GLB animation cache ───────────────────────────────────────────────────
    /// Clip names parsed directly from the GLB file (index → optional name).
    /// Populated once on first selection; never depends on Bevy's async loading.
    pub gltf_clips: HashMap<XrdsSceneNodeId, Vec<GltfClipInfo>>,
    /// Current playback state per GLB node (updated every frame from runtime).
    pub gltf_anim_state: HashMap<XrdsSceneNodeId, Option<XrdsGltfAnimationState>>,

    // ── Pending per-node animation commands (set by inspector UI) ─────────────
    pub pending_gltf_play: Option<(XrdsSceneNodeId, usize, f32, XrdsAnimationRepeatMode)>,
    pub pending_gltf_stop: Option<XrdsSceneNodeId>,
    pub pending_gltf_pause: Option<XrdsSceneNodeId>,
    pub pending_gltf_resume: Option<XrdsSceneNodeId>,

    // ── Snap ─────────────────────────────────────────────────────────────────
    pub snap_step: f32,

    // ── Camera mode ───────────────────────────────────────────────────────────
    pub camera_mode: CameraMode,

    // ── Scene metadata edit buffers ───────────────────────────────────────────
    pub editing_scene_name: Option<String>,
    pub editing_scene_author: Option<String>,

    // ── Performance stats ─────────────────────────────────────────────────────
    pub perf_stats: PerfStats,
    pub show_perf_stats: bool,

    // ── SVG icon cache ────────────────────────────────────────────────────────
    pub icon_cache: crate::icon::SvgIconCache,

    // ── Pending file dialog (import / save / load / export) ──────────────────
    pub pending_file_dialog: Option<PendingFileDialog>,

    // ── Background build job (Export as Application) ──────────────────────────
    pub build_job: Option<BuildJob>,
    pub export_app_pending: Option<ExportAppPending>,

    // ── Template picker (New Scene dialog) ───────────────────────────────────
    pub show_template_picker: bool,
    pub template_picker_selection: &'static str,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            hovered_id: None,
            pending_translations: vec![],
            pending_rotations: vec![],
            pending_scale: None,
            pending_material: None,
            pending_visible: None,
            pending_point_light: None,
            pending_directional_light: None,
            pending_spot_light: None,
            pending_ambient_light: None,
            pending_extruded_color: None,
            show_grid: true,
            light_rays_all: false,
            light_rays_selected: false,
            needs_full_reimport: false,
            pending_node_spawns: vec![],
            needs_runtime_sync: false,
            status_message: None,
            palette_tab: 0,
            gizmo_mode: GizmoMode::default(),
            show_help: false,
            show_hierarchy: true,
            show_inspector: true,
            show_palette: true,
            asset_search: String::new(),
            editing_name: None,
            renaming_id: None,
            gizmo_hover: None,
            gizmo_drag: None,
            frame_selected_target: None,
            clipboard: None,
            play_snapshot: None,
            is_playing: false,
            pawn_entity: None,
            play_started: false,
            gltf_clips: HashMap::new(),
            gltf_anim_state: HashMap::new(),
            pending_gltf_play: None,
            pending_gltf_stop: None,
            pending_gltf_pause: None,
            pending_gltf_resume: None,
            snap_step: 0.25,
            camera_mode: CameraMode::Orbit,
            editing_scene_name: None,
            editing_scene_author: None,
            perf_stats: PerfStats::default(),
            show_perf_stats: false,
            icon_cache: crate::icon::SvgIconCache::new(),
            pending_file_dialog: None,
            build_job: None,
            export_app_pending: None,
            show_template_picker: false,
            template_picker_selection: "empty",
        }
    }
}

impl EditorState {
    // ── pending_translations helpers ─────────────────────────────────────────

    pub fn pending_translation_for(&self, id: XrdsSceneNodeId) -> Option<[f32; 3]> {
        self.pending_translations.iter()
            .find(|(sid, _)| *sid == id)
            .map(|(_, v)| *v)
    }

    pub fn set_pending_translation(&mut self, id: XrdsSceneNodeId, v: [f32; 3]) {
        if let Some(entry) = self.pending_translations.iter_mut().find(|(sid, _)| *sid == id) {
            entry.1 = v;
        } else {
            self.pending_translations.push((id, v));
        }
    }

    pub fn clear_pending_translations(&mut self) {
        self.pending_translations.clear();
    }

    // ── pending_rotations helpers ────────────────────────────────────────────

    pub fn pending_rotation_for(&self, id: XrdsSceneNodeId) -> Option<[f32; 4]> {
        self.pending_rotations.iter()
            .find(|(sid, _)| *sid == id)
            .map(|(_, v)| *v)
    }

    pub fn set_pending_rotation(&mut self, id: XrdsSceneNodeId, v: [f32; 4]) {
        if let Some(entry) = self.pending_rotations.iter_mut().find(|(sid, _)| *sid == id) {
            entry.1 = v;
        } else {
            self.pending_rotations.push((id, v));
        }
    }

    pub fn clear_pending_rotations(&mut self) {
        self.pending_rotations.clear();
    }
}
