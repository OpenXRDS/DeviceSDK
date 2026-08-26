import { useState } from "react";
import type { EditorCommand, EditorSnapshot, NamedTrackDto } from "../types/bridge";
import { conflictingTracks, fmtTime } from "../lib/sequencer";
import { ConfirmDialog } from "./ui/ConfirmDialog";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  /** Which Track the Sequencer workspace has open, so this list can mark it. */
  openTrack: string | null;
  onOpenTrack: (name: string) => void;
}

/** Left-hand list of every Track in the document.
 *
 * One flat list, no tabs. The two-tab split (Timelines / Action Chains) existed
 * when there were two execution models; there is now one, so a tab that always
 * held everything and a tab that was always empty would be pure noise. */
export function SequencerListPanel({ snapshot, send, openTrack, onOpenTrack }: Props) {
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  const tracks = snapshot.tracks;

  function create() {
    let name = "New Track";
    let n = 1;
    while (tracks.some(t => t.name === name)) name = `New Track ${++n}`;
    send({ type: "CreateTrack", payload: { name } });
    onOpenTrack(name);
  }

  function commitRename(oldName: string) {
    const newName = renameValue.trim();
    if (newName && newName !== oldName) {
      send({ type: "RenameTrack", payload: { old_name: oldName, new_name: newName } });
    }
    setRenaming(null);
  }

  /** Diagnostics naming this Track. Matches on the exact-quoted name Rust's
   * `Debug` for `String` produces (`Track "name"`), so two similarly-named
   * Tracks cannot collide — not a fragile substring guess. */
  function diagCount(name: string): number {
    const needle = `Track ${JSON.stringify(name)}`;
    return snapshot.track_diagnostics.filter(d => d.detail.includes(needle)).length;
  }

  function summary(t: NamedTrackDto): string {
    const events = t.assets.reduce((n, a) => n + a.keys.length, 0);
    const parts = [
      `${t.assets.length} asset${t.assets.length === 1 ? "" : "s"}`,
      `${events} event${events === 1 ? "" : "s"}`,
    ];
    if (t.effective_duration_secs > 0) parts.push(fmtTime(t.effective_duration_secs));
    if (t.looping) parts.push("loops");
    return parts.join(" · ");
  }

  return (
    <div className="seq-list-panel">
      <div className="panel-header">Tracks</div>

      <div className="seq-list-toolbar">
        <button className="tb-btn text-[10.5px] px-2 py-0.5"
          title="A Track is choreography over a set of assets, fired by a trigger."
          onClick={create}>
          + Track
        </button>
      </div>

      {tracks.length === 0 ? (
        <div className="seq-list-empty">
          No Tracks yet.
          <div className="mt-1.5 text-overlay0">
            A Track holds one row per asset, with events pinned to times. A trigger fires the
            whole thing.
          </div>
        </div>
      ) : (
        <div className="seq-list-items">
          {tracks.map(t => {
            const diags = diagCount(t.name);
            const conflicts = conflictingTracks(t.name, tracks);
            const isOpen = openTrack === t.name;
            return (
              <div key={t.name} className={`seq-list-row${isOpen ? " open" : ""}`}
                title={isOpen ? "Open in the Sequencer below" : `Open "${t.name}"`}
                onClick={() => onOpenTrack(t.name)}>
                <span className="seq-list-row-icon">{t.looping ? "↻" : "▶"}</span>
                {renaming === t.name ? (
                  <input autoFocus className="tree-rename" value={renameValue}
                    onClick={e => e.stopPropagation()}
                    onChange={e => setRenameValue(e.target.value)}
                    onBlur={() => commitRename(t.name)}
                    onKeyDown={e => {
                      e.stopPropagation();
                      if (e.key === "Enter") commitRename(t.name);
                      if (e.key === "Escape") setRenaming(null);
                    }} />
                ) : (
                  <span className="seq-list-row-name" title="Double-click to rename"
                    onDoubleClick={e => {
                      e.stopPropagation();
                      setRenaming(t.name);
                      setRenameValue(t.name);
                    }}>
                    {t.name}
                  </span>
                )}
                <span className="seq-list-row-meta">{summary(t)}</span>
                {conflicts.length > 0 && (
                  <span className="text-peach text-[10px]"
                    title={`Shares an asset with ${conflicts.join(", ")} — they cannot run at the same time.`}>
                    ⇄
                  </span>
                )}
                {diags > 0 && (
                  <span className="text-yellow text-[10px]"
                    title={`${diags} diagnostic${diags === 1 ? "" : "s"}`}>⚠</span>
                )}
                <button className="seq-list-row-del"
                  title={`Delete "${t.name}". Bindings that fire it will be cleared.`}
                  onClick={e => {
                    e.stopPropagation();
                    setPendingDelete(t.name);
                  }}>✕</button>
              </div>
            );
          })}
        </div>
      )}

      {/* Registry-level diagnostics, which belong to no single Track row. */}
      {pendingDelete !== null && (
        <ConfirmDialog
          message={`Delete Track "${pendingDelete}"?`}
          detail="Bindings that fire it will be cleared."
          onConfirm={() => {
            send({ type: "DeleteTrack", payload: { name: pendingDelete } });
            setPendingDelete(null);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}

      {snapshot.track_diagnostics.length > 0 && (
        <div className="flex flex-col gap-1 px-2.5 py-2 border-t border-surface0">
          {snapshot.track_diagnostics.slice(0, 6).map((d, i) => (
            <span key={i}
              className={`text-[10px] ${d.severity === "error" ? "text-red" : d.severity === "warning" ? "text-yellow" : "text-overlay0"}`}
              title={d.detail}>
              ⚠ {d.title}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
