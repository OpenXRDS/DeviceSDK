import { useState } from "react";
import type { EditorSnapshot, EditorCommand } from "../types/bridge";
import { KIND_ICON } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}

interface ItemMeta { label: string; tip: string; }

const PALETTE_META: Record<string, ItemMeta> = {
  // Geometry
  Cube:             { label: "Cube",           tip: "Axis-aligned box primitive." },
  Sphere:           { label: "Sphere",         tip: "UV sphere primitive." },
  Cylinder:         { label: "Cylinder",       tip: "Cylinder primitive." },
  Plane:            { label: "Plane",          tip: "Flat plane primitive." },
  Tetrahedron:      { label: "Tetrahedron",    tip: "Four-sided solid." },
  Empty:            { label: "Empty",          tip: "Invisible transform-only node. Use as a group parent." },
  // Lights
  PointLight:       { label: "Point Light",    tip: "Omni-directional light emitting from a point." },
  SpotLight:        { label: "Spot Light",     tip: "Cone-shaped directional light." },
  DirectionalLight: { label: "Sun Light",      tip: "Parallel-ray light (like the sun). Position is ignored; only rotation matters." },
  AmbientLight:     { label: "Ambient Light",  tip: "Scene-wide base illumination with no direction." },
  // Scene
  Camera:           { label: "Camera",         tip: "Free scene camera, not player-bound." },
  AudioClip:        { label: "Audio Clip",     tip: "Positional audio source placed in the scene." },
  InteractionZone:  { label: "Interaction Zone", tip: "Trigger volume that fires events when a player enters or exits." },
  // Player
  PlayerSpawn:      { label: "Spawn Point",    tip: "World-space marker defining where a player enters the scene. Standalone — no parent node required." },
  PlayerSpawnZone:  { label: "Spawn Zone",     tip: "Rectangular volume where players are randomly placed on load. Width × Height × Depth in metres." },
  Player:           { label: "Player",         tip: "Player root entity. Add Camera Anchors as children to define viewpoints and HUD placement." },
  PlayerAnchor:     { label: "Camera Anchor",  tip: "A camera viewpoint owned by one Player. Must be a child of a Player node. Each anchor can link to a HUD template." },
  // Text
  Text:             { label: "Text",           tip: "Flat billboard text rendered in the scene." },
  ExtrudedText:     { label: "Extruded Text",  tip: "3-D extruded text mesh." },
  Billboard:        { label: "Billboard",      tip: "Sprite-like quad that always faces the camera." },
  HudText:          { label: "HUD Text",       tip: "Head-locked text element. Use the HUD Library panel to manage HUD layouts instead of placing these individually." },
  // XR
  WorldPanel:       { label: "World Panel",    tip: "Flat interactive UI panel anchored at a world-space position. Add buttons, labels, sliders, and toggles as child widgets." },
};

const PRIMITIVE_GROUPS = [
  { label: "Geometry", items: ["Cube","Sphere","Cylinder","Plane","Tetrahedron","Empty"] },
  { label: "Lights",   items: ["PointLight","SpotLight","DirectionalLight","AmbientLight"] },
  { label: "Scene",    items: ["Camera","AudioClip","InteractionZone"] },
  { label: "Player",   items: ["PlayerSpawn","PlayerSpawnZone","Player","PlayerAnchor"] },
  { label: "Text",     items: ["Text","ExtrudedText","Billboard","HudText"] },
  { label: "XR",       items: ["WorldPanel"] },
];

export function Palette({ snapshot, send }: Props) {
  const [tab, setTab] = useState<"primitives"|"assets">("primitives");
  const parentId = snapshot.selection.length === 1 ? snapshot.selection[0] : null;

  function spawnPrimitive(kind: string) {
    send({ type: "SpawnPrimitive", payload: { kind, parent_id: parentId } });
  }
  function spawnAsset(assetId: string) {
    send({ type: "SpawnAsset", payload: { asset_id: assetId, parent_id: parentId } });
  }

  return (
    <div className="palette">
      <div className="pal-tabs">
        <div className={`pal-tab${tab === "primitives" ? " active" : ""}`} onClick={() => setTab("primitives")}>Primitives</div>
        <div className={`pal-tab${tab === "assets" ? " active" : ""}`} onClick={() => setTab("assets")}>Project Assets</div>
      </div>
      <div className="pal-content">
        {tab === "primitives" && PRIMITIVE_GROUPS.flatMap(group => [
          <div key={`__hdr_${group.label}`} className="pal-group-header">{group.label}</div>,
          ...group.items.map(kind => {
            const meta = PALETTE_META[kind];
            return (
              <button
                key={kind}
                className="pal-btn"
                title={meta?.tip}
                onClick={() => spawnPrimitive(kind)}
              >
                {KIND_ICON[kind] ?? "○"} {meta?.label ?? kind}
              </button>
            );
          }),
        ])}
        {tab === "assets" && (
          snapshot.asset_catalog.length === 0
            ? <span className="pal-empty">No assets imported yet.  Use File → Import Asset…</span>
            : snapshot.asset_catalog.map(asset => (
                <div key={asset.id} className="asset-row"
                     style={{ display:"flex", alignItems:"center", gap:6, width:"100%" }}
                     onClick={() => spawnAsset(asset.id)}>
                  <span style={{ fontSize:11, flexShrink:0 }}>
                    {asset.kind === "Gltf" ? "📦" : asset.kind === "Texture" ? "🖼" : asset.kind === "Audio" ? "♪" : "🌄"}
                  </span>
                  <span style={{ flex: 1 }}>{asset.name}</span>
                  <span className="asset-kind">{asset.kind}</span>
                  <button
                    className="tb-btn"
                    style={{ padding: "1px 6px", fontSize: 10, flexShrink: 0 }}
                    title="Remove asset from project"
                    onClick={e => {
                      e.stopPropagation();
                      if (confirm(`Remove "${asset.id}" and all scene nodes that use it?`)) {
                        send({ type: "RemoveAsset", payload: { asset_id: asset.id } });
                      }
                    }}
                  >✕</button>
                </div>
              ))
        )}
      </div>
    </div>
  );
}
