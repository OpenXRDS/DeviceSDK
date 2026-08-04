import { useState } from "react";
import type { EditorCommand, EditorSnapshot } from "../types/bridge";
import { useResizable } from "../hooks/useResizable";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  onEditRunnable: (name: string) => void;
}

/** Small persistent sidebar list — same role as HudLibraryPanel for the HUD
 * library. The actual step/key editing happens in TriggerActionEditorOverlay,
 * opened via "Edit ↗", not here. */
export function TriggerActionLibraryPanel({ snapshot, send, onEditRunnable }: Props) {
  const runnables = snapshot.runnables;
  const [editingName, setEditingName] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  // Handle sits on this panel's own top edge — drag up to grow it.
  const { size: height, dragging, onPointerDown } =
    useResizable({ axis: "y", initial: 150, min: 60, max: 400, invert: true });

  function startRename(name: string) {
    setEditingName(name);
    setEditValue(name);
  }

  function commitRename(oldName: string) {
    const newName = editValue.trim();
    if (newName && newName !== oldName) {
      send({ type: "RenameRunnable", payload: { old_name: oldName, new_name: newName } });
    }
    setEditingName(null);
  }

  function createRunnable(kind: "sequence" | "timeline") {
    const base = kind === "sequence" ? "New Sequence" : "New Timeline";
    let name = base;
    let n = 1;
    while (runnables.some(r => r.name === name)) {
      name = `${base} ${++n}`;
    }
    send({ type: "CreateRunnable", payload: { name, kind } });
  }

  return (
    <div className="hud-library-panel" style={{ height }}>
      <div className={`panel-resize-handle--h${dragging ? " dragging" : ""}`}
        onPointerDown={onPointerDown} title="Drag to resize" />
      <div className="hud-library-header">
        <span className="hud-library-title">Trigger-Action Library</span>
        <div className="flex gap-1">
          <button className="tb-btn text-[10px] px-2 py-0.5"
            title="Create a new ordered-queue runnable" onClick={() => createRunnable("sequence")}>
            + Sequence
          </button>
          <button className="tb-btn text-[10px] px-2 py-0.5"
            title="Create a new absolute-time runnable" onClick={() => createRunnable("timeline")}>
            + Timeline
          </button>
        </div>
      </div>

      {runnables.length === 0 ? (
        <div className="hud-library-empty">
          No runnables yet. Click "+ Sequence" or "+ Timeline" to create one, then reference it
          by name from a node's trigger binding (or an Run action) in the Inspector.
        </div>
      ) : (
        <div className="hud-library-list">
          {runnables.map(r => {
            const count = r.body.type === "Sequence" ? r.body.steps.length : r.body.keys.length;
            const noun = r.body.type === "Sequence" ? "step" : "key";
            return (
              <div key={r.name} className="hud-library-row">
                {editingName === r.name ? (
                  <input
                    className="hud-library-name-input"
                    value={editValue}
                    autoFocus
                    onKeyDown={e => {
                      e.stopPropagation();
                      if (e.key === "Enter") commitRename(r.name);
                      if (e.key === "Escape") setEditingName(null);
                    }}
                    onChange={e => setEditValue(e.target.value)}
                    onBlur={() => commitRename(r.name)}
                  />
                ) : (
                  <span
                    className="hud-library-name"
                    title="Double-click to rename"
                    onDoubleClick={() => startRename(r.name)}
                  >
                    {r.name}
                  </span>
                )}
                <span className="hud-library-meta">
                  {r.body.type} · {count} {noun}{count !== 1 ? "s" : ""}
                  {r.body.type === "Timeline" && r.body.looping ? " · loops" : ""}
                </span>
                <div className="hud-library-actions">
                  <button
                    className="tb-btn text-[10px] py-px px-[7px]"
                    title="Edit this runnable's steps/keys"
                    onClick={() => onEditRunnable(r.name)}
                  >Edit ↗</button>
                  <button
                    className="tb-btn text-[10px] py-px px-[7px] text-red"
                    title="Delete this runnable (bindings/Run steps naming it will fire nothing)"
                    onClick={() => {
                      if (confirm(`Delete runnable "${r.name}"? Anything referencing it by name will fire nothing.`)) {
                        send({ type: "DeleteRunnable", payload: { name: r.name } });
                      }
                    }}
                  >✕</button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {snapshot.runnable_diagnostics.length > 0 && (
        <div className="flex flex-col gap-0.5 px-2 py-1">
          {snapshot.runnable_diagnostics.map((d, i) => (
            <span key={i} className="text-[10px] text-red" title={d.detail}>
              ⚠ {d.title}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
