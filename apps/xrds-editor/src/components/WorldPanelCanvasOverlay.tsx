import { useCallback, useEffect, useRef, useState } from "react";
import type { EditorCommand, EditorSnapshot, WorldWidget, WorldLayout, RGBA } from "../types/bridge";
import { rgbaToHex, hexToRgba } from "../types/bridge";

interface Props {
  panelId: number;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  /** Native file dialog for picking an image asset; resolves to a path or null. */
  onPickAsset: () => Promise<string | null>;
  onClose: () => void;
}

const WIDGET_KINDS = ["Label", "Button", "Image", "Slider", "Toggle"] as const;
const WIDGET_ICONS: Record<string, string> = {
  Label: "🏷", Button: "🔘", Image: "🖼", Slider: "🎚", Toggle: "⏻",
};

function cssRgba(c: RGBA): string {
  return `rgba(${Math.round(c[0] * 255)},${Math.round(c[1] * 255)},${Math.round(c[2] * 255)},${c[3]})`;
}

/** Layout slot size [w,h] metres — mirrors widget_layout_size in world_ui_layout.rs. */
function slotSize(w: WorldWidget): [number, number] {
  switch (w.type) {
    case "Label":  return w.layout_size;
    case "Slider": return [w.size[0], w.thumb_size * 1.5];
    default:       return w.size;
  }
}

/** Mirrors compute_positions in world_ui_layout.rs. Returns per-widget [x,y] metres. */
function layoutPositions(layout: WorldLayout, widgets: WorldWidget[]): [number, number][] | null {
  if (layout.type === "None") return null;
  const sizes = widgets.map(slotSize);
  const n = widgets.length;
  if (layout.type === "VStack") {
    const totalH = sizes.reduce((a, s) => a + s[1], 0) + layout.gap * Math.max(0, n - 1);
    let y = totalH * 0.5;
    return sizes.map(s => {
      y -= s[1] * 0.5;
      const p: [number, number] = [0, y];
      y -= s[1] * 0.5 + layout.gap;
      return p;
    });
  }
  if (layout.type === "HStack") {
    const totalW = sizes.reduce((a, s) => a + s[0], 0) + layout.gap * Math.max(0, n - 1);
    let x = -totalW * 0.5;
    return sizes.map(s => {
      x += s[0] * 0.5;
      const p: [number, number] = [x, 0];
      x += s[0] * 0.5 + layout.gap;
      return p;
    });
  }
  // Grid
  const cols = Math.max(1, layout.cols);
  const rows = Math.ceil(n / cols);
  const colW = new Array(cols).fill(0);
  const rowH = new Array(rows).fill(0);
  sizes.forEach((s, i) => {
    colW[i % cols] = Math.max(colW[i % cols], s[0]);
    rowH[Math.floor(i / cols)] = Math.max(rowH[Math.floor(i / cols)], s[1]);
  });
  const totalW = colW.reduce((a, w) => a + w, 0) + layout.gap[0] * (cols - 1);
  const totalH = rowH.reduce((a, h) => a + h, 0) + layout.gap[1] * (rows - 1);
  const colStart: number[] = []; let cx = -totalW * 0.5;
  for (let c = 0; c < cols; c++) { colStart.push(cx); cx += colW[c] + layout.gap[0]; }
  const rowStart: number[] = []; let ry = totalH * 0.5;
  for (let r = 0; r < rows; r++) { rowStart.push(ry); ry -= rowH[r] + layout.gap[1]; }
  return sizes.map((_, i) => {
    const c = i % cols, r = Math.floor(i / cols);
    return [colStart[c] + colW[c] * 0.5, rowStart[r] - rowH[r] * 0.5] as [number, number];
  });
}

export function WorldPanelCanvasOverlay({ panelId, snapshot, send, onPickAsset, onClose }: Props) {
  // Paint over the Bevy viewport hole while open
  useEffect(() => {
    (window as any).ipc?.postMessage(JSON.stringify({ type: "set_viewport_hole", enabled: false }));
    return () => {
      (window as any).ipc?.postMessage(JSON.stringify({ type: "set_viewport_hole", enabled: true }));
    };
  }, []);

  // Panel data comes from the selected-node inspector DTO (the panel is selected
  // when the overlay opens; selection can't change while the overlay covers the UI).
  const node = snapshot.selected_node;
  const panel = node && node.id === panelId && node.payload.type === "WorldPanel" ? node.payload : null;
  const panelName = node && node.id === panelId ? node.name : "";

  // Snapshot of the panel state at open time — restored by Cancel. Edits apply live
  // to the document (so the 3-D viewport previews them); Save just keeps them.
  const initial = useRef<{
    size: [number, number]; color: RGBA; corner_radius: number; opacity: number;
    layout: WorldLayout; widgets: WorldWidget[];
  } | null>(null);
  useEffect(() => {
    if (panel && !initial.current) {
      initial.current = JSON.parse(JSON.stringify({
        size: panel.size, color: panel.color, corner_radius: panel.corner_radius,
        opacity: panel.opacity, layout: panel.layout ?? { type: "None" },
        widgets: panel.widgets ?? [],
      }));
    }
  }, [panel]);

  const cancel = useCallback(() => {
    const s = initial.current;
    if (s) {
      send({ type: "SetWorldPanelParams", payload: {
        id: panelId, size: s.size, color: s.color,
        corner_radius: s.corner_radius, opacity: s.opacity } });
      send({ type: "SetWorldPanelLayout",  payload: { id: panelId, layout: s.layout } });
      send({ type: "SetWorldPanelWidgets", payload: { id: panelId, widgets: s.widgets } });
    }
    onClose();
  }, [panelId, send, onClose]);

  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") cancel(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [cancel]);

  const [widgets, setWidgets] = useState<WorldWidget[]>(panel?.widgets ?? []);
  const [selected, setSelected] = useState<number | null>(null);
  const dragging = useRef(false);

  // Sync widgets from the snapshot except mid-drag. JSON key avoids the
  // new-array-reference-every-frame problem.
  const widgetsKey = JSON.stringify(panel?.widgets ?? []);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { if (!dragging.current && panel) setWidgets(panel.widgets); }, [widgetsKey]);

  // Selecting a different widget must never leave snapshot sync blocked by a
  // half-finished edit on the previous one.
  useEffect(() => { dragging.current = false; }, [selected]);

  const layout: WorldLayout = panel?.layout ?? { type: "None" };
  const layoutActive = layout.type !== "None";
  const panelW = panel?.size[0] ?? 0.4;
  const panelH = panel?.size[1] ?? 0.3;

  // Canvas sizing: largest box with the panel's aspect that fits the available area
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
  const pad = 30;
  const aspect = panelW / panelH;
  const canvasW = Math.min(outerSz.w - pad * 2, (outerSz.h - pad * 2) * aspect);
  const canvasH = canvasW / aspect;
  const pxPerM = canvasW / panelW;

  // Panel-local metres (centre origin, Y up) → canvas pixels
  const toPx = useCallback((wx: number, wy: number) => ({
    x: (wx / panelW + 0.5) * canvasW,
    y: (0.5 - wy / panelH) * canvasH,
  }), [panelW, panelH, canvasW, canvasH]);

  // Layout preview positions (metres), or null for manual positioning
  const autoPositions = layoutActive ? layoutPositions(layout, widgets) : null;
  const widgetPos = (w: WorldWidget, i: number): [number, number] =>
    autoPositions?.[i] ?? w.local_position;

  // ── Drag state ─────────────────────────────────────────────────────────────
  const dragRef = useRef<{
    index: number;
    startPx: number; startPy: number;
    startWx: number; startWy: number;
    moved: boolean;
    last: [number, number] | null;
  } | null>(null);

  const commitWidget = useCallback((index: number, widget: WorldWidget) => {
    send({ type: "SetWorldPanelWidget", payload: { id: panelId, index, widget } });
  }, [panelId, send]);

  const onPointerDown = useCallback((e: React.PointerEvent, i: number) => {
    e.stopPropagation();
    const w = widgets[i];
    dragRef.current = {
      index: i,
      startPx: e.clientX, startPy: e.clientY,
      startWx: w.local_position[0], startWy: w.local_position[1],
      moved: false, last: null,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }, [widgets]);

  const onPointerMove = useCallback((e: React.PointerEvent, i: number) => {
    const ds = dragRef.current;
    if (!ds || ds.index !== i || layoutActive) return;
    const dpx = e.clientX - ds.startPx;
    const dpy = e.clientY - ds.startPy;
    if (!ds.moved && Math.abs(dpx) < 4 && Math.abs(dpy) < 4) return;
    ds.moved = true;
    dragging.current = true;
    const pos: [number, number] = [
      ds.startWx + dpx / pxPerM,
      ds.startWy - dpy / pxPerM,
    ];
    ds.last = pos;
    setWidgets(prev => prev.map((w, j) => j === i ? { ...w, local_position: pos } as WorldWidget : w));
  }, [pxPerM, layoutActive]);

  const onPointerUp = useCallback((_e: React.PointerEvent, i: number) => {
    const ds = dragRef.current;
    if (!ds || ds.index !== i) return;
    if (ds.moved && ds.last) {
      dragging.current = false;
      commitWidget(i, { ...widgets[i], local_position: ds.last } as WorldWidget);
    } else {
      setSelected(prev => prev === i ? null : i);
    }
    dragRef.current = null;
  }, [widgets, commitWidget]);

  // ── Corner-resize state ─────────────────────────────────────────────────────
  // Dragging the SE handle resizes the widget about its centre, so the size delta
  // is 2× the cursor delta (the opposite corner mirrors the movement).
  const resizeRef = useRef<{
    index: number;
    startPx: number; startPy: number;
    startSize: [number, number];
    startFont: number;           // Label: font scales with height
    last: WorldWidget | null;
  } | null>(null);

  const visualSize = (w: WorldWidget): [number, number] =>
    w.type === "Label" ? w.layout_size : w.size;

  const onResizeDown = useCallback((e: React.PointerEvent, i: number) => {
    e.stopPropagation();
    const w = widgets[i];
    resizeRef.current = {
      index: i,
      startPx: e.clientX, startPy: e.clientY,
      startSize: [...visualSize(w)] as [number, number],
      startFont: w.type === "Label" || w.type === "Button" ? w.font_size : 0,
      last: null,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }, [widgets]);

  const onResizeMove = useCallback((e: React.PointerEvent, i: number) => {
    const rs = resizeRef.current;
    if (!rs || rs.index !== i) return;
    dragging.current = true;
    const nw = Math.max(0.01, rs.startSize[0] + ((e.clientX - rs.startPx) / pxPerM) * 2);
    const nh = Math.max(0.01, rs.startSize[1] + ((e.clientY - rs.startPy) / pxPerM) * 2);
    const w = widgets[i];
    let updated: WorldWidget;
    if (w.type === "Label") {
      // Text has no explicit box — scale the font with the drag height.
      const scale = rs.startSize[1] > 0 ? nh / rs.startSize[1] : 1;
      updated = { ...w, layout_size: [nw, nh], font_size: Math.max(0.005, rs.startFont * scale) };
    } else {
      updated = { ...w, size: [nw, nh] } as WorldWidget;
    }
    rs.last = updated;
    setWidgets(prev => prev.map((pw, j) => j === i ? updated : pw));
  }, [pxPerM, widgets]);

  const onResizeUp = useCallback((_e: React.PointerEvent, i: number) => {
    const rs = resizeRef.current;
    if (!rs || rs.index !== i) return;
    if (rs.last) {
      dragging.current = false;
      commitWidget(i, rs.last);
    }
    resizeRef.current = null;
  }, [commitWidget]);

  // ── Selected-widget editing helpers ────────────────────────────────────────
  const sel = selected !== null ? widgets[selected] : null;
  // Live edit: update local state only and block snapshot sync (a commit round-trips
  // through a full reimport, so the echo would reset the field mid-typing).
  const liveSel = (patch: object) => {
    if (selected === null) return;
    dragging.current = true;
    setWidgets(prev => prev.map((w, j) => j === selected ? { ...w, ...patch } as WorldWidget : w));
  };
  // Commit: push the current local widget to the document (on blur / discrete change).
  const commitSel = (patch: object = {}) => {
    if (selected === null) return;
    dragging.current = false;
    const nw = { ...widgets[selected], ...patch } as WorldWidget;
    setWidgets(prev => prev.map((w, j) => j === selected ? nw : w));
    commitWidget(selected, nw);
  };

  // ── Widget visuals (WYSIWYG-ish) ───────────────────────────────────────────
  const resizeHandle = (i: number) => (
    <div title="Drag to resize"
      style={{ position: "absolute", right: -7, bottom: -7, width: 13, height: 13,
        background: "var(--blue)", border: "1px solid #fff", borderRadius: 3,
        cursor: "nwse-resize", touchAction: "none", zIndex: 2 }}
      onPointerDown={e => onResizeDown(e, i)}
      onPointerMove={e => onResizeMove(e, i)}
      onPointerUp={e => onResizeUp(e, i)} />
  );

  function renderWidget(w: WorldWidget, i: number) {
    const pos = widgetPos(w, i);
    const cp = toPx(pos[0], pos[1]);
    const isSel = selected === i;
    const base: React.CSSProperties = {
      position: "absolute", left: cp.x, top: cp.y,
      transform: "translate(-50%, -50%)",
      cursor: layoutActive ? "pointer" : "grab",
      touchAction: "none", userSelect: "none",
      outline: isSel ? "2px solid var(--blue)" : "1px dashed rgba(137,180,250,.35)",
      outlineOffset: 2,
      boxSizing: "border-box",
    };
    const handlers = {
      onPointerDown: (e: React.PointerEvent) => onPointerDown(e, i),
      onPointerMove: (e: React.PointerEvent) => onPointerMove(e, i),
      onPointerUp:   (e: React.PointerEvent) => onPointerUp(e, i),
      onClick:       (e: React.MouseEvent)   => e.stopPropagation(),
    };

    switch (w.type) {
      case "Label":
        return (
          <div key={i} style={{ ...base, color: cssRgba(w.color), fontSize: Math.max(8, w.font_size * pxPerM), whiteSpace: "nowrap" }} {...handlers}>
            {w.text || "(empty label)"}
            {isSel && resizeHandle(i)}
          </div>
        );
      case "Button":
        return (
          <div key={i} style={{
            ...base, width: w.size[0] * pxPerM, height: w.size[1] * pxPerM,
            background: cssRgba(w.normal_color), borderRadius: 4,
            display: "flex", alignItems: "center", justifyContent: "center",
            color: cssRgba(w.label_color), fontSize: Math.max(8, w.font_size * pxPerM), whiteSpace: "nowrap", overflow: "visible",
          }} {...handlers}>
            {w.label || "Button"}
            {isSel && resizeHandle(i)}
          </div>
        );
      case "Image":
        return (
          <div key={i} style={{
            ...base, width: w.size[0] * pxPerM, height: w.size[1] * pxPerM,
            background: cssRgba([w.tint[0] * 0.5, w.tint[1] * 0.5, w.tint[2] * 0.5, Math.max(0.3, w.tint[3])]),
            display: "flex", alignItems: "center", justifyContent: "center",
            fontSize: Math.min(w.size[0], w.size[1]) * pxPerM * 0.5,
          }} {...handlers}>
            🖼
            {isSel && resizeHandle(i)}
          </div>
        );
      case "Slider": {
        const frac = w.max > w.min ? (w.value - w.min) / (w.max - w.min) : 0;
        const trackW = w.size[0] * pxPerM, trackH = Math.max(2, w.size[1] * pxPerM);
        const thumb = Math.max(6, w.thumb_size * pxPerM);
        return (
          <div key={i} style={{ ...base, width: trackW, height: Math.max(trackH, thumb) }} {...handlers}>
            <div style={{ position: "absolute", top: "50%", left: 0, width: trackW, height: trackH, transform: "translateY(-50%)", background: cssRgba(w.track_color), borderRadius: trackH / 2 }} />
            <div style={{ position: "absolute", top: "50%", left: 0, width: trackW * frac, height: trackH, transform: "translateY(-50%)", background: cssRgba(w.fill_color), borderRadius: trackH / 2 }} />
            <div style={{ position: "absolute", top: "50%", left: trackW * frac, width: thumb, height: thumb, transform: "translate(-50%, -50%)", background: cssRgba(w.thumb_color), borderRadius: "50%" }} />
            {isSel && resizeHandle(i)}
          </div>
        );
      }
      case "Toggle": {
        const tw = w.size[0] * pxPerM, th = w.size[1] * pxPerM;
        return (
          <div key={i} style={{
            ...base, width: tw, height: th, borderRadius: th / 2,
            background: cssRgba(w.checked ? w.track_on_color : w.track_off_color),
          }} {...handlers}>
            <div style={{
              position: "absolute", top: "50%", left: w.checked ? tw - th * 0.5 : th * 0.5,
              width: th * 0.8, height: th * 0.8, transform: "translate(-50%, -50%)",
              background: cssRgba(w.thumb_color), borderRadius: "50%",
            }} />
            {isSel && resizeHandle(i)}
          </div>
        );
      }
    }
  }

  // ── Selected-widget editor bar ─────────────────────────────────────────────
  const numStyle = { width: 84, fontSize: 12, padding: "3px 5px" } as const;
  const numField = (label: string, key: string, step = 0.01) => (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 3 }}>
      <label style={{ fontSize: 10, color: "var(--overlay0)", whiteSpace: "nowrap" }}>{label}</label>
      <input type="number" step={step} value={(sel as any)?.[key] ?? 0} style={numStyle}
        onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
        onChange={e => liveSel({ [key]: +e.target.value })}
        onBlur={() => commitSel()} />
    </span>
  );
  const vec2Field = (label: string, key: string) => {
    const v = ((sel as any)?.[key] ?? [0, 0]) as [number, number];
    return (
      <span style={{ display: "inline-flex", alignItems: "center", gap: 3 }}>
        <label style={{ fontSize: 10, color: "var(--overlay0)", whiteSpace: "nowrap" }}>{label}</label>
        <input type="number" step={0.01} value={v[0]} style={numStyle}
          onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          onChange={e => liveSel({ [key]: [+e.target.value, v[1]] })}
          onBlur={() => commitSel()} />
        <input type="number" step={0.01} value={v[1]} style={numStyle}
          onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          onChange={e => liveSel({ [key]: [v[0], +e.target.value] })}
          onBlur={() => commitSel()} />
      </span>
    );
  };
  const colorField = (label: string, key: string) => {
    const c = ((sel as any)?.[key] ?? [1, 1, 1, 1]) as RGBA;
    return (
      <span style={{ display: "inline-flex", alignItems: "center", gap: 3 }}>
        <label style={{ fontSize: 10, color: "var(--overlay0)", whiteSpace: "nowrap" }}>{label}</label>
        <input type="color" value={rgbaToHex(c)}
          onChange={e => liveSel({ [key]: hexToRgba(e.target.value, c[3]) })}
          onBlur={() => commitSel()} />
      </span>
    );
  };
  const textField = (label: string, key: string) => (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 3, flex: 1, minWidth: 120 }}>
      <label style={{ fontSize: 10, color: "var(--overlay0)", whiteSpace: "nowrap" }}>{label}</label>
      <input type="text" value={(sel as any)?.[key] ?? ""} style={{ flex: 1 }}
        onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
        onChange={e => liveSel({ [key]: e.target.value })}
        onBlur={() => commitSel()} />
    </span>
  );

  // ── Layout controls (bottom bar) ───────────────────────────────────────────
  const setLayout = (l: WorldLayout) =>
    send({ type: "SetWorldPanelLayout", payload: { id: panelId, layout: l } });
  const layoutGap = layout.type === "VStack" || layout.type === "HStack" ? layout.gap : 0.01;

  return (
    <div className="hud-canvas-overlay"
      style={{ background: "rgba(0,0,0,.6)", padding: 22, boxSizing: "border-box" }}
      onClick={() => setSelected(null)}>
      {/* Window frame filling the editor almost fully */}
      <div style={{
        flex: 1, minHeight: 0, display: "flex", flexDirection: "column",
        background: "var(--base, #11111b)", border: "1px solid var(--surface1)",
        borderRadius: 8, overflow: "hidden", boxShadow: "0 12px 48px rgba(0,0,0,.7)",
      }}>

      {/* Title bar */}
      <div className="hud-canvas-header">
        <div>
          <span className="hud-canvas-title">World Panel Editor</span>
          <span className="hud-canvas-subtitle">
            {panelName ? `"${panelName}" · ` : ""}
            {layoutActive
              ? "Positions are managed by the panel layout — click a widget to edit it."
              : "Drag widgets to move · drag the corner handle to resize · click to edit."}
          </span>
        </div>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          {WIDGET_KINDS.map(kind => (
            <button key={kind} className="tb-btn" style={{ fontSize: 10, padding: "2px 7px" }}
              title={`Add ${kind} widget`}
              onClick={e => { e.stopPropagation(); send({ type: "AddWorldPanelWidget", payload: { id: panelId, kind } }); }}>
              + {WIDGET_ICONS[kind]} {kind}
            </button>
          ))}
          <button className="hud-canvas-done"
            style={{ background: "var(--blue)", color: "var(--base, #11111b)", fontWeight: "bold" }}
            title="Keep all changes and close"
            onClick={onClose}>✓ Save & Close</button>
          <button className="hud-canvas-done"
            title="Discard all changes made in this editor and close (Esc)"
            onClick={cancel}>✕ Cancel</button>
        </div>
      </div>

      {/* Canvas area */}
      <div className="hud-canvas-outer" ref={outerRef}>
        <div className="hud-canvas-inner"
          style={{ width: canvasW, height: canvasH, background: panel ? cssRgba(panel.color) : "#07071a" }}>
          <div className="hud-canvas-fov-label">
            {panelW.toFixed(2)} × {panelH.toFixed(2)} m · {widgets.length} widget{widgets.length !== 1 ? "s" : ""}
            {layoutActive ? ` · layout: ${layout.type}` : ""}
          </div>

          <div className="hud-crosshair-h" />
          <div className="hud-crosshair-v" />

          {!panel && (
            <div className="hud-canvas-no-panel">
              Panel not selected — close this editor and select the World Panel node
            </div>
          )}

          {panel && widgets.map((w, i) => renderWidget(w, i))}
        </div>
      </div>

      {/* Widget editor bar */}
      {sel ? (
        <div className="hud-slot-editor" style={{ flexWrap: "wrap" }} onClick={e => e.stopPropagation()}>
          <span className="hud-slot-editor-label">{WIDGET_ICONS[sel.type]} {sel.type}</span>

          {sel.type === "Label" && <>
            {textField("Text", "text")}
            {numField("Font (m)", "font_size", 0.005)}
            {colorField("Color", "color")}
            {vec2Field("Slot (m)", "layout_size")}
          </>}
          {sel.type === "Button" && <>
            {textField("Label", "label")}
            {numField("Font (m)", "font_size", 0.005)}
            {vec2Field("Size (m)", "size")}
            {colorField("Normal", "normal_color")}
            {colorField("Hover", "hover_color")}
            {colorField("Pressed", "pressed_color")}
            {colorField("Text", "label_color")}
          </>}
          {sel.type === "Image" && <>
            {textField("Asset path", "asset_path")}
            <button className="tb-btn" style={{ fontSize: 10, padding: "2px 8px", whiteSpace: "nowrap" }}
              title="Browse for an image file"
              onClick={async () => {
                const path = await onPickAsset();
                if (path) commitSel({ asset_path: path });
              }}>Browse…</button>
            {vec2Field("Size (m)", "size")}
            {colorField("Tint", "tint")}
          </>}
          {sel.type === "Slider" && <>
            {numField("Min", "min")}
            {numField("Max", "max")}
            {numField("Value", "value")}
            {vec2Field("Track (m)", "size")}
            {numField("Thumb (m)", "thumb_size", 0.001)}
            {colorField("Track", "track_color")}
            {colorField("Fill", "fill_color")}
            {colorField("Thumb", "thumb_color")}
          </>}
          {sel.type === "Toggle" && <>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 3 }}>
              <label style={{ fontSize: 10, color: "var(--overlay0)" }}>Checked</label>
              <input type="checkbox" checked={sel.checked}
                style={{ accentColor: "var(--green)", cursor: "pointer", width: 15, height: 15 }}
                onChange={e => commitSel({ checked: e.target.checked })} />
            </span>
            {vec2Field("Size (m)", "size")}
            {colorField("Off", "track_off_color")}
            {colorField("On", "track_on_color")}
            {colorField("Thumb", "thumb_color")}
          </>}

          <span style={{ marginLeft: "auto", display: "inline-flex", gap: 4, alignItems: "center" }}>
            <label style={{ fontSize: 10, color: "var(--overlay0)" }} title="Order in the widget list — determines layout position">Order</label>
            <button className="tb-btn" style={{ padding: "0 6px", fontSize: 10 }} disabled={selected === 0}
              title="Move earlier in the list"
              onClick={() => {
                send({ type: "MoveWorldPanelWidget", payload: { id: panelId, index: selected!, delta: -1 } });
                setSelected(selected! - 1);
              }}>▲</button>
            <button className="tb-btn" style={{ padding: "0 6px", fontSize: 10 }} disabled={selected === widgets.length - 1}
              title="Move later in the list"
              onClick={() => {
                send({ type: "MoveWorldPanelWidget", payload: { id: panelId, index: selected!, delta: 1 } });
                setSelected(selected! + 1);
              }}>▼</button>
            <button className="tb-btn" style={{ color: "var(--red)", fontSize: 10, whiteSpace: "nowrap" }}
              onClick={() => {
                send({ type: "RemoveWorldPanelWidget", payload: { id: panelId, index: selected! } });
                setSelected(null);
              }}>✕ Remove widget</button>
          </span>
        </div>
      ) : panel && (
        <div className="hud-slot-editor hud-slot-editor--hint">
          Click a widget to select it and edit its properties
        </div>
      )}

      {/* Bottom bar: panel size + background + layout */}
      {panel && (
        <div className="hud-canvas-bottom" onClick={e => e.stopPropagation()}>
          <label>W (m)</label>
          <input type="range" min={0.05} max={5} step={0.01} value={panelW} style={{ width: 100 }}
            onChange={e => send({ type: "SetWorldPanelParams", payload: {
              id: panelId, size: [+e.target.value, panelH], color: panel.color,
              corner_radius: panel.corner_radius, opacity: panel.opacity } })} />
          <span style={{ minWidth: 36 }}>{panelW.toFixed(2)}</span>

          <label>H (m)</label>
          <input type="range" min={0.05} max={5} step={0.01} value={panelH} style={{ width: 100 }}
            onChange={e => send({ type: "SetWorldPanelParams", payload: {
              id: panelId, size: [panelW, +e.target.value], color: panel.color,
              corner_radius: panel.corner_radius, opacity: panel.opacity } })} />
          <span style={{ minWidth: 36 }}>{panelH.toFixed(2)}</span>

          <label style={{ marginLeft: 12 }}>Background</label>
          <input type="color" value={rgbaToHex(panel.color)}
            onChange={e => send({ type: "SetWorldPanelParams", payload: {
              id: panelId, size: [panelW, panelH], color: hexToRgba(e.target.value, panel.color[3]),
              corner_radius: panel.corner_radius, opacity: panel.opacity } })} />
          <label>Alpha</label>
          <input type="range" min={0} max={1} step={0.01} value={panel.color[3]} style={{ width: 70 }}
            onChange={e => send({ type: "SetWorldPanelParams", payload: {
              id: panelId, size: [panelW, panelH],
              color: [panel.color[0], panel.color[1], panel.color[2], +e.target.value],
              corner_radius: panel.corner_radius, opacity: panel.opacity } })} />

          <label style={{ marginLeft: 12 }}>Layout</label>
          <select value={layout.type} onChange={e => {
            const k = e.target.value;
            if (k === "VStack")      setLayout({ type: "VStack", gap: layoutGap });
            else if (k === "HStack") setLayout({ type: "HStack", gap: layoutGap });
            else if (k === "Grid")   setLayout({ type: "Grid", cols: layout.type === "Grid" ? layout.cols : 2, gap: layout.type === "Grid" ? layout.gap : [0.01, 0.01] });
            else                     setLayout({ type: "None" });
          }}>
            {["None", "VStack", "HStack", "Grid"].map(k => <option key={k} value={k}>{k}</option>)}
          </select>
          {(layout.type === "VStack" || layout.type === "HStack") && <>
            <label>Gap</label>
            <input type="number" step={0.005} min={0} value={layoutGap} style={{ width: 62 }}
              onKeyDown={e => e.stopPropagation()}
              onChange={e => setLayout({ type: layout.type, gap: +e.target.value } as WorldLayout)} />
          </>}
          {layout.type === "Grid" && <>
            <label>Cols</label>
            <input type="number" step={1} min={1} max={8} value={layout.cols} style={{ width: 48 }}
              onKeyDown={e => e.stopPropagation()}
              onChange={e => setLayout({ type: "Grid", cols: Math.max(1, Math.round(+e.target.value)), gap: layout.gap })} />
            <label>Gap</label>
            <input type="number" step={0.005} min={0} value={layout.gap[0]} style={{ width: 62 }}
              onKeyDown={e => e.stopPropagation()}
              onChange={e => setLayout({ type: "Grid", cols: layout.cols, gap: [+e.target.value, layout.gap[1]] })} />
            <input type="number" step={0.005} min={0} value={layout.gap[1]} style={{ width: 62 }}
              onKeyDown={e => e.stopPropagation()}
              onChange={e => setLayout({ type: "Grid", cols: layout.cols, gap: [layout.gap[0], +e.target.value] })} />
          </>}
        </div>
      )}
      </div>
    </div>
  );
}
