import { useRef, useEffect } from "react";
import type { EditorSnapshot, EditorCommand } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  onOpen: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  onImportAsset: () => void;
  onExportGlb: () => void;
  onExportApp: () => void;
  onShowShortcuts: () => void;
}

export function Menubar({ snapshot, send, onOpen, onSave, onSaveAs, onImportAsset, onExportGlb, onExportApp, onShowShortcuts }: Props) {
  const openMenu = useRef<string | null>(null);
  const rootRef  = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        document.querySelectorAll(".mb-item").forEach(el => el.classList.remove("open"));
        openMenu.current = null;
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  function toggle(id: string) {
    const el = document.getElementById(`mb-${id}`);
    const wasOpen = el?.classList.contains("open");
    document.querySelectorAll(".mb-item").forEach(e => e.classList.remove("open"));
    if (!wasOpen) { el?.classList.add("open"); openMenu.current = id; }
    else openMenu.current = null;
  }
  function close() {
    document.querySelectorAll(".mb-item").forEach(e => e.classList.remove("open"));
    openMenu.current = null;
  }

  function action(fn: () => void) { return () => { close(); fn(); }; }

  return (
    <div className="menubar" ref={rootRef}>
      {/* File */}
      <div className="mb-item" id="mb-file" onClick={() => toggle("file")}>
        File
        <div className="mb-dropdown">
          <div className="mb-action" onClick={action(() => send({ type: "NewScene" }))}>
            New Scene <span className="mb-shortcut">Ctrl+N</span>
          </div>
          <div className="mb-action" onClick={action(onOpen)}>
            Open… <span className="mb-shortcut">Ctrl+O</span>
          </div>
          <div className="mb-sep" />
          <div className="mb-action" onClick={action(onImportAsset)}>
            Import Asset… <span className="mb-shortcut">Ctrl+I</span>
          </div>
          <div className="mb-sep" />
          <div className="mb-action" onClick={action(onSave)}>
            Save <span className="mb-shortcut">Ctrl+S</span>
          </div>
          <div className="mb-action" onClick={action(onSaveAs)}>
            Save As… <span className="mb-shortcut">Ctrl+Shift+S</span>
          </div>
          <div className="mb-sep" />
          <div className="mb-action" onClick={action(onExportGlb)}>
            Export Scene GLB… <span className="mb-shortcut">Ctrl+Shift+E</span>
          </div>
          <div className={`mb-action${snapshot.is_exporting ? " disabled" : ""}`}
               onClick={snapshot.is_exporting ? undefined : action(onExportApp)}>
            {snapshot.is_exporting ? "⏳ Exporting…" : "Export Application…"}
            {!snapshot.is_exporting && <span className="mb-shortcut">Ctrl+Shift+A</span>}
          </div>
        </div>
      </div>

      {/* Edit */}
      <div className="mb-item" id="mb-edit" onClick={() => toggle("edit")}>
        Edit
        <div className="mb-dropdown">
          <div className={`mb-action ${snapshot.undo_count === 0 ? "disabled" : ""}`}
               onClick={action(() => send({ type: "Undo" }))}>
            Undo <span className="mb-shortcut">Ctrl+Z</span>
          </div>
          <div className={`mb-action ${snapshot.redo_count === 0 ? "disabled" : ""}`}
               onClick={action(() => send({ type: "Redo" }))}>
            Redo <span className="mb-shortcut">Ctrl+Y</span>
          </div>
          <div className="mb-sep" />
          <div className={`mb-action ${snapshot.selection.length === 0 ? "disabled" : ""}`}
               onClick={action(() => send({ type: "CopySelection" }))}>
            Copy <span className="mb-shortcut">Ctrl+C</span>
          </div>
          <div className={`mb-action ${snapshot.selection.length === 0 ? "disabled" : ""}`}
               onClick={action(() => send({ type: "CutSelection" }))}>
            Cut <span className="mb-shortcut">Ctrl+X</span>
          </div>
          <div className={`mb-action ${!snapshot.has_clipboard ? "disabled" : ""}`}
               onClick={action(() => send({ type: "PasteClipboard" }))}>
            Paste <span className="mb-shortcut">Ctrl+V</span>
          </div>
          <div className="mb-sep" />
          <div className="mb-action" onClick={action(() => send({ type: "DuplicateSelection" }))}>
            Duplicate <span className="mb-shortcut">Ctrl+D</span>
          </div>
          <div className="mb-action" onClick={action(() => send({ type: "DeleteSelection" }))}>
            Delete <span className="mb-shortcut">Del</span>
          </div>
        </div>
      </div>

      {/* Help */}
      <div className="mb-item" id="mb-help" onClick={() => toggle("help")}>
        Help
        <div className="mb-dropdown">
          <div className="mb-action" onClick={action(onShowShortcuts)}>
            Keyboard Shortcuts…
          </div>
        </div>
      </div>
    </div>
  );
}
