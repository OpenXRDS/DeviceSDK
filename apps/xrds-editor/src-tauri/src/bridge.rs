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

    // --- World Panel ---
    SetWorldPanelParams { id: u64, size: [f32; 2], color: [f32; 4], corner_radius: f32, opacity: f32 },
    /// kind: "Label" | "Button" | "Image" | "Slider" | "Toggle" — appends a default widget.
    AddWorldPanelWidget    { id: u64, kind: String },
    RemoveWorldPanelWidget { id: u64, index: usize },
    /// Reorder a widget within the panel's list by ±1.
    MoveWorldPanelWidget   { id: u64, index: usize, delta: i32 },
    SetWorldPanelWidget    { id: u64, index: usize, widget: WorldWidgetDto },
    /// Replace the whole widget list at once (used by the panel editor's Cancel/revert).
    SetWorldPanelWidgets   { id: u64, widgets: Vec<WorldWidgetDto> },
    SetWorldPanelLayout    { id: u64, layout: WorldLayoutDto },

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

    // --- Trigger-action: runnable registry (document-level) ---
    /// kind: "sequence" | "timeline". Rejected (logged, no-op) if `name` is
    /// already taken.
    CreateRunnable { name: String, kind: String },
    DeleteRunnable { name: String },
    RenameRunnable { old_name: String, new_name: String },
    SetTimelineLooping  { name: String, looping: bool },
    SetTimelineDuration { name: String, duration_secs: Option<f32> },

    // --- Trigger-action: steps (registry sequence body OR a binding's inline sequence) ---
    /// kind: one of the `XrdsAction` variant names ("SetVisible", "Teleport", ...).
    AddActionStep    { target: StepTargetDto, kind: String },
    RemoveActionStep { target: StepTargetDto, index: usize },
    /// Reorder a step within its list by ±1.
    MoveActionStep   { target: StepTargetDto, index: usize, delta: i32 },
    SetActionStep    { target: StepTargetDto, index: usize, action: XrdsActionDto },

    // --- Trigger-action: timeline keys (registry timeline body only) ---
    AddTimelineKey    { name: String, at_secs: f32, kind: String },
    RemoveTimelineKey { name: String, index: usize },
    SetTimelineKey    { name: String, index: usize, key: XrdsTimelineKeyDto },

    // --- Trigger-action: per-node bindings ---
    AddTriggerBinding    { node_id: u64 },
    RemoveTriggerBinding { node_id: u64, index: usize },
    SetTriggerBindingTrigger  { node_id: u64, index: usize, trigger: XrdsTriggerKindDto },
    /// hand: "Left" | "Right" | null.
    SetTriggerBindingHand     { node_id: u64, index: usize, hand: Option<String> },
    SetTriggerBindingDisabled { node_id: u64, index: usize, disabled: bool },
    SetTriggerBindingRunnable { node_id: u64, index: usize, runnable: Option<String> },

    // --- Trigger-action: per-node threshold watchers ---
    AddWatcher    { node_id: u64 },
    RemoveWatcher { node_id: u64, index: usize },
    SetWatcher    { node_id: u64, index: usize, watcher: ThresholdWatcherDto },

    /// Fires binding `index` on `node_id` right now, without waiting for a
    /// real ZoneEnter/Grabbed/etc event — there is no other way to trigger
    /// one from a desktop editor UI. See `XrdsUpdateContext::fire_trigger`.
    PreviewFireTrigger { node_id: u64, index: usize },

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
    /// Document-level named-runnable registry (Phase 9a).
    #[serde(default)]
    pub runnables: Vec<NamedRunnableDto>,
    /// Registry-level trigger diagnostics only (`node_id: None`) — a `Run`
    /// naming an unregistered runnable, or a cycle in the registry's own
    /// `Run`-graph. Per-node diagnostics ride on `selected_node.trigger_diagnostics`
    /// instead.
    #[serde(default)]
    pub runnable_diagnostics: Vec<TriggerDiagnosticDto>,
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
    /// Applies regardless of `payload` kind — any node can carry triggers.
    #[serde(default)]
    pub triggers: Vec<TriggerBindingDto>,
    #[serde(default)]
    pub watchers: Vec<ThresholdWatcherDto>,
    /// This node's subset of `trigger_diagnostics()` (`node_id == Some(id)`).
    #[serde(default)]
    pub trigger_diagnostics: Vec<TriggerDiagnosticDto>,
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
    WorldPanel {
        size: [f32; 2], color: [f32; 4], corner_radius: f32, opacity: f32,
        layout: WorldLayoutDto, widgets: Vec<WorldWidgetDto>,
    },
    Other     { kind: String },
}

/// Mirrors `xrds_scene_graph::XrdsSceneWorldLayout` for the webview.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum WorldLayoutDto {
    None,
    VStack { gap: f32 },
    HStack { gap: f32 },
    Grid   { cols: usize, gap: [f32; 2] },
}

/// Mirrors `xrds_scene_graph::XrdsSceneWorldWidget` for the webview.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum WorldWidgetDto {
    Label {
        text: String, font_size: f32, color: [f32; 4],
        local_position: [f32; 2], layout_size: [f32; 2],
    },
    Button {
        label: String, font_size: f32, label_color: [f32; 4],
        size: [f32; 2], local_position: [f32; 2],
        normal_color: [f32; 4], hover_color: [f32; 4], pressed_color: [f32; 4],
    },
    Image {
        asset_path: String, size: [f32; 2], local_position: [f32; 2], tint: [f32; 4],
    },
    Slider {
        min: f32, max: f32, value: f32,
        size: [f32; 2], local_position: [f32; 2],
        track_color: [f32; 4], fill_color: [f32; 4], thumb_color: [f32; 4], thumb_size: f32,
    },
    Toggle {
        checked: bool, size: [f32; 2], local_position: [f32; 2],
        track_off_color: [f32; 4], track_on_color: [f32; 4], thumb_color: [f32; 4],
    },
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

// ---------------------------------------------------------------------------
// Trigger-action (Phases 6 / 9 / 9a)
// ---------------------------------------------------------------------------

/// Mirrors `xrds_scene_graph::XrdsAction`. Same adjacent-tag JSON shape
/// (`{"kind": "...", "data": ...}`) as the real type, kept as a separate
/// wire type per this editor's DTO convention.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "kind", content = "data")]
pub enum XrdsActionDto {
    /// `clip_index` always addresses the real `XrdsSceneGltfAnimationSelector::Index`
    /// variant — the `Name(String)` selector isn't editable through this UI,
    /// matching the existing imperative `EditorCommand::PlayGltfAnimation`
    /// command's convention (also index-only).
    PlayGltfAnimation { clip_index: usize, speed: f32, repeat: String, start_paused: bool },
    StopGltfAnimation,
    SetVisible(bool),
    Teleport { destination: [f32; 3] },
    ModifyHealth { target: ActionTargetDto, delta: ActionValueDto },
    Wait { seconds: f32 },
    FireCustomEvent { name: String },
    Run { runnable: String, wait: bool },
    Unknown,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ActionTargetDto {
    SelfNode,
    Node { id: u64 },
    TriggerSource,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ActionValueDto {
    Fixed { value: f32 },
    FromTriggerSource,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct XrdsSequenceDto {
    pub steps: Vec<XrdsActionDto>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct XrdsTimelineKeyDto {
    pub at_secs: f32,
    pub action: XrdsActionDto,
}

/// Mirrors `xrds_scene_graph::XrdsRunnable` — a named registry entry is
/// either a `Sequence` or a `Timeline`, never both.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum RunnableBodyDto {
    Sequence { steps: Vec<XrdsActionDto> },
    Timeline { keys: Vec<XrdsTimelineKeyDto>, duration_secs: Option<f32>, looping: bool },
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct NamedRunnableDto {
    pub name: String,
    pub body: RunnableBodyDto,
}

/// Mirrors `xrds_scene_graph::XrdsTriggerKind`. Same adjacent-tag shape.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "kind", content = "data")]
pub enum XrdsTriggerKindDto {
    ZoneEnter,
    ZoneExit,
    Grabbed,
    Dropped,
    HoverEnter,
    HoverExit,
    ButtonPress,
    ButtonRelease,
    SliderChange,
    ToggleChange,
    AnimationComplete,
    RunawayDetected,
    Custom(String),
    Unknown,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TriggerBindingDto {
    pub trigger: XrdsTriggerKindDto,
    pub sequence: XrdsSequenceDto,
    pub disabled: bool,
    /// "Left" | "Right" | null.
    pub hand: Option<String>,
    pub runnable: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ObservableDto {
    RotationDegrees { axis: String },
    DistanceTo { node: u64 },
    Height,
    ScaleMagnitude,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ThresholdWatcherDto {
    pub observable: ObservableDto,
    /// "Above" | "Below" | "Either".
    pub crossing: String,
    pub value: f32,
    pub hysteresis: f32,
    pub fires: String,
    pub disabled: bool,
}

/// Mirrors `xrds_scene_graph::XrdsSceneTriggerDiagnostic`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TriggerDiagnosticDto {
    /// `None` for a registry-level problem (e.g. a `Run` cycle) — not any
    /// one node's fault.
    pub node_id: Option<u64>,
    /// "info" | "warning" | "error".
    pub severity: String,
    pub title: String,
    pub detail: String,
}

/// Addresses where an `XrdsAction` step list lives — a registry runnable's
/// body, or a specific trigger binding's inline sequence.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum StepTargetDto {
    Runnable { name: String },
    Binding { node_id: u64, binding_index: usize },
}
