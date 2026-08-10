// TypeScript mirrors of the Rust EditorCommand / EditorSnapshot types in bridge.rs

// ---------------------------------------------------------------------------
// Snapshot (Bevy → webview)
// ---------------------------------------------------------------------------

export interface HierarchyNode {
  id: number;
  name: string;
  kind: string;
  visible: boolean;
  children: HierarchyNode[];
}

export interface AssetCatalogEntry {
  id: string;
  name: string;
  kind: string;
}

/** A reusable panel template — the unified model behind HUD panels and
 *  world-space panels, where the only difference is attachment.
 *
 *  Carries **no placement**: depth belongs to the anchor that head-locks it,
 *  position to the node that places it in the scene. */
export interface PanelTemplateDto {
  id: number;
  name: string;
  size: [number, number];
  color: [number, number, number, number];
  corner_radius: number;
  opacity: number;
  layout: WorldLayout;
  elements: PanelElementDto[];
}

/** One named element on a panel.
 *
 *  `widget` reuses {@link WorldWidget} rather than a parallel five-kind type,
 *  for the same reason the schema reuses `XrdsSceneWorldWidget`: a second copy
 *  would drift, and an element genuinely *is* a named widget with triggers. */
export interface PanelElementDto {
  /** Unique within its template — **the addressing key**. Commands take this,
   *  never an index, so reordering cannot silently re-point a binding. */
  name: string;
  widget: WorldWidget;
  /** Which trigger kinds this element can actually emit.
   *
   *  Resolved server-side from `XrdsPanelElement::can_emit` rather than
   *  re-derived here: reachability is a runtime fact (a Label emits nothing;
   *  `Custom` needs a node id an element does not have), and a second copy
   *  would drift from the Rust diagnostics that use the original.
   *
   *  There is no `triggers` field: a template carries no bindings. The Panels
   *  workspace designs panels; wiring happens per placed node — see
   *  {@link PanelInstanceElementDto}. */
  emittable_triggers: string[];
}

/** One element of a panel template, reduced to what a picker needs. */
export interface PanelElementRefDto {
  name: string;
  /** "Label" | "Button" | "Image" | "Slider" | "Toggle". */
  kind: string;
}

/** A placed Panel node with the elements its template defines.
 *
 *  Thinner than {@link PanelInstanceElementDto} on purpose — no wiring, no
 *  emittable set. Those are per-selection detail; computing them for every panel
 *  every frame would be work nothing reads. */
export interface PanelInstanceSummaryDto {
  node_id: number;
  node_name: string;
  elements: PanelElementRefDto[];
}

/** One element of a *placed* Panel node: the template's element joined with this
 *  instance's wiring.
 *
 *  Joined server-side so the Inspector need not cross-reference `panel_library`,
 *  and so an orphaned binding (a key whose element the template no longer has)
 *  stays visible instead of silently vanishing from the list while remaining in
 *  the saved file. */
export interface PanelInstanceElementDto {
  name: string;
  /** "Label" | "Button" | "Image" | "Slider" | "Toggle", or "missing" when
   *  `orphaned`. */
  kind: string;
  emittable_triggers: string[];
  triggers: TriggerBindingDto[];
  /** True when this row exists only because the instance has wiring for a name
   *  the template does not define — what a deleted element leaves behind. */
  orphaned: boolean;
}

export interface MaterialParams {
  base_color: [number, number, number, number];
  metallic: number;
  roughness: number;
  emissive: [number, number, number];
  /** Currently-assigned texture slots. **Read-only here** — writes go via
   *  `SetNodeMaterialTexture`, one slot at a time. This same struct is the
   *  live drag payload, so letting it write textures would round-trip the
   *  whole slot set on every pointer move and let a stale copy clobber slots
   *  it never touched. */
  textures: MaterialTextures;
}

/** The five texture slots, each an asset id or `null`. */
export interface MaterialTextures {
  base_color: string | null;
  metallic_roughness: string | null;
  normal: string | null;
  occlusion: string | null;
  emissive: string | null;
}

/** Radix `Select.Item` forbids `value=""`, so "no selection" needs a stand-in.
 *  Exported rather than declared per-component: two copies of a sentinel that
 *  drift apart would silently stop matching, and the wire convention stays `null`
 *  either way — these never leave the UI. */
export const TRACK_NONE_SENTINEL = "__none__";
export const HAND_ANY_SENTINEL = "__any__";

/** Slot keys of {@link MaterialTextures}, paired with the wire name
 *  `SetNodeMaterialTexture` expects and a human label. One list so the UI
 *  cannot drift from the command's accepted values. */
export const MATERIAL_TEXTURE_SLOTS = [
  { key: "base_color", wire: "BaseColor", label: "Base Color" },
  { key: "metallic_roughness", wire: "MetallicRoughness", label: "Metal/Rough" },
  { key: "normal", wire: "Normal", label: "Normal" },
  { key: "occlusion", wire: "Occlusion", label: "Occlusion" },
  { key: "emissive", wire: "Emissive", label: "Emissive" },
] as const;

export type NodePayload =
  | { type: "Empty" }
  | { type: "Cube";     material: MaterialParams; physics_body: string; gravity_scale: number; mass: number }
  | { type: "Sphere";   material: MaterialParams; physics_body: string; gravity_scale: number; mass: number }
  | { type: "Cylinder"; material: MaterialParams; physics_body: string; gravity_scale: number; mass: number }
  | { type: "Plane";    material: MaterialParams; physics_body: string; gravity_scale: number; mass: number }
  | { type: "Camera";   fov: number; near: number; far: number }
  | { type: "PointLight";       color: [number,number,number,number]; intensity: number; range: number }
  | { type: "DirectionalLight"; color: [number,number,number,number]; illuminance: number }
  | { type: "SpotLight";        color: [number,number,number,number]; intensity: number; range: number; inner_angle: number; outer_angle: number }
  | { type: "AmbientLight";     color: [number,number,number,number]; brightness: number }
  | { type: "Text";         text: string; font_size: number; color: [number,number,number,number]; alignment: string; anchor: string; anchor_param: number }
  | { type: "ExtrudedText"; text: string; font_size: number; depth: number; color: [number,number,number,number]; alignment: string }
  | { type: "GltfAsset";    clips: { index: number; name: string }[] }
  | { type: "HudText";   text: string; font_size: number; color: [number,number,number,number]; anchor: string; offset: [number,number] }
  | { type: "Player" }
  // `panel_template_id` + `panel_depth` replace `hud_template_id`. Depth is here
  // rather than on the template so two anchors can share one at different
  // distances — the limitation that retired `XrdsHudTemplate::depth`.
  | { type: "PlayerAnchor"; fov_deg: number; is_initial: boolean; panel_template_id: number | null; panel_depth: number; exposure: number | null }
  | { type: "PlayerSpawnZone"; size: [number, number, number]; player_node_id: number | null }
  // `WorldPanel` lived here: a panel with widgets stored inline, and no triggers
  // field to wire them — every button on one was permanently dead.
  //
  // A scene-placed instance of a panel template — the counterpart to an anchor's
  // head-locked link. Only the id travels; the name is resolved from
  // `panel_library` so a rename cannot leave a stale copy behind.
  | { type: "Panel"; template_id: number; elements: PanelInstanceElementDto[] }
  | { type: "Other";        kind: string };

/** Which editor layout is active.
 *
 *  `scene` and `sequencer` both keep the Bevy viewport live — the Sequencer just
 *  gives it less room. `panels` hides it outright: panel design is 2D, so a live
 *  viewport would only be a hole to click through by accident. */
export type Workspace = "scene" | "sequencer" | "panels";

export type RGBA = [number, number, number, number];

export type WorldLayout =
  | { type: "None" }
  | { type: "VStack"; gap: number }
  | { type: "HStack"; gap: number }
  | { type: "Grid"; cols: number; gap: [number, number] };

export type WorldWidget =
  | { type: "Label";  text: string; font_size: number; color: RGBA; local_position: [number, number]; layout_size: [number, number] }
  | { type: "Button"; label: string; font_size: number; label_color: RGBA; size: [number, number]; local_position: [number, number]; normal_color: RGBA; hover_color: RGBA; pressed_color: RGBA }
  | { type: "Image";  asset_path: string; size: [number, number]; local_position: [number, number]; tint: RGBA }
  | { type: "Slider"; min: number; max: number; value: number; size: [number, number]; local_position: [number, number]; track_color: RGBA; fill_color: RGBA; thumb_color: RGBA; thumb_size: number }
  | { type: "Toggle"; checked: boolean; size: [number, number]; local_position: [number, number]; track_off_color: RGBA; track_on_color: RGBA; thumb_color: RGBA };

export interface NodeInspector {
  id: number;
  name: string;
  visible: boolean;
  grabbable: boolean;
  parent_id: number | null;
  translation: [number, number, number];
  rotation_euler_degrees: [number, number, number];
  scale: [number, number, number];
  payload: NodePayload;
  parent_kind: string | null;
  /** Applies regardless of `payload` kind — any node can carry triggers. */
  triggers: TriggerBindingDto[];
  watchers: ThresholdWatcherDto[];
  /** This node's subset of trigger_diagnostics (`node_id === this node`). */
  trigger_diagnostics: TriggerDiagnosticDto[];
}

// ---------------------------------------------------------------------------
// Trigger-action (Phases 6 / 9 / 9a)
// ---------------------------------------------------------------------------

/** Texture slots a `SetMaterial` event can drive. */
export const TEXTURE_SLOTS = [
  "BaseColor", "MetallicRoughness", "Normal", "Occlusion", "Emissive",
] as const;

/** One texture-slot assignment. `texture_asset_id: null` *clears* the slot,
 *  which is why it is nullable rather than the whole `texture` being absent —
 *  "clear the normal map at t=2s" is a real thing to author. */
export interface ActionTexture {
  slot: string;
  texture_asset_id: string | null;
}

export type XrdsAction =
  | { kind: "PlayGltfAnimation"; data: { clip_index: number; speed: number; repeat: string; start_paused: boolean } }
  | { kind: "StopGltfAnimation" }
  | { kind: "SetVisible"; data: boolean }
  | {
      kind: "SetTransform";
      data: {
        position: [number, number, number] | null;
        rotation: [number, number, number] | null;
        scale: [number, number, number] | null;
        duration_secs: number;
        /** "Linear" | "Quad" | "Cubic". */
        ease: string;
      };
    }
  // No `target` field on either of these: both apply to whichever asset row
  // they sit on, same as every other action. They used to carry their own
  // target — a leftover from before rows were asset-scoped — which meant one
  // could silently apply to a *different* node than its row, invisibly to
  // the cross-Track conflict check.
  | {
      kind: "SetMaterial";
      data: {
        base_color: [number, number, number, number] | null;
        metallic: number | null;
        roughness: number | null;
        /** One texture slot assignment; `null` leaves every slot alone. */
        texture: ActionTexture | null;
      };
    }
  | { kind: "ModifyHealth"; data: { delta: ActionValue } }
  // Element-scoped. Only meaningful on an `Element` asset row — a node has no
  // text, scalar or enabled state of this kind.
  | { kind: "SetElementText"; data: { text: string } }
  | { kind: "SetElementValue"; data: { value: number } }
  | { kind: "SetElementEnabled"; data: { enabled: boolean } }
  /** An action this build does not recognize — from a newer editor. Skipped
   *  at runtime and reported by `track_diagnostics`. */
  | { kind: "Unknown" };

export type ActionTarget =
  | { type: "SelfNode" }
  | { type: "Node"; id: number }
  | { type: "TriggerSource" }
  // One named element on one placed Panel node. Two fields because an element has
  // no id of its own — it is not a document node — and going through the panel is
  // what makes two instances of one template two different targets.
  | { type: "Element"; panel: number; name: string };

export type ActionValue =
  | { type: "Fixed"; value: number }
  | { type: "FromTriggerSource" };


export type XrdsTriggerKind =
  | { kind: "ZoneEnter" }
  | { kind: "ZoneExit" }
  | { kind: "Grabbed" }
  | { kind: "Dropped" }
  | { kind: "HoverEnter" }
  | { kind: "HoverExit" }
  | { kind: "ButtonPress" }
  | { kind: "ButtonRelease" }
  | { kind: "SliderChange" }
  | { kind: "ToggleChange" }
  | { kind: "AnimationComplete" }
  | { kind: "RunawayDetected" }
  | { kind: "Custom"; data: string }
  | { kind: "Unknown" };

export interface XrdsTrackKeyDto {
  at_secs: number;
  action: XrdsAction;
}

/** One asset row inside a Track — a node plus every event on it.
 *
 *  `node_name` is resolved server-side so a row can be labelled without
 *  walking the hierarchy. It is `null` for a `SelfNode`/`TriggerSource` row
 *  (no concrete node until the Track is fired) and also for a `Node` target
 *  that no longer exists — that second case is separately diagnosed. */
export interface XrdsTrackAssetDto {
  target: ActionTarget;
  node_name: string | null;
  keys: XrdsTrackKeyDto[];
}

export interface NamedTrackDto {
  name: string;
  assets: XrdsTrackAssetDto[];
  duration_secs: number | null;
  /** What the ruler should span: `duration_secs` when set, otherwise the span
   *  the events occupy including a trailing interpolation. Computed in Rust so
   *  the editor and the runtime cannot disagree. */
  effective_duration_secs: number;
  looping: boolean;
}

/** Live editor preview, or `null` when nothing is previewing. */
export interface TrackPreviewDto {
  name: string;
  elapsed_secs: number;
  duration_secs: number;
  playing: boolean;
}

/** The most recent asset-conflict refusal. In the snapshot because a refused
 *  Track is otherwise a silent no-op — see the reject-the-newcomer policy. */
export interface TrackConflictDto {
  blocked_track: string;
  contended: string[];
}

/** Whether a binding starts or stops its Track.
 *
 *  A stop button is the motivating case: without it, first-run priority means a
 *  running Track cannot be interrupted from authored content at all. Two bindings
 *  on one element — Stop then Fire — restart a Track from the top, which is how a
 *  restart button avoids needing an "is it running?" condition. */
export type TriggerEffect = "Fire" | "Stop";

export const TRIGGER_EFFECTS: TriggerEffect[] = ["Fire", "Stop"];

export interface TriggerBindingDto {
  trigger: XrdsTriggerKind;
  effect: TriggerEffect;
  disabled: boolean;
  /** "Left" | "Right" | null. */
  hand: string | null;
  /** The Track this binding fires, or `null` for authored-but-unwired. There
   *  is deliberately no inline alternative. */
  track: string | null;
}

export type ObservableDto =
  | { type: "RotationDegrees"; axis: string }
  | { type: "DistanceTo"; node: number }
  | { type: "Height" }
  | { type: "ScaleMagnitude" };

export interface ThresholdWatcherDto {
  observable: ObservableDto;
  /** "Above" | "Below" | "Either". */
  crossing: string;
  value: number;
  hysteresis: number;
  fires: string;
  disabled: boolean;
}

/** One watcher, tagged with its owning node — see `EditorSnapshot.all_node_watchers`. */
export interface NodeWatcherSummary {
  node_id: number;
  node_name: string;
  watcher_index: number;
  watcher: ThresholdWatcherDto;
}

export interface TriggerDiagnosticDto {
  /** null for a registry-level problem (e.g. a Run cycle) — not any one node's fault. */
  node_id: number | null;
  /** "info" | "warning" | "error". */
  severity: string;
  title: string;
  detail: string;
}

export interface CameraNodeDto {
  id: number;
  name: string;
}

export interface PlayerAnchorEntry {
  id: number;
  name: string;
  player_name: string;
}

export interface ApkPrerequisite {
  name: string;
  ok: boolean;
  hint: string;
}

/** Must match `BRIDGE_VERSION` in `src-tauri/src/bridge.rs`.
 *
 *  This file is a hand-written mirror of that one with nothing linking them, so
 *  drift produces no compile error on either side. It fails only at runtime, and
 *  quietly: an unknown command is dropped Rust-side (and `useSendCommand` is
 *  fire-and-forget, so nothing here ever learns), while a removed snapshot field
 *  arrives as `undefined` and throws on the first `.map()`. `defaultSnapshot`
 *  does not shield that — it is only the initial `useState` value and is
 *  replaced wholesale by the first real snapshot.
 *
 *  Bump this together with the Rust constant whenever a DTO changes. */
export const BRIDGE_VERSION = 16;

export interface EditorSnapshot {
  /** See {@link BRIDGE_VERSION}. `0` means a build predating the check. */
  bridge_version: number;
  hierarchy: HierarchyNode[];
  selection: number[];
  selected_node: NodeInspector | null;
  asset_catalog: AssetCatalogEntry[];
  undo_count: number;
  redo_count: number;
  is_dirty: boolean;
  scene_name: string;
  status_message: string | null;
  gizmo_mode: string;
  camera_mode: string;
  show_grid: boolean;
  show_fov_overlay: boolean;
  is_playing: boolean;
  snap_step: number;
  is_exporting: boolean;
  has_clipboard: boolean;
  environment: EnvironmentDto | null;
  available_cameras: CameraNodeDto[];
  active_camera_id: number | null;
  player_anchors: PlayerAnchorEntry[];
  active_player_anchor_id: number | null;
  /** Panel template library — the unified model. */
  panel_library: PanelTemplateDto[];
  /** Every placed Panel node and its elements — what the Sequencer needs to offer
   *  element rows. `hierarchy` cannot serve this: it carries a node's name and
   *  kind but not its `template_id`, so it cannot say which elements a Panel has. */
  panel_instances: PanelInstanceSummaryDto[];
  /** Panel authoring problems, separate from `track_diagnostics` so the panel
   *  workspace shows its own. */
  panel_diagnostics: TriggerDiagnosticDto[];
  stereo_preview_active: boolean;
  /** Populated for one frame after CheckApkPrerequisites; null otherwise. */
  apk_prerequisites: ApkPrerequisite[] | null;
  is_exporting_apk: boolean;
  /** Tail of the APK build log (last ≤200 lines). Empty when idle. */
  apk_build_log: string[];
  /** Document-level named-runnable registry (Phase 9a). */
  tracks: NamedTrackDto[];
  /** Registry-level trigger diagnostics only (node_id === null). */
  track_diagnostics: TriggerDiagnosticDto[];
  track_preview: TrackPreviewDto | null;
  track_conflict: TrackConflictDto | null;
  /** Every trigger binding in the document, tagged with its owning node —
   * not just the selected node's. Powers the sequencer redesign's
   * hierarchy-wide "Triggers" grouping and reverse lookup. */
  all_node_bindings: NodeBindingSummary[];
  /** Every threshold watcher in the document, tagged with its owning node
   * — same rationale as `all_node_bindings`. */
  all_node_watchers: NodeWatcherSummary[];
}

/** One binding, tagged with its owning node — see `EditorSnapshot.all_node_bindings`. */
export interface NodeBindingSummary {
  node_id: number;
  node_name: string;
  binding_index: number;
  binding: TriggerBindingDto;
}

export interface EnvironmentDto {
  fog_enabled: boolean;  fog_color: [number,number,number,number]; fog_start: number; fog_end: number;
  exposure_enabled: boolean; ev100: number;
  ibl_enabled: boolean; ibl_diffuse: string; ibl_specular: string; ibl_intensity: number;
  skybox_enabled: boolean; skybox_asset: string; skybox_brightness: number;
}

export const defaultSnapshot: EditorSnapshot = {
  bridge_version: BRIDGE_VERSION,
  hierarchy: [],
  selection: [],
  selected_node: null,
  asset_catalog: [],
  undo_count: 0,
  redo_count: 0,
  is_dirty: false,
  scene_name: "—",
  status_message: null,
  gizmo_mode: "Translate",
  camera_mode: "Orbit",
  show_grid: true,
  show_fov_overlay: false,
  is_playing: false,
  snap_step: 0.25,
  is_exporting: false,
  has_clipboard: false,
  environment: null,
  available_cameras: [],
  active_camera_id: null,
  player_anchors: [],
  active_player_anchor_id: null,
  panel_library: [],
  panel_instances: [],
  panel_diagnostics: [],
  stereo_preview_active: false,
  apk_prerequisites: null,
  is_exporting_apk: false,
  apk_build_log: [],
  tracks: [],
  track_diagnostics: [],
  track_preview: null,
  track_conflict: null,
  all_node_bindings: [],
  all_node_watchers: [],
};

// ---------------------------------------------------------------------------
// Commands (webview → Bevy)  — a subset; extend as needed
// ---------------------------------------------------------------------------

export type EditorCommand =
  | { type: "SelectNode";      payload: { id: number } }
  | { type: "MultiSelectNode"; payload: { id: number; extend: boolean } }
  | { type: "DeselectAll" }
  | { type: "RenameNode";   payload: { id: number; name: string } }
  | { type: "DeleteNode";   payload: { id: number } }
  | { type: "DuplicateNode";payload: { id: number } }
  | { type: "ReparentNode";  payload: { id: number; new_parent_id: number | null; index: number } }
  | { type: "SpawnPrimitive"; payload: { kind: string; parent_id: number | null } }
  | { type: "SpawnAsset";     payload: { asset_id: string; parent_id: number | null } }
  | { type: "SetTranslation";   payload: { id: number; value: [number,number,number] } }
  | { type: "SetRotationEuler"; payload: { id: number; degrees: [number,number,number] } }
  | { type: "SetScale";         payload: { id: number; value: [number,number,number] } }
  | { type: "CommitTransform";  payload: { id: number; translation: [number,number,number]; rotation_euler_degrees: [number,number,number]; scale: [number,number,number] } }
  | { type: "SetMaterial";   payload: { id: number; params: MaterialParams } }
  | { type: "CommitMaterial"; payload: { id: number; params: MaterialParams } }
  /** Assigns (or with `texture_asset_id: null`, clears) one texture slot on a
   *  node's authored material. `slot` is a `MATERIAL_TEXTURE_SLOTS[].wire`. */
  | { type: "SetNodeMaterialTexture"; payload: { id: number; slot: string; texture_asset_id: string | null } }
  | { type: "SetPointLight";       payload: { id: number; color: [number,number,number,number]; intensity: number; range: number } }
  | { type: "SetDirectionalLight"; payload: { id: number; color: [number,number,number,number]; illuminance: number } }
  | { type: "SetSpotLight";        payload: { id: number; color: [number,number,number,number]; intensity: number; range: number; inner_angle: number; outer_angle: number } }
  | { type: "SetAmbientLight";     payload: { id: number; color: [number,number,number,number]; brightness: number } }
  | { type: "SetVisible";    payload: { id: number; visible: boolean } }
  | { type: "SetGrabbable"; payload: { id: number; grabbable: boolean } }
  | { type: "SetFog";       payload: { color: [number,number,number,number]; start: number; end: number } }
  | { type: "ClearFog" }
  | { type: "SetExposure";  payload: { ev100: number } }
  | { type: "ClearExposure" }
  | { type: "SetIbl";       payload: { diffuse_asset_id: string; specular_asset_id: string; intensity: number } }
  | { type: "ClearIbl" }
  | { type: "SetSkybox";    payload: { texture_asset_id: string; brightness: number } }
  | { type: "ClearSkybox" }
  | { type: "SetHudText";         payload: { id: number; text: string; font_size: number; color: [number,number,number,number]; anchor: string; offset: [number,number] } }
  // The 12 HudTemplate/HudItem commands are gone with `XrdsHudTemplate`: a HUD is
  // a panel template head-locked to an anchor, so the panel commands below cover
  // every one of them, and `LinkHudTemplate` became `LinkPanelTemplate`.
  | { type: "SetCameraParams";    payload: { id: number; fov: number; near: number; far: number } }
  | { type: "CommitCameraParams"; payload: { id: number; fov: number; near: number; far: number } }
  | { type: "SetPlayerAnchorFov";      payload: { id: number; fov_deg: number } }
  | { type: "SetPlayerAnchorInitial";  payload: { id: number; is_initial: boolean } }
  | { type: "SetPlayerAnchorExposure"; payload: { id: number; ev100: number | null } }
  | { type: "SetSpawnZoneSize";        payload: { id: number; size: [number, number, number] } }
  | { type: "SetSpawnZonePlayer";      payload: { id: number; player_node_id: number | null } }
  // The 7 WorldPanel commands lived here. Retired with the payload: inline
  // widgets carried no triggers, so every button on one was permanently dead.
  // --- Panel template library (unified model) ---
  // Elements are addressed by **name**, never index.
  | { type: "CreatePanelTemplate";    payload: { name: string } }
  | { type: "DeletePanelTemplate";    payload: { id: number } }
  | { type: "RenamePanelTemplate";    payload: { id: number; name: string } }
  | { type: "SetPanelTemplateParams"; payload: { id: number; size: [number, number]; color: [number,number,number,number]; corner_radius: number; opacity: number } }
  | { type: "AddPanelElement";        payload: { template_id: number; kind: string; name: string } }
  | { type: "RemovePanelElement";     payload: { template_id: number; name: string } }
  | { type: "RenamePanelElement";     payload: { template_id: number; name: string; new_name: string } }
  | { type: "SetPanelElementWidget";  payload: { template_id: number; name: string; widget: WorldWidget } }
  // Element trigger bindings. The *element* by name (reordering must not
  // re-point a binding); the binding within it by index, like node bindings.
  // Node-scoped, not template-scoped: bindings live on the placed instance so two
  // instances of one template can drive two different targets.
  | { type: "AddPanelNodeTrigger";    payload: { id: number; element: string } }
  | { type: "RemovePanelNodeTrigger"; payload: { id: number; element: string; index: number } }
  | { type: "SetPanelNodeTriggerKind";     payload: { id: number; element: string; index: number; trigger: XrdsTriggerKind } }
  | { type: "SetPanelNodeTriggerTrack";    payload: { id: number; element: string; index: number; track: string | null } }
  | { type: "SetPanelNodeTriggerHand";     payload: { id: number; element: string; index: number; hand: string | null } }
  | { type: "SetPanelNodeTriggerDisabled"; payload: { id: number; element: string; index: number; disabled: boolean } }
  | { type: "LinkPanelTemplate"; payload: { anchor_id: number; template_id: number | null; depth: number } }
  // Not nullable, unlike LinkPanelTemplate: a Panel node *is* its template
  // reference, so clearing it would leave a node that can never render.
  | { type: "SetPanelInstanceTemplate"; payload: { id: number; template_id: number } }
  | { type: "SetPhysicsBody";          payload: { id: number; physics_body: string } }
  | { type: "SetGravityScale";         payload: { id: number; value: number } }
  | { type: "SetMass";                 payload: { id: number; value: number } }
  | { type: "SetActiveCamera";    payload: { id: number | null } }
  | { type: "SetActivePlayerAnchor"; payload: { id: number | null } }
  | { type: "PreviewFromAnchor";     payload: { id: number } }
  | { type: "CommitLight";  payload: { id: number } }
  | { type: "PlayGltfAnimation";  payload: { id: number; clip_index: number; speed: number; repeat: string } }
  | { type: "StopGltfAnimation";  payload: { id: number } }
  | { type: "PauseGltfAnimation"; payload: { id: number } }
  | { type: "ResumeGltfAnimation"; payload: { id: number } }
  | { type: "SetTextContent";  payload: { id: number; text: string; font_size: number; color: [number,number,number,number]; alignment: string; anchor: string; anchor_param: number } }
  | { type: "SetExtrudedText"; payload: { id: number; text: string; font_size: number; depth: number; color: [number,number,number,number]; alignment: string } }
  | { type: "SetExtrudedTextColor"; payload: { id: number; color: [number,number,number,number] } }
  | { type: "SetGizmoMode";   payload: { mode: string } }
  | { type: "SetCameraMode";  payload: { mode: string } }
  | { type: "ToggleGrid" }
  | { type: "ToggleFovOverlay" }
  | { type: "FrameSelected" }
  | { type: "SetPlayMode"; payload: { playing: boolean } }
  | { type: "Undo" }
  | { type: "Redo" }
  | { type: "NewScene" }
  | { type: "OpenScene";     payload: { path: string } }
  | { type: "ImportAsset";   payload: { path: string } }
  | { type: "RemoveAsset";   payload: { asset_id: string } }
  // `ExportGlb` removed: glTF has no vocabulary for panels, triggers, Tracks,
  // anchors or zones, so a scene export wrote a mesh dump that looked complete.
  // glTF *import* is unaffected, and `ExportApplication`/`ExportApk` never
  // depended on it — they only copy existing .glb assets.
  | { type: "ExportApplication";    payload: { output_dir: string } }
  | { type: "CheckApkPrerequisites" }
  | { type: "ExportApk";            payload: { output_dir: string } }
  | { type: "SaveScene" }
  | { type: "SaveSceneAs"; payload: { path: string } }
  | { type: "DeleteSelection" }
  | { type: "DuplicateSelection" }
  | { type: "CopySelection" }
  | { type: "CutSelection" }
  | { type: "PasteClipboard" }
  | { type: "SelectAll" }
  // --- Tracks: the registry (document-level) ---
  | { type: "CreateTrack"; payload: { name: string } }
  | { type: "DeleteTrack"; payload: { name: string } }
  | { type: "RenameTrack"; payload: { old_name: string; new_name: string } }
  | { type: "SetTrackLooping";  payload: { name: string; looping: boolean } }
  | { type: "SetTrackDuration"; payload: { name: string; duration_secs: number | null } }
  // --- Tracks: asset rows. Refused server-side if the asset already has a
  //     row, so the UI cannot create a duplicate the diagnostics would flag. ---
  | { type: "AddTrackAsset";       payload: { track: string; node_id: number } }
  | { type: "AddTrackElementAsset"; payload: { track: string; panel: number; element: string } }
  | { type: "RemoveTrackAsset";    payload: { track: string; asset_index: number } }
  | { type: "SetTrackAssetTarget"; payload: { track: string; asset_index: number; node_id: number } }
  // --- Tracks: events on a row. Row-addressed, because an event belongs to an
  //     asset row rather than to a flat list. Keys are kept sorted server-side,
  //     so `key_index` means the same thing on both sides. ---
  | { type: "AddTrackKey";    payload: { track: string; asset_index: number; at_secs: number; kind: string } }
  | { type: "RemoveTrackKey"; payload: { track: string; asset_index: number; key_index: number } }
  | { type: "SetTrackKey";    payload: { track: string; asset_index: number; key_index: number; key: XrdsTrackKeyDto } }
  // --- Tracks: editor preview transport. Independent of SetPlayMode by
  //     design — previewing one Track is not running the simulation. ---
  | { type: "PreviewPlayTrack";  payload: { name: string } }
  | { type: "PreviewPauseTrack"; payload: { paused: boolean } }
  | { type: "PreviewStopTrack" }
  // --- Trigger-action: per-node bindings ---
  | { type: "AddTriggerBinding";    payload: { node_id: number } }
  | { type: "RemoveTriggerBinding"; payload: { node_id: number; index: number } }
  | { type: "SetTriggerBindingTrigger";  payload: { node_id: number; index: number; trigger: XrdsTriggerKind } }
  | { type: "SetTriggerBindingHand";     payload: { node_id: number; index: number; hand: string | null } }
  | { type: "SetTriggerBindingDisabled"; payload: { node_id: number; index: number; disabled: boolean } }
  | { type: "SetTriggerBindingEffect"; payload: { node_id: number; index: number; effect: TriggerEffect } }
  | { type: "SetPanelNodeTriggerEffect"; payload: { id: number; element: string; index: number; effect: TriggerEffect } }
  | { type: "SetTriggerBindingTrack";    payload: { node_id: number; index: number; track: string | null } }
  // --- Trigger-action: per-node threshold watchers ---
  | { type: "AddWatcher";    payload: { node_id: number } }
  | { type: "RemoveWatcher"; payload: { node_id: number; index: number } }
  | { type: "SetWatcher";    payload: { node_id: number; index: number; watcher: ThresholdWatcherDto } }
  | { type: "PreviewFireTrigger"; payload: { node_id: number; index: number } };

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export function rgbaToHex(c: [number, number, number, number]): string {
  return "#" + c.slice(0, 3).map(v =>
    Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0")
  ).join("");
}

export function hexToRgba(hex: string, a = 1.0): [number,number,number,number] {
  return [
    parseInt(hex.slice(1, 3), 16) / 255,
    parseInt(hex.slice(3, 5), 16) / 255,
    parseInt(hex.slice(5, 7), 16) / 255,
    a,
  ];
}

export const KIND_ICON: Record<string, string> = {
  Cube: "⬛", Sphere: "⚪", Cylinder: "🥫", Plane: "▭", Tetrahedron: "△",
  Camera: "📷", DirectionalLight: "☀", PointLight: "💡", SpotLight: "🔦",
  AmbientLight: "🌤", GltfAsset: "📦", Text: "T", ExtrudedText: "E³", Billboard: "📋",
  HudText: "HUD", AudioClip: "♪", InteractionZone: "⬡", PlayerSpawn: "🧍", PlayerSpawnZone: "◻",
  Player: "🎮", PlayerAnchor: "📍",
  // `Panel` had no entry at all before this — a placed panel showed no icon in
  // the hierarchy tree. `WorldPanel`'s icon moves here rather than being lost.
  Panel: "🪟",
  Empty: "○",
};
