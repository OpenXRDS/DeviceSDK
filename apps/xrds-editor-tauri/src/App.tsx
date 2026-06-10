import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEditorState } from "./hooks/useEditorState";
import { useSendCommand } from "./hooks/useSendCommand";
import { Menubar } from "./components/Menubar";
import { Toolbar } from "./components/Toolbar";
import { Hierarchy } from "./components/Hierarchy";
import { Palette } from "./components/Palette";
import { Inspector } from "./components/Inspector";
import { PlayerPanel } from "./components/PlayerPanel";
import type { EditorCommand } from "./types/bridge";

export default function App() {
  const snapshot = useEditorState();
  const send = useSendCommand();
  const statusTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

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
      if (e.key === " ") { e.preventDefault(); send({ type: "SetPlayMode", payload: { playing: !snapshot.is_playing } }); return; }
      if (e.key === "Delete" || e.key === "Backspace") {
        if (snapshot.selection.length) send({ type: "DeleteSelection" });
      }
      if (e.ctrlKey && e.key === "d") { e.preventDefault(); send({ type: "DuplicateSelection" }); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  // Include all snapshot fields used inside the handler so closures are never stale.
  }, [send, snapshot]);

  async function handleImportAsset() {
    const path = await invoke<string | null>("show_import_asset_dialog");
    if (path) send({ type: "ImportAsset", payload: { path } });
  }
  async function handleExportApp() {
    const dir = await invoke<string | null>("show_export_app_dialog");
    if (dir) send({ type: "ExportApplication", payload: { output_dir: dir } });
  }
  async function handleExportGlb() {
    const path = await invoke<string | null>("show_export_glb_dialog", { sceneName: snapshot.scene_name });
    if (path) send({ type: "ExportGlb", payload: { path } });
  }
  async function handleOpen() {
    const path = await invoke<string | null>("show_open_dialog");
    if (path) send({ type: "OpenScene", payload: { path } });
  }
  async function handleSave() {
    send({ type: "SaveScene" });
  }
  async function handleSaveAs() {
    const name = (snapshot.scene_name || "scene") + ".json";
    const path = await invoke<string | null>("show_save_dialog", { currentName: name });
    if (path) send({ type: "SaveSceneAs", payload: { path } });
  }

  return (
    <div className="editor-root">
      <Menubar
        snapshot={snapshot}
        send={send}
        onOpen={handleOpen}
        onImportAsset={handleImportAsset}
        onExportGlb={handleExportGlb}
        onExportApp={handleExportApp}
        onSave={handleSave}
        onSaveAs={handleSaveAs}
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
        <div className="left-sidebar">
          <Hierarchy snapshot={snapshot} send={send} />
          <PlayerPanel snapshot={snapshot} send={send} />
        </div>
        <div className="editor-center">
          <div className="viewport-placeholder">
            ← 3D viewport is in the Bevy window →
          </div>
        </div>
        <Inspector snapshot={snapshot} send={send} />
      </div>

      <Palette snapshot={snapshot} send={send} />
    </div>
  );
}
