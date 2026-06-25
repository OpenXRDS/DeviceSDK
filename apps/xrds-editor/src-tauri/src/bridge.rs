use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Shared channel between the UI (main thread) and Bevy.
pub struct EditorBridge {
    /// UI → Bevy: commands pushed by IPC handler, drained by Bevy each frame.
    pub inbound: Arc<Mutex<VecDeque<EditorCommand>>>,
    /// Bevy → UI: state snapshots pushed each frame, consumed by push_snapshot_to_webview.
    pub outbound: Arc<Mutex<VecDeque<EditorSnapshot>>>,
}

impl EditorBridge {
    pub fn new() -> Self {
        Self {
            inbound: Arc::new(Mutex::new(VecDeque::new())),
            outbound: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// Commands — webview → Bevy
// ---------------------------------------------------------------------------

/// Every action the webview can request of the Bevy world.
/// Tagged with `type` so JS can send `{ type: "SelectNode", payload: { id: 1 } }`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "payload")]
pub enum EditorCommand {
    // --- Selection ---
    SelectNode { id: u64 },
    MultiSelectNode { id: u64, extend: bool },
    DeselectAll,

    // --- Hierarchy mutations ---
    RenameNode { id: u64, name: String },
    DeleteNode { id: u64 },
    DuplicateNode { id: u64 },
    ReparentNode { id: u64, new_parent_id: Option<u64>, index: usize },

    // --- Palette spawning ---
    SpawnPrimitive { kind: String, parent_id: Option<u64> },
    SpawnAsset { asset_id: String, parent_id: Option<u64> },

    // --- Transform (live preview + commit) ---
    SetTranslation { id: u64, value: [f32; 3] },
    SetRotationEuler { id: u64, degrees: [f32; 3] },
    SetScale { id: u64, value: [f32; 3] },
    /// Commit carries final values — independent of pending state timing.
    CommitTransform { id: u64, translation: [f32; 3], rotation_euler_degrees: [f32; 3], scale: [f32; 3] },

    // --- Material ---
    SetMaterial { id: u64, params: MaterialParamsDto },
    /// Commit carries params — independent of pending state timing.
    CommitMaterial { id: u64, params: MaterialParamsDto },

    // --- Lights ---
    SetPointLight { id: u64, color: [f32; 4], intensity: f32, range: f32 },
    SetDirectionalLight { id: u64, color: [f32; 4], illuminance: f32 },
    SetSpotLight { id: u64, color: [f32; 4], intensity: f32, range: f32, inner_angle: f32, outer_angle: f32 },
    SetAmbientLight { id: u64, color: [f32; 4], brightness: f32 },
    CommitLight { id: u64 },

    // --- Visibility / interaction ---
    SetVisible   { id: u64, visible: bool },
    SetGrabbable { id: u64, grabbable: bool },
    SetHudText      { id: u64, text: String, font_size: f32, color: [f32; 4], anchor: String, offset: [f32; 2] },
    // --- HUD library ---
    CreateHudTemplate   { name: String },
    DeleteHudTemplate   { id: u64 },
    RenameHudTemplate   { id: u64, name: String },
    SetHudTemplateDepth { id: u64, depth: f32 },
    AddHudItem          { template_id: u64 },
    RemoveHudItem       { template_id: u64, item_id: u64 },
    RenameHudItem       { template_id: u64, item_id: u64, name: String },
    SetHudItemPosition  { template_id: u64, item_id: u64, position: [f32; 2] },
    SetHudItemText      { template_id: u64, item_id: u64, text: String },
    SetHudItemFontSize  { template_id: u64, item_id: u64, font_size: f32 },
    SetHudItemColor     { template_id: u64, item_id: u64, color: [f32; 4] },
    LinkHudTemplate     { anchor_id: u64, template_id: Option<u64> },
    SetTextContent { id: u64, text: String, font_size: f32, color: [f32; 4], alignment: String, anchor: String, anchor_param: f32 },
    SetExtrudedText { id: u64, text: String, font_size: f32, depth: f32, color: [f32; 4], alignment: String },
    /// Color-only update for ExtrudedText — in-place via StandardMaterial, no reimport.
    SetExtrudedTextColor { id: u64, color: [f32; 4] },
    CommitText { id: u64 },

    // --- Camera node ---
    SetCameraParams    { id: u64, fov: f32, near: f32, far: f32 },
    CommitCameraParams { id: u64, fov: f32, near: f32, far: f32 },

    // --- Physics ---
    /// "None" | "Static" | "Dynamic"
    SetPhysicsBody  { id: u64, physics_body: String },
    SetGravityScale { id: u64, value: f32 },
    SetMass         { id: u64, value: f32 },

    // --- Player / PlayerAnchor / SpawnZone ---
    SetPlayerAnchorFov      { id: u64, fov_deg: f32 },
    SetPlayerAnchorInitial  { id: u64, is_initial: bool },
    SetPlayerAnchorExposure { id: u64, ev100: Option<f32> },
    SetSpawnZoneSize        { id: u64, size: [f32; 3] },
    SetSpawnZonePlayer      { id: u64, player_node_id: Option<u64> },
    /// Switch the editor viewport to render through a scene camera node, or back to the editor camera (id = None).
    SetActiveCamera    { id: Option<u64> },
    /// Set the active PlayerAnchor for anchor systems.  None = all anchors active.
    SetActivePlayerAnchor { id: Option<u64> },
    /// Teleport the editor camera to a PlayerAnchor's authored world position for preview.
    PreviewFromAnchor { id: u64 },

    // --- GLTF Animation ---
    PlayGltfAnimation { id: u64, clip_index: usize, speed: f32, repeat: String },
    StopGltfAnimation { id: u64 },
    PauseGltfAnimation { id: u64 },
    ResumeGltfAnimation { id: u64 },

    // --- Viewport / editor settings ---
    SetGizmoMode { mode: String },
    SetCameraMode { mode: String },
    ToggleGrid,
    ToggleFovOverlay,
    FrameSelected,
    SetPlayMode { playing: bool },
    SetSnapStep { step: f32 },
    TogglePanel { panel: String },
    SetViewportFocus { focused: bool },

    // --- Scene environment ---
    SetFog        { color: [f32; 4], start: f32, end: f32 },
    ClearFog,
    SetExposure   { ev100: f32 },
    ClearExposure,
    SetIbl        { diffuse_asset_id: String, specular_asset_id: String, intensity: f32 },
    ClearIbl,
    SetSkybox     { texture_asset_id: String, brightness: f32 },
    ClearSkybox,

    // --- Asset catalog ---
    RemoveAsset { asset_id: String },

    // --- File I/O ---
    NewScene,
    OpenScene { path: String },
    SaveScene,
    SaveSceneAs { path: String },
    ImportAsset { path: String },
    ExportGlb { path: String },
    ExportApplication { output_dir: String },

    // --- Android / Quest export ---
    CheckApkPrerequisites,
    ExportApk { output_dir: String },

    // --- Edit ---
    Undo,
    Redo,
    CutSelection,
    CopySelection,
    PasteClipboard,
    DeleteSelection,
    DuplicateSelection,
    SelectAll,
}

// ---------------------------------------------------------------------------
// Snapshot — Bevy → webview
// ---------------------------------------------------------------------------

/// State snapshot emitted to the webview after each frame's update.
/// Grow this incrementally — add fields as each phase is implemented.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct EditorSnapshot {
    // Phase 1
    pub hierarchy: Vec<HierarchyNode>,
    pub selection: Vec<u64>,

    // Phase 2
    pub asset_catalog: Vec<AssetCatalogEntry>,

    // Phase 3
    pub selected_node: Option<NodeInspectorDto>,

    // Phase 4
    pub undo_count: usize,
    pub redo_count: usize,
    pub is_dirty: bool,
    pub scene_name: String,
    pub status_message: Option<String>,

    // Phase 5
    pub gizmo_mode: String,
    pub camera_mode: String,
    pub show_grid: bool,
    pub show_fov_overlay: bool,
    pub is_playing: bool,
    pub snap_step: f32,
    /// True while a background `cargo build` export job is running.
    pub is_exporting: bool,
    pub has_clipboard: bool,
    /// Current scene environment (from document metadata). None = no environment set.
    pub environment: Option<EnvironmentDto>,
    /// All Camera nodes in the current scene (for the camera selector dropdown).
    pub available_cameras: Vec<CameraNodeDto>,
    /// None = editor camera active; Some(id) = scene camera node active.
    pub active_camera_id: Option<u64>,
    /// All PlayerAnchor nodes in the current scene.
    pub player_anchors: Vec<PlayerAnchorNodeDto>,
    /// None = all anchors active; Some(id) = only that anchor processes anchor systems.
    pub active_player_anchor_id: Option<u64>,
    /// HUD template library — all authored HUD layouts in this document.
    #[serde(default)]
    pub hud_library: Vec<HudTemplateDto>,
    /// True when the side-by-side stereo preview is active.
    #[serde(default)]
    pub stereo_preview_active: bool,
    /// Results of the last `CheckApkPrerequisites` command.
    /// `None` on most frames (cleared after sending); `Some` for one frame when a check completes.
    #[serde(default)]
    pub apk_prerequisites: Option<Vec<ApkPrerequisite>>,
    /// True while an APK export build is running.
    #[serde(default)]
    pub is_exporting_apk: bool,
    /// Tail of the APK build log (last ≤200 lines).  Empty when no APK export is running.
    #[serde(default)]
    pub apk_build_log: Vec<String>,
}

/// Snapshot of the scene-wide environment settings (fog, exposure, IBL, skybox).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct EnvironmentDto {
    pub fog_enabled:  bool,
    pub fog_color:    [f32; 4],
    pub fog_start:    f32,
    pub fog_end:      f32,
    pub exposure_enabled: bool,
    pub ev100:        f32,
    pub ibl_enabled:  bool,
    pub ibl_diffuse:  String,
    pub ibl_specular: String,
    pub ibl_intensity:f32,
    pub skybox_enabled:  bool,
    pub skybox_asset:    String,
    pub skybox_brightness: f32,
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct HierarchyNode {
    pub id: u64,
    pub name: String,
    pub kind: String,
    pub visible: bool,
    pub children: Vec<HierarchyNode>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AssetCatalogEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
}

/// Inspector data for the selected node.
/// Uses an untagged enum so the frontend can switch on `payload_type`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct NodeInspectorDto {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub grabbable: bool,
    /// `Some(parent_id)` when this node is a child of another node.
    /// Used by the inspector to label the transform section as "local to parent".
    pub parent_id: Option<u64>,
    pub translation: [f32; 3],
    pub rotation_euler_degrees: [f32; 3],
    pub scale: [f32; 3],
    pub payload: NodePayloadDto,
    /// Kind string of the immediate parent node, or None for root nodes.
    pub parent_kind: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum NodePayloadDto {
    Empty,
    Cube    { material: MaterialParamsDto, physics_body: String, gravity_scale: f32, mass: f32 },
    Sphere  { material: MaterialParamsDto, physics_body: String, gravity_scale: f32, mass: f32 },
    Cylinder{ material: MaterialParamsDto, physics_body: String, gravity_scale: f32, mass: f32 },
    Plane   { material: MaterialParamsDto, physics_body: String, gravity_scale: f32, mass: f32 },
    Camera  { fov: f32, near: f32, far: f32 },
    PointLight { color: [f32; 4], intensity: f32, range: f32 },
    DirectionalLight { color: [f32; 4], illuminance: f32 },
    SpotLight { color: [f32; 4], intensity: f32, range: f32, inner_angle: f32, outer_angle: f32 },
    AmbientLight { color: [f32; 4], brightness: f32 },
    Text    { text: String, font_size: f32, color: [f32; 4], alignment: String, anchor: String, anchor_param: f32 },
    ExtrudedText { text: String, font_size: f32, depth: f32, color: [f32; 4], alignment: String },
    GltfAsset { clips: Vec<GltfClipDto> },
    HudText   { text: String, font_size: f32, color: [f32; 4], anchor: String, offset: [f32; 2] },
    Player,
    PlayerAnchor  { fov_deg: f32, is_initial: bool, hud_template_id: Option<u64>, exposure: Option<f32> },
    PlayerSpawnZone { size: [f32; 3], player_node_id: Option<u64> },
    Other     { kind: String },
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MaterialParamsDto {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
}

impl Default for MaterialParamsDto {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GltfClipDto {
    pub index: usize,
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct HudItemDefDto {
    pub id: u64,
    pub name: String,
    pub position: [f32; 2],
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct HudTemplateDto {
    pub id: u64,
    pub name: String,
    pub depth: f32,
    pub items: Vec<HudItemDefDto>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CameraNodeDto {
    pub id: u64,
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PlayerAnchorNodeDto {
    pub id: u64,
    pub name: String,
    /// Name of the parent `Player` node, or empty string if standalone.
    pub player_name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ApkPrerequisite {
    pub name: String,
    pub ok: bool,
    pub hint: String,
}
