import { useEffect, useRef, useState } from "react";
import { useEditorState } from "./hooks/useEditorState";
import { useSendCommand } from "./hooks/useSendCommand";
import { useResizable } from "./hooks/useResizable";
import { Menubar } from "./components/Menubar";
import { Toolbar } from "./components/Toolbar";
import { Hierarchy } from "./components/Hierarchy";
import { Palette } from "./components/Palette";
import { Inspector } from "./components/Inspector";
import { PlayerPanel } from "./components/PlayerPanel";
import { ViewportCanvas } from "./components/ViewportCanvas";
import { KeyboardShortcutsModal } from "./components/KeyboardShortcutsModal";
import { HudCanvasOverlay } from "./components/HudCanvasOverlay";
import { WorldPanelCanvasOverlay } from "./components/WorldPanelCanvasOverlay";
import { HudLibraryPanel } from "./components/HudLibraryPanel";
import { ApkExportDialog } from "./components/ApkExportDialog";
import { TriggerActionLibraryPanel } from "./components/TriggerActionLibraryPanel";
import { TriggerActionEditorOverlay } from "./components/TriggerActionEditorOverlay";
import type { EditorCommand, StepTarget } from "./types/bridge";

// ---------------------------------------------------------------------------
// IPC helpers
// ---------------------------------------------------------------------------

let _dialogSeq = 0;

/** Show a native file dialog via wry IPC and return the chosen path (or null). */
function ipcDialog(kind: string): Promise<string | null> {
  return new Promise((resolve) => {
    const id = `d${++_dialogSeq}`;
    window.__xrds__ ??= {};
    window.__xrds__.dialogs ??= {};
    window.__xrds__.dialogs[id] = resolve as (r: string | null) => void;
    (window as any).ipc?.postMessage(JSON.stringify({ type: "file_dialog", id, kind }));
  });
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export default function App() {
  const snapshot        = useEditorState();
  const send            = useSendCommand();
  const centerRef       = useRef<HTMLDivElement>(null);
  const [showShortcuts,  setShowShortcuts]  = useState(false);
  const [hudTemplateId,  setHudTemplateId]  = useState<number | null>(null);
  const [worldPanelId,   setWorldPanelId]   = useState<number | null>(null);
  const [showApkExport,  setShowApkExport]  = useState(false);
  const [editingStepTarget, setEditingStepTarget] = useState<StepTarget | null>(null);

  // Handle sits on the sidebar's right edge — drag right to grow it.
  const sidebar = useResizable({ axis: "x", initial: 240, min: 180, max: 520 });
  // Handle sits on the inspector's left edge — drag left to grow it.
  const inspector = useResizable({ axis: "x", initial: 280, min: 220, max: 560, invert: true });

  // Report exact viewport bounds to Rust whenever the layout changes.
  useEffect(() => {
    const el = centerRef.current;
    if (!el) return;
    const notify = () => {
      const r = el.getBoundingClientRect();
      (window as any).ipc?.postMessage(
        JSON.stringify({ type: "viewport_bounds", x: r.left, y: r.top, w: r.width, h: r.height })
      );
    };
    const ro = new ResizeObserver(notify);
    ro.observe(el);
    notify();
    return () => ro.disconnect();
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (document.activeElement as HTMLElement)?.tagName?.toLowerCase();
      const inInput = tag === "input" || tag === "textarea" || tag === "select";

      if (e.key === "Escape") {
        if (inInput) { (document.activeElement as HTMLElement).blur(); return; }
        if (snapshot.is_playing) send({ type: "SetPlayMode", payload: { playing: false } });
        else send({ type: "DeselectAll" });
        return;
      }

      if (e.ctrlKey) {
        if (e.key === "z") { e.preventDefault(); send({ type: "Undo" }); return; }
        if (e.key === "y") { e.preventDefault(); send({ type: "Redo" }); return; }
        if (e.key === "c" && snapshot.selection.length) { e.preventDefault(); send({ type: "CopySelection" }); return; }
        if (e.key === "x" && snapshot.selection.length) { e.preventDefault(); send({ type: "CutSelection" }); return; }
        if (e.key === "v" && snapshot.has_clipboard)    { e.preventDefault(); send({ type: "PasteClipboard" }); return; }
        if (e.key === "a" && e.shiftKey) { e.preventDefault(); handleExportApp(); return; }
        if (e.key === "e" && e.shiftKey) { e.preventDefault(); handleExportGlb(); return; }
        if (e.key === "s" && e.shiftKey) { e.preventDefault(); handleSaveAs(); return; }
        if (e.key === "s") { e.preventDefault(); handleSave(); return; }
        if (e.key === "i") { e.preventDefault(); handleImportAsset(); return; }
        if (e.key === "o") { e.preventDefault(); handleOpen(); return; }
        if (e.key === "n") { e.preventDefault(); send({ type: "NewScene" }); return; }
      }

      if (inInput) return;

      if (e.key === "t" || e.key === "T") { send({ type: "SetGizmoMode", payload: { mode: "Translate" } }); return; }
      if (e.key === "r" || e.key === "R") { send({ type: "SetGizmoMode", payload: { mode: "Rotate" } }); return; }
      if (e.key === "y" || e.key === "Y") { send({ type: "SetGizmoMode", payload: { mode: "Scale" } }); return; }
      if (e.key === "g" || e.key === "G") { send({ type: "ToggleGrid" }); return; }
      if (e.key === "f" || e.key === "F") { send({ type: "FrameSelected" }); return; }
      if (e.key === "F5") { e.preventDefault(); send({ type: "SetPlayMode", payload: { playing: !snapshot.is_playing } }); return; }
      if (e.key === "Delete" || e.key === "Backspace") {
        if (snapshot.selection.length) send({ type: "DeleteSelection" });
      }
      if (e.ctrlKey && e.key === "d") { e.preventDefault(); send({ type: "DuplicateSelection" }); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [send, snapshot]);

  async function handleImportAsset() {
    const path = await ipcDialog("import_asset");
    if (path) send({ type: "ImportAsset", payload: { path } });
  }
  async function handleExportApp() {
    const dir = await ipcDialog("export_app");
    if (dir) send({ type: "ExportApplication", payload: { output_dir: dir } });
  }
  function handleExportApk() { setShowApkExport(true); }
  async function handleExportGlb() {
    const path = await ipcDialog("export_glb");
    if (path) send({ type: "ExportGlb", payload: { path } });
  }
  async function handleOpen() {
    const path = await ipcDialog("open_scene");
    if (path) send({ type: "OpenScene", payload: { path } });
  }
  function handleSave() {
    send({ type: "SaveScene" });
  }
  async function handleSaveAs() {
    const path = await ipcDialog("save_scene");
    if (path) send({ type: "SaveSceneAs", payload: { path } });
  }

  return (
    <>
      <div className="editor-root">
        <Menubar
          snapshot={snapshot}
          send={send}
          onOpen={handleOpen}
          onImportAsset={handleImportAsset}
          onExportGlb={handleExportGlb}
          onExportApp={handleExportApp}
          onExportApk={handleExportApk}
          onSave={handleSave}
          onSaveAs={handleSaveAs}
          onShowShortcuts={() => setShowShortcuts(true)}
        />
        <Toolbar snapshot={snapshot} send={send} onSaveAs={handleSaveAs} />

        {snapshot.is_exporting && (
          <div className="export-bar">
            <div className="export-spinner" />
            <span>Exporting application… <em style={{opacity:0.7, fontSize:11}}>compiling xrds-app, this may take a minute</em></span>
          </div>
        )}

        {snapshot.status_message && (
          <div className="status-toast">{snapshot.status_message}</div>
        )}

        <div className="editor-panels">
          <div className="left-sidebar" style={{ width: sidebar.size }}>
            <Hierarchy snapshot={snapshot} send={send} />
            <PlayerPanel snapshot={snapshot} send={send} />
            <HudLibraryPanel snapshot={snapshot} send={send} onEditTemplate={id => setHudTemplateId(id)} />
            <TriggerActionLibraryPanel snapshot={snapshot} send={send} onEditRunnable={name => setEditingStepTarget({ type: "Runnable", name })} />
            <button className={`panel-lock-btn${sidebar.locked ? " locked" : ""}`}
              style={{ top: 6, right: 6 }} onClick={sidebar.toggleLock}
              title={sidebar.locked ? "Unlock sidebar width" : "Lock sidebar width"}>
              {sidebar.locked ? "🔒" : "🔓"}
            </button>
            <div className={`panel-resize-handle--v${sidebar.dragging ? " dragging" : ""}${sidebar.locked ? " locked" : ""}`}
              style={{ right: -4 }} onPointerDown={sidebar.onPointerDown} title={sidebar.locked ? "Sidebar width is locked" : "Drag to resize"} />
          </div>
          <div className="editor-center" ref={centerRef}>
            <ViewportCanvas send={send} />
          </div>
          <div className="inspector-wrap" style={{ width: inspector.size }}>
            <button className={`panel-lock-btn${inspector.locked ? " locked" : ""}`}
              style={{ top: 6, left: 6 }} onClick={inspector.toggleLock}
              title={inspector.locked ? "Unlock inspector width" : "Lock inspector width"}>
              {inspector.locked ? "🔒" : "🔓"}
            </button>
            <div className={`panel-resize-handle--v${inspector.dragging ? " dragging" : ""}${inspector.locked ? " locked" : ""}`}
              style={{ left: -4 }} onPointerDown={inspector.onPointerDown} title={inspector.locked ? "Inspector width is locked" : "Drag to resize"} />
            <Inspector snapshot={snapshot} send={send} onEditWorldPanel={id => setWorldPanelId(id)}
              onEditBindingSequence={(nodeId, bindingIndex) => setEditingStepTarget({ type: "Binding", node_id: nodeId, binding_index: bindingIndex })} />
          </div>
        </div>

        <Palette snapshot={snapshot} send={send} />
      </div>

      {showShortcuts && <KeyboardShortcutsModal onClose={() => setShowShortcuts(false)} />}
      {showApkExport && (
        <ApkExportDialog
          snapshot={snapshot}
          send={send}
          onPickFolder={() => ipcDialog("export_app")}
          onClose={() => setShowApkExport(false)}
        />
      )}
      {hudTemplateId !== null && (
        <HudCanvasOverlay
          templateId={hudTemplateId}
          snapshot={snapshot}
          send={send}
          onClose={() => setHudTemplateId(null)}
        />
      )}
      {worldPanelId !== null && (
        <WorldPanelCanvasOverlay
          panelId={worldPanelId}
          snapshot={snapshot}
          send={send}
          onPickAsset={() => ipcDialog("pick_texture")}
          onClose={() => setWorldPanelId(null)}
        />
      )}
      {editingStepTarget !== null && (
        <TriggerActionEditorOverlay
          target={editingStepTarget}
          snapshot={snapshot}
          send={send}
          onClose={() => setEditingStepTarget(null)}
        />
      )}
    </>
  );
}
