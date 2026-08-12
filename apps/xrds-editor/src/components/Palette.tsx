import { useState } from "react";
import type { EditorSnapshot, EditorCommand } from "../types/bridge";
import { KIND_ICON } from "../types/bridge";
import { useResizable } from "../hooks/useResizable";

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
  Capsule:          { label: "Capsule",        tip: "Capsule primitive — a cylinder with hemispherical caps. Useful as a character/physics collider shape." },
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
  HudText:          { label: "HUD Text",       tip: "Head-locked text element. For a full head-locked layout, build a panel template in the Panels workspace and link it on a Player Anchor instead." },
  // Effects
  EffectBurst:      { label: "Burst",          tip: "One-shot particle burst — an impact, explosion or spark hit. Placed idle so a Track can fire it; nothing is drawn until it fires. Tune it in the Inspector." },
  EffectTrail:      { label: "Trail",          tip: "Continuously emitting particles — smoke, a plume, a sparkle trail. Starts running as soon as it is placed." },
  // XR
  Panel:            { label: "Panel",          tip: "Places a panel template in the scene. Its buttons, sliders and toggles can fire Tracks. Build the template in the Panels workspace; the same template can also be head-locked to a Player Anchor." },
};

const PRIMITIVE_GROUPS = [
  { label: "Geometry", items: ["Cube","Sphere","Cylinder","Capsule","Plane","Tetrahedron","Empty"] },
  { label: "Lights",   items: ["PointLight","SpotLight","DirectionalLight","AmbientLight"] },
  { label: "Scene",    items: ["Camera","AudioClip","InteractionZone"] },
  { label: "Player",   items: ["PlayerSpawn","PlayerSpawnZone","Player","PlayerAnchor"] },
  { label: "Text",     items: ["Text","ExtrudedText","Billboard","HudText"] },
  // `WorldPanel` was offered here once, then removed from the palette (its
  // inline widgets carried no triggers, so every button on one was dead), and is
  // now retired from the schema entirely — no tracked scene ever used it.
  // `Panel` is the working replacement.
  // Effects get their own group rather than folding into Geometry: a particle
  // effect is not a mesh shape, and the two kinds differ in behaviour (one-shot
  // vs continuous) rather than in form.
  { label: "Effects",  items: ["EffectBurst","EffectTrail"] },
  { label: "XR",       items: ["Panel"] },
];

const DEFAULT_HEIGHT = 148;
const MIN_HEIGHT = 84;
const MAX_HEIGHT = 440;

export function Palette({ snapshot, send }: Props) {
  const [tab, setTab] = useState<"primitives"|"assets">("primitives");
  const [category, setCategory] = useState<string>(PRIMITIVE_GROUPS[0].label);
  // Handle sits on the palette's own top edge — drag up (away from the
  // bottom of the window) to grow it, same as every other bottom-docked panel.
  const { size: height, dragging, onPointerDown: onResizeStart, locked, toggleLock } =
    useResizable({ axis: "y", initial: DEFAULT_HEIGHT, min: MIN_HEIGHT, max: MAX_HEIGHT, invert: true });
  const parentId = snapshot.selection.length === 1 ? snapshot.selection[0] : null;

  function spawnPrimitive(kind: string) {
    send({ type: "SpawnPrimitive", payload: { kind, parent_id: parentId } });
  }
  function spawnAsset(assetId: string) {
    send({ type: "SpawnAsset", payload: { asset_id: assetId, parent_id: parentId } });
  }

  const activeGroup = PRIMITIVE_GROUPS.find(g => g.label === category) ?? PRIMITIVE_GROUPS[0];

  return (
    <div className="palette" style={{ height }}>
      <button className={`panel-lock-btn${locked ? " locked" : ""}`}
        style={{ top: 6, right: 6 }} onClick={toggleLock}
        title={locked ? "Unlock palette height" : "Lock palette height"}>
        {locked ? "🔒" : "🔓"}
      </button>
      <div className={`panel-resize-handle--h${dragging ? " dragging" : ""}${locked ? " locked" : ""}`}
        onPointerDown={onResizeStart} title={locked ? "Palette height is locked" : "Drag to resize"} />
      <div className="pal-tabs">
        <div className={`pal-tab${tab === "primitives" ? " active" : ""}`} onClick={() => setTab("primitives")}>Primitives</div>
        <div className={`pal-tab${tab === "assets" ? " active" : ""}`} onClick={() => setTab("assets")}>Project Assets</div>
      </div>
      {tab === "primitives" && (
        <div className="pal-cat-tabs">
          {PRIMITIVE_GROUPS.map(group => (
            <div key={group.label}
              className={`pal-cat-tab${category === group.label ? " active" : ""}`}
              onClick={() => setCategory(group.label)}
            >
              {group.label}
            </div>
          ))}
        </div>
      )}
      <div className="pal-content">
        {tab === "primitives" && activeGroup.items.map(kind => {
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
        })}
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
