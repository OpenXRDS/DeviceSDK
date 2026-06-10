import { useState } from "react";
import type { EditorSnapshot, EditorCommand } from "../types/bridge";
import { KIND_ICON } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}

const PRIMITIVE_GROUPS = [
  { label: "Geometry", items: ["Cube","Sphere","Cylinder","Plane","Tetrahedron","Empty"] },
  { label: "Lights",   items: ["PointLight","SpotLight","DirectionalLight","AmbientLight"] },
  { label: "Scene",    items: ["Camera","AudioClip","InteractionZone","PlayerSpawn","Player","PlayerAnchor"] },
  { label: "Text",     items: ["Text","ExtrudedText","Billboard","HudText"] },
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
        {tab === "primitives" && PRIMITIVE_GROUPS.flatMap(group =>
          group.items.map(kind => (
            <button key={kind} className="pal-btn" onClick={() => spawnPrimitive(kind)}>
              {KIND_ICON[kind] ?? "○"} {kind}
            </button>
          ))
        )}
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
