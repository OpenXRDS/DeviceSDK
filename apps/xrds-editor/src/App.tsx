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
import { SequencerWorkspace } from "./components/SequencerWorkspace";
import type { SelectedEvent } from "./components/SequencerInspector";
import { SequencerListPanel } from "./components/SequencerListPanel";
import { BridgeMismatchBanner } from "./components/BridgeMismatchBanner";
import type { EditorCommand } from "./types/bridge";

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
  // Which Track the Sequencer has open, by name. There is no longer an
  // inline-sequence alternative to address, so a name is the whole target.
  const [openTrack, setOpenTrack] = useState<string | null>(null);
  // Which workspace layout is active. "sequencer" reflows the whole window
  // after docs/Sequencer_Editor.dc.html — viewport + hierarchy on top, the
  // Sequencer taking roughly half the height below, its own status bar at
  // the base — rather than squeezing a sequencer strip under the normal
  // scene layout. The Bevy viewport stays live in both (its hole just
  // moves; the ResizeObserver below re-reports the new bounds).
  const [workspace, setWorkspace] = useState<"scene" | "sequencer">("scene");
  const seqMode = workspace === "sequencer";
  // Which event the Sequencer's inspector is editing — a row plus a key
  // within it, since events belong to asset rows rather than a flat list.
  // Lives here (not local to the Sequencer) only so it can be reset whenever
  // the open Track changes.
  const [selectedEvent, setSelectedEvent] = useState<SelectedEvent | null>(null);
  useEffect(() => setSelectedEvent(null), [openTrack]);

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
    // `workspace` is a real dependency, not noise: switching layouts can
    // unmount/remount .editor-center, which would leave the observer bound
    // to a detached node and freeze the Bevy viewport hole in place.
  }, [workspace]);

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

  // Opening something to edit also switches into the Sequencer workspace —
  // otherwise clicking a sequencer in the Hierarchy would load it into a
  // panel that isn't on screen.
  function openTrackByName(name: string) {
    setOpenTrack(name);
    setWorkspace("sequencer");
  }

  return (
    <>
      {/* Rendered ahead of the editor: a bridge mismatch means the snapshot
        * shape is untrustworthy, so panels reading it may throw. The banner has
        * to survive that. */}
      <BridgeMismatchBanner snapshot={snapshot} />
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
        <Toolbar snapshot={snapshot} send={send} onSaveAs={handleSaveAs}
          workspace={workspace} onWorkspaceChange={setWorkspace} />

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
          {/* Left column. Scene mode: the scene tree + player/HUD libraries.
            * Sequencer mode: the Sequencers list, since a sequencer isn't a
            * scene node and the tree was the wrong home for it. */}
          <div className="left-sidebar" key="left" style={{ width: sidebar.size }}>
            {seqMode ? (
              <SequencerListPanel snapshot={snapshot} send={send}
                openTrack={openTrack} onOpenTrack={openTrackByName} />
            ) : (
              <>
                <Hierarchy snapshot={snapshot} send={send} onOpenTrack={openTrackByName} />
                <PlayerPanel snapshot={snapshot} send={send} />
                <HudLibraryPanel snapshot={snapshot} send={send} onEditTemplate={id => setHudTemplateId(id)} />
              </>
            )}
            <button className={`panel-lock-btn${sidebar.locked ? " locked" : ""}`}
              style={{ top: 6, right: 6 }} onClick={sidebar.toggleLock}
              title={sidebar.locked ? "Unlock sidebar width" : "Lock sidebar width"}>
              {sidebar.locked ? "🔒" : "🔓"}
            </button>
            <div className={`panel-resize-handle--v${sidebar.dragging ? " dragging" : ""}${sidebar.locked ? " locked" : ""}`}
              style={{ right: -4 }} onPointerDown={sidebar.onPointerDown} title={sidebar.locked ? "Sidebar width is locked" : "Drag to resize"} />
          </div>

          {/* Centre column: viewport, with the Sequencer docked *under it*
            * rather than across the whole window. That's what lets the
            * left and right panels keep the full window height — the node
            * Inspector stops needing to scroll, which was the point. */}
          <div className={`editor-column${seqMode ? " editor-column--seq" : ""}`}>
            <div className="editor-center" key="center" ref={centerRef}>
              <ViewportCanvas send={send} />
            </div>
            {seqMode && (
              <SequencerWorkspace
                track={openTrack}
                snapshot={snapshot}
                send={send}
                selected={selectedEvent}
                onSelectedChange={setSelectedEvent}
              />
            )}
          </div>

          {/* Node Inspector, both modes — still needed in Sequencer mode to
            * bind a trigger to a node. It gets the column's full height
            * there (nothing stacked below it), which is what stops it
            * needing to scroll. Fog/Exposure/IBL are suppressed: they're
            * scene-environment settings, not behaviour authoring. */}
          <div className="inspector-wrap" key="right" style={{ width: inspector.size }}>
            <button className={`panel-lock-btn${inspector.locked ? " locked" : ""}`}
              style={{ top: 6, left: 6 }} onClick={inspector.toggleLock}
              title={inspector.locked ? "Unlock inspector width" : "Lock inspector width"}>
              {inspector.locked ? "🔒" : "🔓"}
            </button>
            <div className={`panel-resize-handle--v${inspector.dragging ? " dragging" : ""}${inspector.locked ? " locked" : ""}`}
              style={{ left: -4 }} onPointerDown={inspector.onPointerDown} title={inspector.locked ? "Inspector width is locked" : "Drag to resize"} />
            <Inspector snapshot={snapshot} send={send} onEditWorldPanel={id => setWorldPanelId(id)}
              onOpenTrack={openTrackByName} showEnvironment={!seqMode} />
          </div>
        </div>

        {!seqMode && <Palette snapshot={snapshot} send={send} />}
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
    </>
  );
}
