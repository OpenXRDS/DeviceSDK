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

export interface HudItemDefDto {
  id: number;
  name: string;
  position: [number, number];
  text: string;
  font_size: number;
  color: [number, number, number, number];
}

export interface HudTemplateDto {
  id: number;
  name: string;
  depth: number;
  items: HudItemDefDto[];
}

export interface MaterialParams {
  base_color: [number, number, number, number];
  metallic: number;
  roughness: number;
  emissive: [number, number, number];
}

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
  | { type: "PlayerAnchor"; fov_deg: number; is_initial: boolean; hud_template_id: number | null; exposure: number | null }
  | { type: "PlayerSpawnZone"; size: [number, number, number]; player_node_id: number | null }
  | { type: "Other";        kind: string };

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

export interface EditorSnapshot {
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
  hud_library: HudTemplateDto[];
  stereo_preview_active: boolean;
}

export interface EnvironmentDto {
  fog_enabled: boolean;  fog_color: [number,number,number,number]; fog_start: number; fog_end: number;
  exposure_enabled: boolean; ev100: number;
  ibl_enabled: boolean; ibl_diffuse: string; ibl_specular: string; ibl_intensity: number;
  skybox_enabled: boolean; skybox_asset: string; skybox_brightness: number;
}

export const defaultSnapshot: EditorSnapshot = {
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
  hud_library: [],
  stereo_preview_active: false,
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
  // HUD library commands
  | { type: "CreateHudTemplate";    payload: { name: string } }
  | { type: "DeleteHudTemplate";    payload: { id: number } }
  | { type: "RenameHudTemplate";    payload: { id: number; name: string } }
  | { type: "SetHudTemplateDepth";  payload: { id: number; depth: number } }
  | { type: "AddHudItem";           payload: { template_id: number } }
  | { type: "RemoveHudItem";        payload: { template_id: number; item_id: number } }
  | { type: "RenameHudItem";        payload: { template_id: number; item_id: number; name: string } }
  | { type: "SetHudItemPosition";   payload: { template_id: number; item_id: number; position: [number,number] } }
  | { type: "SetHudItemText";       payload: { template_id: number; item_id: number; text: string } }
  | { type: "SetHudItemFontSize";   payload: { template_id: number; item_id: number; font_size: number } }
  | { type: "SetHudItemColor";      payload: { template_id: number; item_id: number; color: [number,number,number,number] } }
  | { type: "LinkHudTemplate";      payload: { anchor_id: number; template_id: number | null } }
  | { type: "SetCameraParams";    payload: { id: number; fov: number; near: number; far: number } }
  | { type: "CommitCameraParams"; payload: { id: number; fov: number; near: number; far: number } }
  | { type: "SetPlayerAnchorFov";      payload: { id: number; fov_deg: number } }
  | { type: "SetPlayerAnchorInitial";  payload: { id: number; is_initial: boolean } }
  | { type: "SetPlayerAnchorExposure"; payload: { id: number; ev100: number | null } }
  | { type: "SetSpawnZoneSize";        payload: { id: number; size: [number, number, number] } }
  | { type: "SetSpawnZonePlayer";      payload: { id: number; player_node_id: number | null } }
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
  | { type: "ExportGlb";            payload: { path: string } }
  | { type: "ExportApplication";    payload: { output_dir: string } }
  | { type: "SaveScene" }
  | { type: "SaveSceneAs"; payload: { path: string } }
  | { type: "DeleteSelection" }
  | { type: "DuplicateSelection" }
  | { type: "CopySelection" }
  | { type: "CutSelection" }
  | { type: "PasteClipboard" }
  | { type: "SelectAll" };

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
  Empty: "○",
};
