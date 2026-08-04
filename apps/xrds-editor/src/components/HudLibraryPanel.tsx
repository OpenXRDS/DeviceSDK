import { useState } from "react";
import type { EditorCommand, EditorSnapshot } from "../types/bridge";
import { useResizable } from "../hooks/useResizable";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  onEditTemplate: (id: number) => void;
}

export function HudLibraryPanel({ snapshot, send, onEditTemplate }: Props) {
  const templates = snapshot.hud_library;
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName]   = useState("");
  // Handle sits on this panel's own top edge — drag up to grow it.
  const { size: height, dragging, onPointerDown } =
    useResizable({ axis: "y", initial: 150, min: 60, max: 400, invert: true });

  function startRename(id: number, currentName: string) {
    setEditingId(id);
    setEditName(currentName);
  }

  function commitRename(id: number) {
    if (editName.trim()) {
      send({ type: "RenameHudTemplate", payload: { id, name: editName.trim() } });
    }
    setEditingId(null);
  }

  function createTemplate() {
    send({ type: "CreateHudTemplate", payload: { name: "New Template" } });
  }

  return (
    <div className="hud-library-panel" style={{ height }}>
      <div className={`panel-resize-handle--h${dragging ? " dragging" : ""}`}
        onPointerDown={onPointerDown} title="Drag to resize" />
      <div className="hud-library-header">
        <span className="hud-library-title">HUD Library</span>
        <button className="tb-btn" style={{ fontSize: 10, padding: "2px 8px" }} onClick={createTemplate}>
          + New
        </button>
      </div>

      {templates.length === 0 ? (
        <div className="hud-library-empty">
          No HUD templates yet. Click "+ New" to create one, then link it to a PlayerAnchor in the Inspector.
        </div>
      ) : (
        <div className="hud-library-list">
          {templates.map(t => (
            <div key={t.id} className="hud-library-row">
              {editingId === t.id ? (
                <input
                  className="hud-library-name-input"
                  value={editName}
                  autoFocus
                  onKeyDown={e => {
                    e.stopPropagation();
                    if (e.key === "Enter") commitRename(t.id);
                    if (e.key === "Escape") setEditingId(null);
                  }}
                  onChange={e => setEditName(e.target.value)}
                  onBlur={() => commitRename(t.id)}
                />
              ) : (
                <span
                  className="hud-library-name"
                  title="Double-click to rename"
                  onDoubleClick={() => startRename(t.id, t.name)}
                >
                  {t.name}
                </span>
              )}
              <span className="hud-library-meta">
                {t.items.length} item{t.items.length !== 1 ? "s" : ""} · {t.depth.toFixed(2)} m
              </span>
              <div className="hud-library-actions">
                <button
                  className="tb-btn"
                  style={{ fontSize: 10, padding: "1px 7px" }}
                  title="Edit this template in the canvas view"
                  onClick={() => onEditTemplate(t.id)}
                >Edit ↗</button>
                <button
                  className="tb-btn"
                  style={{ fontSize: 10, padding: "1px 7px", color: "var(--red)" }}
                  title="Delete this template (unlinks all anchors)"
                  onClick={() => {
                    if (confirm(`Delete template "${t.name}"? All PlayerAnchor links will be cleared.`)) {
                      send({ type: "DeleteHudTemplate", payload: { id: t.id } });
                    }
                  }}
                >✕</button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
