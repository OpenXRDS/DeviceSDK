import { useCallback, useEffect, useRef, useState } from "react";
import type { EditorCommand, EditorSnapshot, HudItemDefDto } from "../types/bridge";
import { rgbaToHex, hexToRgba } from "../types/bridge";

interface Props {
  templateId: number;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  onClose: () => void;
}

const ASPECT = 16 / 9;

export function HudCanvasOverlay({ templateId, snapshot, send, onClose }: Props) {
  // Paint over the Bevy viewport hole while open
  useEffect(() => {
    (window as any).ipc?.postMessage(JSON.stringify({ type: "set_viewport_hole", enabled: false }));
    return () => {
      (window as any).ipc?.postMessage(JSON.stringify({ type: "set_viewport_hole", enabled: true }));
    };
  }, []);

  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose]);

  const template = snapshot.hud_library.find(t => t.id === templateId) ?? null;

  const [depth, setDepth]               = useState<number>(template?.depth ?? 0.5);
  const [items, setItems]               = useState<HudItemDefDto[]>(template?.items ?? []);
  const [selectedItemId, setSelectedItemId] = useState<number | null>(null);
  const [fovDeg, setFovDeg]             = useState(60);

  // Sync depth from snapshot
  useEffect(() => { if (template) setDepth(template.depth); }, [template?.depth]);

  // Sync items on structural changes (item count change = add/remove)
  const itemCount = template?.items.length ?? 0;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { if (template) setItems(template.items); }, [itemCount]);

  // Canvas sizing: largest 16:9 box that fits the available area
  const outerRef = useRef<HTMLDivElement>(null);
  const [outerSz, setOuterSz] = useState({ w: 900, h: 500 });
  useEffect(() => {
    const el = outerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setOuterSz({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setOuterSz({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);
  const pad = 20;
  const canvasW = Math.min(outerSz.w - pad * 2, (outerSz.h - pad * 2) * ASPECT);
  const canvasH = canvasW / ASPECT;

  // World ↔ canvas pixel conversion
  const fovRad = (fovDeg * Math.PI) / 180;
  const widthM  = 2 * depth * Math.tan(fovRad / 2);
  const heightM = widthM / ASPECT;

  const worldToCanvas = useCallback((wx: number, wy: number) => ({
    x: (wx / widthM + 0.5) * canvasW,
    y: (0.5 - wy / heightM) * canvasH,
  }), [widthM, heightM, canvasW, canvasH]);

  // Per-item drag state
  const dragState = useRef<{
    pointerId: number;
    itemId: number;
    startPx: number; startPy: number;
    startWx: number; startWy: number;
    dragged: boolean;
    lastPos: [number, number] | null;
  } | null>(null);

  const onItemPointerDown = useCallback((e: React.PointerEvent, item: HudItemDefDto) => {
    e.stopPropagation();
    dragState.current = {
      pointerId: e.pointerId,
      itemId: item.id,
      startPx: e.clientX, startPy: e.clientY,
      startWx: item.position[0], startWy: item.position[1],
      dragged: false,
      lastPos: null,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }, []);

  const onItemPointerMove = useCallback((e: React.PointerEvent, item: HudItemDefDto) => {
    const ds = dragState.current;
    if (!ds || ds.itemId !== item.id) return;
    const dpx = e.clientX - ds.startPx;
    const dpy = e.clientY - ds.startPy;
    if (!ds.dragged && Math.abs(dpx) < 4 && Math.abs(dpy) < 4) return;
    ds.dragged = true;
    const newPos: [number, number] = [
      ds.startWx + (dpx / canvasW) * widthM,
      ds.startWy - (dpy / canvasH) * heightM,
    ];
    ds.lastPos = newPos;
    setItems(prev => prev.map(it => it.id === item.id ? { ...it, position: newPos } : it));
  }, [canvasW, canvasH, widthM, heightM, templateId, send]);

  const onItemPointerUp = useCallback((e: React.PointerEvent, item: HudItemDefDto) => {
    const ds = dragState.current;
    if (!ds || ds.itemId !== item.id) return;
    if (ds.dragged && ds.lastPos) {
      send({ type: "SetHudItemPosition", payload: { template_id: templateId, item_id: item.id, position: ds.lastPos } });
    } else {
      setSelectedItemId(prev => prev === item.id ? null : item.id);
    }
    dragState.current = null;
  }, [templateId, send]);

  const selItem = items.find(it => it.id === selectedItemId) ?? null;

  const onDepthChange = useCallback((v: number) => {
    setDepth(v);
    send({ type: "SetHudTemplateDepth", payload: { id: templateId, depth: v } });
  }, [templateId, send]);

  return (
    <div className="hud-canvas-overlay" onClick={() => setSelectedItemId(null)}>

      {/* Header */}
      <div className="hud-canvas-header">
        <div>
          <span className="hud-canvas-title">HUD Template</span>
          <span className="hud-canvas-subtitle">
            {template ? `"${template.name}" · ` : ""}Drag items to reposition. Click to select and edit.
          </span>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          {template && (
            <span style={{ fontSize: 10, color: "var(--overlay0)" }}>
              {items.length} item{items.length !== 1 ? "s" : ""}
            </span>
          )}
          <button
            className="tb-btn"
            style={{ fontSize: 10, padding: "2px 8px" }}
            title="Add a new HUD item to this template"
            onClick={e => { e.stopPropagation(); send({ type: "AddHudItem", payload: { template_id: templateId } }); }}
          >+ Add Item</button>
          <button className="hud-canvas-done" onClick={onClose}>✕ Done</button>
        </div>
      </div>

      {/* Canvas area */}
      <div className="hud-canvas-outer" ref={outerRef}>
        <div className="hud-canvas-inner" style={{ width: canvasW, height: canvasH }}>
          <div className="hud-canvas-fov-label">
            {depth.toFixed(2)} m depth · {widthM.toFixed(2)} × {heightM.toFixed(2)} m at FOV {fovDeg}°
          </div>

          <div className="hud-crosshair-h" />
          <div className="hud-crosshair-v" />

          {!template && (
            <div className="hud-canvas-no-panel">
              Template not found — it may have been deleted
            </div>
          )}

          {template && items.map(item => {
            const cp = worldToCanvas(item.position[0], item.position[1]);
            const isSel = selectedItemId === item.id;
            return (
              <div
                key={item.id}
                className={`hud-item-card${isSel ? " selected" : ""}`}
                style={{ left: cp.x, top: cp.y }}
                onPointerDown={e => onItemPointerDown(e, item)}
                onPointerMove={e => onItemPointerMove(e, item)}
                onPointerUp={e => onItemPointerUp(e, item)}
                onClick={e => e.stopPropagation()}
              >
                <div className="hud-item-card-name">{item.name}</div>
                <div className="hud-item-card-text">{item.text || <em>—</em>}</div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Item editor bar — shown when an item is selected */}
      {selItem ? (
        <div className="hud-slot-editor" onClick={e => e.stopPropagation()}>
          <span className="hud-slot-editor-label">{selItem.name}</span>

          <input
            type="text"
            value={selItem.text}
            placeholder="item text…"
            onKeyDown={e => e.stopPropagation()}
            onChange={e => setItems(prev => prev.map(it => it.id === selItem.id ? { ...it, text: e.target.value } : it))}
            onBlur={e => send({ type: "SetHudItemText", payload: { template_id: templateId, item_id: selItem.id, text: e.target.value } })}
            onKeyUp={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          />

          <label style={{ fontSize: 10, color: "var(--overlay0)", whiteSpace: "nowrap" }}>Font size</label>
          <input
            type="number" min={1} max={20} step={0.5}
            value={selItem.font_size}
            style={{ width: 54 }}
            onKeyDown={e => e.stopPropagation()}
            onChange={e => setItems(prev => prev.map(it => it.id === selItem.id ? { ...it, font_size: +e.target.value } : it))}
            onBlur={e => send({ type: "SetHudItemFontSize", payload: { template_id: templateId, item_id: selItem.id, font_size: +e.target.value } })}
          />

          <label style={{ fontSize: 10, color: "var(--overlay0)" }}>Color</label>
          <input
            type="color"
            value={rgbaToHex(selItem.color)}
            onChange={e => {
              const c = hexToRgba(e.target.value, selItem.color[3]);
              setItems(prev => prev.map(it => it.id === selItem.id ? { ...it, color: c } : it));
              send({ type: "SetHudItemColor", payload: { template_id: templateId, item_id: selItem.id, color: c } });
            }}
          />

          <button
            className="tb-btn"
            style={{ color: "var(--red)", fontSize: 10, marginLeft: 8, whiteSpace: "nowrap" }}
            onClick={() => {
              send({ type: "RemoveHudItem", payload: { template_id: templateId, item_id: selItem.id } });
              setSelectedItemId(null);
            }}
          >✕ Remove item</button>
        </div>
      ) : template && (
        <div className="hud-slot-editor hud-slot-editor--hint">
          Click an item card to select it and edit its text, font size, and color
        </div>
      )}

      {/* Bottom bar: depth + FOV reference */}
      <div className="hud-canvas-bottom">
        <label>Depth</label>
        <input
          type="range" min={0.1} max={2.0} step={0.05}
          value={depth}
          style={{ flex: 1, maxWidth: 200 }}
          onChange={e => onDepthChange(+e.target.value)}
        />
        <span style={{ minWidth: 44 }}>{depth.toFixed(2)} m</span>

        <span style={{ marginLeft: 20, color: "var(--overlay0)" }}>FOV reference</span>
        <input
          type="range" min={30} max={120} step={1}
          value={fovDeg}
          style={{ width: 80 }}
          onChange={e => setFovDeg(+e.target.value)}
        />
        <span style={{ minWidth: 28 }}>{fovDeg}°</span>
      </div>
    </div>
  );
}
