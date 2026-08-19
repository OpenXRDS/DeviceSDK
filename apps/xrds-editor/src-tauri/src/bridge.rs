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
    /// Assigns (or with `texture_asset_id: None`, clears) **one** texture slot
    /// on a node's authored material.
    ///
    /// One slot per command rather than a whole-set write, for the same reason
    /// `XrdsAction::SetMaterial` takes one slot: replacing the set would let
    /// assigning a base-colour map silently drop an authored normal map.
    /// `slot` is "BaseColor" | "MetallicRoughness" | "Normal" | "Occlusion" |
    /// "Emissive".
    SetNodeMaterialTexture {
        id: u64,
        slot: String,
        texture_asset_id: Option<String>,
    },

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
    // The 12 HUD-library commands lived here. They are gone with
    // `XrdsHudTemplate`: a HUD is an `XrdsPanelTemplate` head-locked to an
    // anchor by parenting a `Panel` node under it — see
    // `SetPanelInstanceTemplate` below.
    // --- Panel template library (unified model) ---
    // Elements are addressed by **name**, never index: reordering must not
    // silently re-point a trigger binding.
    CreatePanelTemplate    { name: String },
    DeletePanelTemplate    { id: u64 },
    RenamePanelTemplate    { id: u64, name: String },
    SetPanelTemplateParams { id: u64, size: [f32; 2], color: [f32; 4], corner_radius: f32, opacity: f32 },
    /// kind: "Label" | "Button" | "Image" | "Slider" | "Toggle".
    AddPanelElement        { template_id: u64, kind: String, name: String },
    RemovePanelElement     { template_id: u64, name: String },
    RenamePanelElement     { template_id: u64, name: String, new_name: String },
    SetPanelElementWidget  { template_id: u64, name: String, widget: WorldWidgetDto },
    /// Element trigger bindings, addressed by `(Panel **node** id, element name,
    /// index)`.
    ///
    /// Node-scoped, not template-scoped: bindings live on the placed instance so
    /// two instances of one template can drive two different targets. The
    /// template-scoped versions of these six are gone — with them, an elevator
    /// panel on three floors fired all three doors from any one button.
    ///
    /// The element by *name* because reordering must not re-point a binding; the
    /// binding within one element by index, matching the node commands above,
    /// since nothing references a binding by position.
    AddPanelNodeTrigger    { id: u64, element: String },
    RemovePanelNodeTrigger { id: u64, element: String, index: usize },
    SetPanelNodeTriggerKind     { id: u64, element: String, index: usize, trigger: XrdsTriggerKindDto },
    SetPanelNodeTriggerTrack    { id: u64, element: String, index: usize, track: Option<String> },
    SetPanelNodeTriggerHand     { id: u64, element: String, index: usize, hand: Option<String> },
    SetPanelNodeTriggerDisabled { id: u64, element: String, index: usize, disabled: bool },
    /// effect: "Fire" | "Stop" — a panel button that stops a Track rather than
    /// starting one.
    SetPanelNodeTriggerEffect { id: u64, element: String, index: usize, effect: String },
    /// Repoint a scene-placed `Panel` node at a different template.
    ///
    /// Not `Option<u64>`: a Panel node *is* its template reference — clearing it
    /// would leave a node that can never render. Delete the node instead.
    SetPanelInstanceTemplate { id: u64, template_id: u64 },
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
    /// Radius and length (excluding the hemispherical caps) of a `Capsule` node.
    /// Same live-preview-on-every-event shape as `SetGravityScale`/`SetMass`.
    SetCapsuleGeometry { id: u64, radius: f32, length: f32 },
    /// Every tunable field of an `Effect` node, sent as a whole on each edit.
    /// Same live-preview-on-every-event shape as `SetCapsuleGeometry`: the
    /// frontend owns the current values and re-sends them all, so the backend
    /// needs no per-field commands and no partial-update merge logic.
    /// `when_finished` is "Restore" | "Keep".
    SetTrackAssetWhenFinished { track: String, asset_index: usize, when_finished: String },
    SetEffectParams {
        id: u64,
        /// "Burst" | "Trail"
        kind: String,
        auto_play: bool,
        burst_count: u32,
        spawn_rate: f32,
        lifetime_secs: f32,
        size_min: f32,
        size_max: f32,
        /// Linear RGBA, components <= 1.0 (brighter values are clamped by the
        /// runtime -- the XR cameras have no HDR pass).
        color_start: [f32; 4],
        color_end: [f32; 4],
        speed_min: f32,
        speed_max: f32,
        omnidirectional: bool,
        spread_deg: f32,
        gravity: [f32; 3],
        emission_radius: f32,
        /// "Blend" | "Add" | "Multiply"
        blend: String,
        size_end: f32,
        drag: f32,
        fade_edge: f32,
        fade_scene: f32,
    },

    // The 7 World Panel commands lived here (SetWorldPanelParams,
    // Add/Remove/MoveWorldPanelWidget, SetWorldPanelWidget(s),
    // SetWorldPanelLayout). Retired with `XrdsSceneWorldPanel`: inline widgets
    // carried no triggers, so every button on one was permanently dead. The
    // panel-template commands above are the live replacement.

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
    /// Scene-wide passthrough: `XrdsXrBlendMode::AlphaBlend` when true, `Opaque`
    /// otherwise. Has no visible effect in the editor viewport — passthrough is an
    /// XR compositor layer and desktop has no compositor — so the UI says so
    /// rather than leaving the author to wonder.
    SetXrPassthrough { enabled: bool },

    // --- Audio ---
    /// Audition a clip in the editor. `playing: false` stops and rewinds, so the
    /// next preview starts from the top.
    PreviewAudioClip { id: u64, playing: bool },
    /// Every field of an audio clip in one command, mirroring `SetEffectParams`.
    /// One command rather than nine keeps the document edit — and therefore the
    /// undo entry — a single step, which is what an author expects from dragging
    /// one slider.
    SetAudioClipParams {
        id: u64,
        asset_id: String,
        volume: f32,
        looped: bool,
        spatial: bool,
        autoplay: bool,
        distance_model: String,
        min_distance: f32,
        max_distance: f32,
        rolloff_factor: f32,
    },
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
    // `ExportGlb` removed: glTF cannot represent an XRDS scene (panels, triggers,
    // Tracks, anchors are dropped silently). glTF *import* is unaffected, and
    // `ExportApplication`/`ExportApk` below never depended on it.
    ExportApplication { output_dir: String },

    // --- Android / Quest export ---
    CheckApkPrerequisites,
    ExportApk { output_dir: String },

    // --- Trigger-action: runnable registry (document-level) ---
    /// kind: "sequence" | "timeline". Rejected (logged, no-op) if `name` is
    /// already taken.
    // --- Tracks: the registry ---
    CreateTrack { name: String },
    DeleteTrack { name: String },
    RenameTrack { old_name: String, new_name: String },
    SetTrackLooping  { name: String, looping: bool },
    SetTrackDuration { name: String, duration_secs: Option<f32> },

    // --- Tracks: asset rows ---
    /// Adds a row for `node_id`. Refused (with a status message) when that
    /// asset already has a row in this Track — one row per asset is the
    /// invariant `track_diagnostics` enforces, so the command layer should
    /// not be able to create the violation in the first place.
    AddTrackAsset    { track: String, node_id: u64 },
    /// Adds a row driving one element of one placed Panel node.
    ///
    /// Separate from `AddTrackAsset` rather than an optional field on it: an
    /// element target needs two values and a node target needs one, and a command
    /// where "the second field is only meaningful sometimes" is the shape that
    /// invites a half-filled payload.
    AddTrackElementAsset { track: String, panel: u64, element: String },
    RemoveTrackAsset { track: String, asset_index: usize },
    /// Repoints an existing row at a different node, keeping its events.
    SetTrackAssetTarget { track: String, asset_index: usize, node_id: u64 },

    // --- Tracks: events on a row ---
    /// kind: one of the `XrdsAction` variant names ("SetVisible", "SetTransform", ...).
    AddTrackKey    { track: String, asset_index: usize, at_secs: f32, kind: String },
    RemoveTrackKey { track: String, asset_index: usize, key_index: usize },
    SetTrackKey    { track: String, asset_index: usize, key_index: usize, key: XrdsTrackKeyDto },

    // --- Tracks: editor preview transport ---
    /// Starts `name` in the editor world without entering play mode. Separate
    /// from `SetPlayMode` by design: previewing one Track is not the same as
    /// running the simulation.
    PreviewPlayTrack  { name: String },
    PreviewPauseTrack { paused: bool },
    /// Stops the preview and restores every asset it moved from the document.
    PreviewStopTrack,

    // --- Internal, never sent by the frontend ---
    /// An inbound message the Rust side could not decode.
    ///
    /// Synthesised by `wry_overlay::ipc_handler` and routed through the ordinary
    /// command queue purely to reach `pending_status`, so a rejected command
    /// surfaces in the editor's own status bar instead of only a `warn!` in a
    /// terminal nobody is watching. Reusing the existing queue avoids a second
    /// channel for one message.
    ReportBridgeError { message: String },

    // --- Trigger-action: per-node bindings ---
    AddTriggerBinding    { node_id: u64 },
    RemoveTriggerBinding { node_id: u64, index: usize },
    SetTriggerBindingTrigger  { node_id: u64, index: usize, trigger: XrdsTriggerKindDto },
    /// hand: "Left" | "Right" | null.
    SetTriggerBindingHand     { node_id: u64, index: usize, hand: Option<String> },
    SetTriggerBindingDisabled { node_id: u64, index: usize, disabled: bool },
    /// effect: "Fire" | "Stop".
    SetTriggerBindingEffect { node_id: u64, index: usize, effect: String },
    SetTriggerBindingTrack    { node_id: u64, index: usize, track: Option<String> },

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

/// Bumped on **every** change to `EditorCommand`, `EditorSnapshot`, or any DTO
/// they contain.
///
/// `src/types/bridge.ts` is a hand-written mirror of this file with nothing
/// linking the two, so drift produces no compile error on either side: the Rust
/// build passes, `tsc` passes, and the failure only appears at runtime — a
/// dropped command that does nothing, or an `undefined` snapshot field that
/// throws on first `.map()`. No test crosses the boundary either.
///
/// This constant is the cheap guard. The frontend compares it against its own
/// copy and shows a hard "rebuild the UI" banner on mismatch, which turns both
/// of those silent failures into one clear message.
///
/// **If you change a DTO and do not bump this, you have removed the only thing
/// that would have told anyone.**
pub const BRIDGE_VERSION: u32 = 16;

/// State snapshot emitted to the webview after each frame's update.
/// Grow this incrementally — add fields as each phase is implemented.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct EditorSnapshot {
    /// See [`BRIDGE_VERSION`]. Compared by the frontend against its own copy.
    #[serde(default)]
    pub bridge_version: u32,
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
    /// Scene-wide passthrough (`xr_blend_mode == AlphaBlend`).
    #[serde(default)]
    pub xr_passthrough: bool,
    /// Whether this scene has ever been saved, i.e. whether it has a path.
    ///
    /// The frontend needs this to decide whether Ctrl+S can save at all: without a
    /// path, `SaveScene` has nothing to write to and must become Save As. Before
    /// this existed, Ctrl+S on a new scene silently did nothing.
    #[serde(default)]
    pub has_save_path: bool,
    /// All Camera nodes in the current scene (for the camera selector dropdown).
    pub available_cameras: Vec<CameraNodeDto>,
    /// None = editor camera active; Some(id) = scene camera node active.
    pub active_camera_id: Option<u64>,
    /// All PlayerAnchor nodes in the current scene.
    pub player_anchors: Vec<PlayerAnchorNodeDto>,
    /// None = all anchors active; Some(id) = only that anchor processes anchor systems.
    pub active_player_anchor_id: Option<u64>,
    /// Panel template library — the unified model, instanced either head-locked
    /// by a PlayerAnchor or placed in the scene by a Panel node.
    #[serde(default)]
    pub panel_library: Vec<PanelTemplateDto>,
    /// Every placed Panel node and its elements -- what the Sequencer needs to
    /// offer element rows. See [`PanelInstanceSummaryDto`].
    #[serde(default)]
    pub panel_instances: Vec<PanelInstanceSummaryDto>,
    /// Authoring problems with panel templates, kept separate from
    /// `track_diagnostics` so the panel workspace shows its own.
    #[serde(default)]
    pub panel_diagnostics: Vec<TriggerDiagnosticDto>,
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
    /// Document-level Track registry.
    #[serde(default)]
    pub tracks: Vec<NamedTrackDto>,
    /// Registry-level Track diagnostics only (`node_id: None`) — a duplicate
    /// asset row, a row targeting a deleted node, two Tracks sharing an asset,
    /// a looping Track blocking another. Per-node diagnostics ride on
    /// `selected_node.trigger_diagnostics` instead.
    #[serde(default)]
    pub track_diagnostics: Vec<TriggerDiagnosticDto>,
    /// Live editor preview, or `None` when nothing is previewing.
    #[serde(default)]
    pub track_preview: Option<TrackPreviewDto>,
    /// The most recent asset-conflict refusal. Kept in the snapshot because a
    /// refused Track is otherwise a silent no-op — see the reject-the-newcomer
    /// policy in `docs/done/xrds-track-model-plan.md` §4.
    #[serde(default)]
    pub track_conflict: Option<TrackConflictDto>,
    /// Every trigger binding in the document, tagged with its owning node
    /// — see `build_all_node_bindings_dto`. Powers the sequencer redesign's
    /// hierarchy-wide "Triggers" grouping without needing to select each
    /// node one at a time.
    #[serde(default)]
    pub all_node_bindings: Vec<NodeBindingSummaryDto>,
    /// Every threshold watcher in the document, tagged with its owning
    /// node — see `build_all_node_watchers_dto`. Same rationale as
    /// `all_node_bindings`, for the Watchers half of the same grouping.
    #[serde(default)]
    pub all_node_watchers: Vec<NodeWatcherSummaryDto>,
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
    Capsule { material: MaterialParamsDto, physics_body: String, gravity_scale: f32, mass: f32, radius: f32, length: f32 },
    Plane   { material: MaterialParamsDto, physics_body: String, gravity_scale: f32, mass: f32 },
    /// No `material` field, unlike the mesh primitives: an effect's colour is
    /// its own start/end gradient, not an `XrdsSceneMaterial`.
    Effect  {
        kind: String,
        auto_play: bool,
        burst_count: u32,
        spawn_rate: f32,
        lifetime_secs: f32,
        size_min: f32,
        size_max: f32,
        color_start: [f32; 4],
        color_end: [f32; 4],
        speed_min: f32,
        speed_max: f32,
        omnidirectional: bool,
        spread_deg: f32,
        gravity: [f32; 3],
        emission_radius: f32,
        /// "Blend" | "Add" | "Multiply"
        blend: String,
        size_end: f32,
        drag: f32,
        fade_edge: f32,
        fade_scene: f32,
    },
    /// An authored audio clip. Until 2026-08-19 this fell through to `Other`, so a
    /// placed audio node showed nothing at all in the inspector — its volume, loop,
    /// spatial flag and entire distance-falloff curve were reachable only from Rust.
    AudioClip {
        asset_id: String,
        volume: f32,
        looped: bool,
        spatial: bool,
        autoplay: bool,
        /// "Linear" | "Inverse" | "Exponential"
        distance_model: String,
        min_distance: f32,
        max_distance: f32,
        rolloff_factor: f32,
    },
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
    PlayerAnchor  { fov_deg: f32, is_initial: bool, exposure: Option<f32> },
    PlayerSpawnZone { size: [f32; 3], player_node_id: Option<u64> },
    /// A scene-placed instance of a panel template — the counterpart to a
    /// PlayerAnchor's head-locked link.
    ///
    /// Carries only the id; size, background and elements all live on the
    /// template, and the frontend resolves the name from `panel_library`.
    Panel { template_id: u64, elements: Vec<PanelInstanceElementDto> },
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
    /// Currently-assigned texture slots, **read-only in this DTO**.
    ///
    /// Writes go through `EditorCommand::SetNodeMaterialTexture` instead, one
    /// slot at a time. Deliberately asymmetric: this struct is also the live
    /// *drag* payload (sent on every pointer move), and letting it write
    /// textures would mean every drag frame round-tripping the whole slot set,
    /// with a stale frontend copy able to clobber slots it never touched.
    #[serde(default)]
    pub textures: MaterialTexturesDto,
}

/// The five texture slots, each holding an asset id or nothing. Mirrors
/// `XrdsSceneMaterialTextureSlots`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct MaterialTexturesDto {
    pub base_color: Option<String>,
    pub metallic_roughness: Option<String>,
    pub normal: Option<String>,
    pub occlusion: Option<String>,
    pub emissive: Option<String>,
}

impl Default for MaterialParamsDto {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0],
            textures: MaterialTexturesDto::default(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GltfClipDto {
    pub index: usize,
    pub name: String,
}

/// One element of a placed Panel node, reduced to what a picker needs.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PanelElementRefDto {
    pub name: String,
    /// "Label" | "Button" | "Image" | "Slider" | "Toggle".
    pub kind: String,
}

/// Every placed `Panel` node with the elements its template defines.
///
/// A whole-document summary, same rationale as `all_node_bindings`: the Sequencer
/// needs to offer *every* panel's elements as Track rows, and the snapshot
/// otherwise only carries the selected node's payload. `hierarchy` cannot serve
/// this — it has a node's name and kind but not its `template_id`, so it cannot
/// say which elements a Panel node has.
///
/// Deliberately thinner than `PanelInstanceElementDto`: no wiring, no
/// `emittable_triggers`. Those are per-selection detail, and computing them for
/// every panel every frame would be work nothing reads.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PanelInstanceSummaryDto {
    pub node_id: u64,
    pub node_name: String,
    pub elements: Vec<PanelElementRefDto>,
}

/// A reusable panel template — the unified model behind HUD panels and
/// world-space panels, where the only difference is attachment.
///
/// Carries **no placement**: depth belongs to the anchor that head-locks it, and
/// position to the node that places it in the scene. Mirrors
/// `XrdsPanelTemplate`, which enforces the same thing.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PanelTemplateDto {
    pub id: u64,
    pub name: String,
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub corner_radius: f32,
    pub opacity: f32,
    pub layout: WorldLayoutDto,
    pub elements: Vec<PanelElementDto>,
}

/// One named element on a panel.
///
/// `widget` reuses [`WorldWidgetDto`] rather than declaring a parallel five-kind
/// DTO, for the same reason the schema reuses `XrdsSceneWorldWidget`: a second
/// copy would drift, and an element genuinely *is* a named widget with triggers.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PanelElementDto {
    /// Unique within its template — **the addressing key**. Commands take this,
    /// never an index, so reordering cannot silently re-point a binding.
    pub name: String,
    pub widget: WorldWidgetDto,
    /// Which trigger kinds this element can actually emit, resolved server-side
    /// from `XrdsPanelElement::can_emit`.
    ///
    /// Sent rather than re-derived in TypeScript because the reachability rule
    /// is a runtime fact (a Label emits nothing; `Custom` needs a node id an
    /// element does not have), and a second copy of it would drift from the
    /// diagnostics that use the Rust one.
    ///
    /// No `triggers` field: a template carries no bindings. The Panels workspace
    /// designs panels; wiring happens on each placed node — see
    /// [`PanelInstanceElementDto`].
    pub emittable_triggers: Vec<String>,
}

/// One element of a placed Panel node: the template's element joined with *this
/// instance's* wiring.
///
/// Joined server-side so the Inspector does not have to cross-reference
/// `panel_library` itself — and so an orphaned binding (a key whose element the
/// template no longer has) can be surfaced rather than silently vanishing from
/// the list.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PanelInstanceElementDto {
    pub name: String,
    /// "Label" | "Button" | "Image" | "Slider" | "Toggle", or "missing" when
    /// `orphaned`.
    pub kind: String,
    pub emittable_triggers: Vec<String>,
    pub triggers: Vec<TriggerBindingDto>,
    /// True when this row exists only because the instance has wiring for a name
    /// the template does not define — the shape a deleted element leaves behind.
    pub orphaned: bool,
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
    /// `count: None` uses the effect node's authored Burst Count; `Some` overrides
    /// it, so one effect can be fired at different intensities from different
    /// triggers without duplicating the node.
    PlayEffect { count: Option<u32> },
    /// Stops emitting; particles already alive fade out rather than vanishing.
    StopEffect,
    /// Play the target audio clip, restarting it if it has already finished.
    /// No clip field: the target node *is* the clip, as with PlayEffect.
    PlayAudio,
    /// Stop the target audio clip and rewind, so it can be played again.
    StopAudio,
    SetVisible(bool),
    /// `ease` is "Linear" | "Quad" | "Cubic" — same plain-String-for-a-
    /// closed-set convention as `repeat`/`hand`/`crossing` elsewhere in
    /// this file, rather than a dedicated DTO enum.
    SetTransform {
        position: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
        duration_secs: f32,
        ease: String,
    },
    /// No `target` field: applies to whichever asset row it sits on, same as
    /// every other action. See `XrdsAction::SetMaterial`'s doc comment on the
    /// Rust domain side for why the old per-action target was removed.
    SetMaterial {
        base_color: Option<[f32; 4]>,
        metallic: Option<f32>,
        roughness: Option<f32>,
        /// One texture slot assignment; `None` leaves all slots alone.
        texture: Option<ActionTextureDto>,
    },
    ModifyHealth { delta: ActionValueDto },
    /// Element-scoped actions. Only meaningful on an `Element` asset row -- a
    /// node has no text, scalar or enabled state of this kind.
    SetElementText { text: String },
    SetElementValue { value: f32 },
    SetElementEnabled { enabled: bool },
    Unknown,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ActionTargetDto {
    SelfNode,
    Node { id: u64 },
    TriggerSource,
    /// One named element on one placed Panel node. Two fields because an element
    /// has no id of its own -- it is not a document node.
    Element { panel: u64, name: String },
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ActionValueDto {
    Fixed { value: f32 },
    FromTriggerSource,
}

/// One texture-slot assignment for `SetMaterial`.
///
/// `slot` is a plain String over a closed set ("BaseColor" |
/// "MetallicRoughness" | "Normal" | "Occlusion" | "Emissive"), the same
/// convention `repeat`/`hand`/`ease` already use here rather than a dedicated
/// DTO enum. `texture_asset_id` of `None` clears the slot.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ActionTextureDto {
    pub slot: String,
    pub texture_asset_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct XrdsTrackKeyDto {
    pub at_secs: f32,
    pub action: XrdsActionDto,
}

/// One asset row. `node_id` is `None` for a `SelfNode`/`TriggerSource` row,
/// which has no concrete node until the Track is fired — the frontend renders
/// those rows with a placeholder label rather than a node name.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct XrdsTrackAssetDto {
    pub target: ActionTargetDto,
    /// Resolved display name for a `Node` target, so the frontend does not
    /// have to walk the hierarchy to label a row.
    pub node_name: Option<String>,
    pub keys: Vec<XrdsTrackKeyDto>,    /// "Restore" | "Keep" — what happens to this row's node when the Track
    /// finishes on its own. Mirrors Unreal Sequencer's per-track `When Finished`.
    pub when_finished: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct NamedTrackDto {
    pub name: String,
    pub assets: Vec<XrdsTrackAssetDto>,
    pub duration_secs: Option<f32>,
    /// What the ruler should span: `duration_secs` when set, otherwise the
    /// span the events actually occupy including a trailing interpolation.
    /// Computed here so the editor and the runtime cannot disagree.
    pub effective_duration_secs: f32,
    pub looping: bool,
}

/// Live editor-preview state, or `None` when nothing is previewing. Drives the
/// transport readout and the playhead.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TrackPreviewDto {
    pub name: String,
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub playing: bool,
}

/// The most recent refusal by the asset-conflict guard, so a Track that
/// silently did not start is diagnosable from the UI.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TrackConflictDto {
    pub blocked_track: String,
    /// Node names where resolvable, so the message can say "crane_arm" rather
    /// than an opaque entity id.
    pub contended: Vec<String>,
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

/// One binding, tagged with its owning node — see
/// `EditorSnapshot::all_node_bindings`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct NodeBindingSummaryDto {
    pub node_id: u64,
    pub node_name: String,
    pub binding_index: usize,
    pub binding: TriggerBindingDto,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TriggerBindingDto {
    pub trigger: XrdsTriggerKindDto,
    /// "Fire" or "Stop" — whether this binding starts or stops its Track.
    ///
    /// A stop button is the motivating case. Two bindings on one element, Stop
    /// then Fire, restart a Track from the top with no conditional.
    pub effect: String,
    pub disabled: bool,
    /// "Left" | "Right" | null.
    pub hand: Option<String>,
    /// The Track this binding fires, or `None` for authored-but-unwired.
    /// There is no inline alternative — see `XrdsTriggerBinding::track`.
    pub track: Option<String>,
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

/// One watcher, tagged with its owning node — see
/// `EditorSnapshot::all_node_watchers`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct NodeWatcherSummaryDto {
    pub node_id: u64,
    pub node_name: String,
    pub watcher_index: usize,
    pub watcher: ThresholdWatcherDto,
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


