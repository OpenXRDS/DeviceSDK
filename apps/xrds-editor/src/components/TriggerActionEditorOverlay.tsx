import { useEffect, useState } from "react";
import type {
  ActionTarget, ActionValue, EditorCommand, EditorSnapshot, StepTarget, XrdsAction,
  XrdsTimelineKeyDto,
} from "../types/bridge";
import { Select } from "./ui/Select";
import { Checkbox } from "./ui/Checkbox";

interface Props {
  /** Either a registry runnable's body, or one node's inline binding
   * sequence — see Stage 3 in docs/xrds-trigger-action-editor-plan.md. A
   * binding's inline sequence is always a Sequence, never a Timeline. */
  target: StepTarget;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  onClose: () => void;
}

const ACTION_KINDS = [
  "SetVisible", "Teleport", "Wait", "ModifyHealth", "FireCustomEvent", "Run",
  "PlayGltfAnimation", "StopGltfAnimation",
] as const;
const ACTION_ICONS: Record<string, string> = {
  SetVisible: "👁", Teleport: "🚀", Wait: "⏱", ModifyHealth: "❤",
  FireCustomEvent: "📣", Run: "▶", PlayGltfAnimation: "🎞", StopGltfAnimation: "⏹",
};

function summarizeAction(a: XrdsAction): string {
  switch (a.kind) {
    case "SetVisible": return `SetVisible(${a.data})`;
    case "Teleport": return `Teleport → (${a.data.destination.map(v => v.toFixed(1)).join(", ")})`;
    case "Wait": return `Wait ${a.data.seconds}s`;
    case "ModifyHealth": return `ModifyHealth`;
    case "FireCustomEvent": return `FireCustomEvent("${a.data.name}")`;
    case "Run": return `Run("${a.data.runnable}"${a.data.wait ? "" : ", fire-and-forget"})`;
    case "PlayGltfAnimation": return `PlayGltfAnimation(clip ${a.data.clip_index})`;
    case "StopGltfAnimation": return "StopGltfAnimation";
    case "Unknown": return "Unknown (from a newer editor)";
  }
}

// Radix Select.Item forbids an empty-string value, but `value === ""` is this
// codebase's existing "nothing picked" convention (see RunnablePicker below
// and Inspector.tsx's trigger-kind picker) — map it to/from this sentinel at
// the Select boundary instead of touching that convention everywhere.
const NONE_SENTINEL = "__none__";

/** Same shape every editor overlay in this codebase uses for a "pick a
 * foreign name" field with a dangling-reference warning — see
 * PlayerAnchorSection's HUD-template picker in Inspector.tsx. */
function RunnablePicker({ value, runnableNames, onChange }: {
  value: string; runnableNames: string[]; onChange: (v: string) => void;
}) {
  const dangling = value !== "" && !runnableNames.includes(value);
  return (
    <span className="inline-flex flex-col gap-0.5">
      <span className="inline-flex items-center gap-[3px]">
        <label className="text-[10px] text-overlay0">Runnable</label>
        <Select
          value={value === "" ? NONE_SENTINEL : value}
          onValueChange={v => onChange(v === NONE_SENTINEL ? "" : v)}
          options={[
            { value: NONE_SENTINEL, label: "— none —" },
            ...runnableNames.map(n => ({ value: n, label: n })),
          ]}
        />
      </span>
      {dangling && (
        <span className="text-[10px] text-red">⚠ "{value}" has no matching runnable</span>
      )}
    </span>
  );
}

export function TriggerActionEditorOverlay({ target, snapshot, send, onClose }: Props) {
  // Paint over the Bevy viewport hole while open — same trick every other
  // full-viewport overlay in this codebase uses.
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

  const targetKey = JSON.stringify(target);
  const runnable = target.type === "Runnable" ? snapshot.runnables.find(r => r.name === target.name) ?? null : null;
  // A binding's inline sequence comes from the selected-node inspector DTO —
  // same assumption WorldPanelCanvasOverlay makes about its panel: selection
  // can't change while this overlay covers the UI.
  const binding = target.type === "Binding" && snapshot.selected_node?.id === target.node_id
    ? snapshot.selected_node.triggers[target.binding_index] ?? null
    : null;
  const found = target.type === "Runnable" ? runnable !== null : binding !== null;
  const isTimeline = runnable?.body.type === "Timeline";
  const runnableNames = snapshot.runnables.map(r => r.name);
  // null for a Binding target — a binding isn't itself a named runnable, so
  // there's no "self" name to exclude from the Run picker or warn about.
  const ownRunnableName = target.type === "Runnable" ? target.name : null;

  const [selected, setSelected] = useState<number | null>(null);
  useEffect(() => { setSelected(null); }, [targetKey]);

  // Rename (Runnable targets only — a binding has no name of its own).
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  function commitRename() {
    if (target.type === "Runnable") {
      const newName = renameValue.trim();
      if (newName && newName !== target.name) {
        send({ type: "RenameRunnable", payload: { old_name: target.name, new_name: newName } });
      }
    }
    setRenaming(false);
  }

  // Local mirror for live-editing the selected step/key, committed on blur —
  // same liveSel/commitSel split WorldPanelCanvasOverlay uses, minus the
  // drag machinery (nothing here is spatial).
  const steps: XrdsAction[] =
    target.type === "Binding" ? (binding?.sequence.steps ?? []) :
    runnable?.body.type === "Sequence" ? runnable.body.steps : [];
  const keys: XrdsTimelineKeyDto[] = runnable?.body.type === "Timeline" ? runnable.body.keys : [];

  const [draftAction, setDraftAction] = useState<XrdsAction | null>(null);
  const [draftAtSecs, setDraftAtSecs] = useState(0);
  // `AddActionStep`/`AddTimelineKey` round-trip through Rust before `steps`/
  // `keys` actually grows, so selecting the just-added index at click time
  // still sees the OLD (shorter) array for a frame. Depending on this JSON
  // key (not just `selected`) re-syncs once the real data lands, instead of
  // only ever seeing the stale snapshot from the moment of selection.
  const selectedItemKey = JSON.stringify(
    selected === null ? null : (isTimeline ? keys[selected] : steps[selected]) ?? null
  );
  useEffect(() => {
    if (selected === null) { setDraftAction(null); return; }
    if (isTimeline) {
      const k = keys[selected];
      setDraftAction(k ? k.action : null);
      setDraftAtSecs(k ? k.at_secs : 0);
    } else {
      setDraftAction(steps[selected] ?? null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected, targetKey, selectedItemKey]);

  // Only a Runnable target can ever be a Timeline (bindings only hold an
  // inline Sequence), so these three helpers only need the registry name
  // in the isTimeline branch — safe to assume target.type === "Runnable" there.
  function commitDraft(action: XrdsAction, atSecs: number) {
    if (selected === null) return;
    if (isTimeline && target.type === "Runnable") {
      send({ type: "SetTimelineKey", payload: { name: target.name, index: selected, key: { at_secs: atSecs, action } } });
    } else {
      send({ type: "SetActionStep", payload: { target, index: selected, action } });
    }
  }

  function addStep(kind: string) {
    if (isTimeline && target.type === "Runnable") {
      send({ type: "AddTimelineKey", payload: { name: target.name, at_secs: 0, kind } });
    } else {
      send({ type: "AddActionStep", payload: { target, kind } });
    }
    setSelected(isTimeline ? keys.length : steps.length);
  }

  function removeSelected() {
    if (selected === null) return;
    if (isTimeline && target.type === "Runnable") {
      send({ type: "RemoveTimelineKey", payload: { name: target.name, index: selected } });
    } else {
      send({ type: "RemoveActionStep", payload: { target, index: selected } });
    }
    setSelected(null);
  }

  function moveSelected(delta: number) {
    if (selected === null || isTimeline) return; // timeline order comes from at_secs, not array position
    send({ type: "MoveActionStep", payload: { target, index: selected, delta } });
    setSelected(selected + delta);
  }

  const numCls = "w-[84px] text-[12px] text-text px-2 py-1 rounded bg-surface0 border border-transparent focus:outline focus:outline-1 focus:outline-blue font-mono";

  return (
    <div className="hud-canvas-overlay bg-black/60 p-[22px] box-border"
      onClick={() => setSelected(null)}>
      <div className="flex-1 min-h-0 flex flex-col bg-base border border-surface1 rounded-lg overflow-hidden shadow-[0_12px_48px_rgba(0,0,0,.7)]">

        {/* Title bar */}
        <div className="hud-canvas-header">
          <div>
            <span className="hud-canvas-title">Trigger-Action Editor</span>
            <span className="hud-canvas-subtitle">
              {target.type === "Runnable" ? (
                runnable ? (
                  <>
                    {renaming ? (
                      <input
                        autoFocus
                        value={renameValue}
                        className="text-[11px] text-text bg-surface0 rounded px-1.5 py-0.5 border border-transparent focus:outline focus:outline-1 focus:outline-blue"
                        onClick={e => e.stopPropagation()}
                        onKeyDown={e => {
                          e.stopPropagation();
                          if (e.key === "Enter") commitRename();
                          if (e.key === "Escape") setRenaming(false);
                        }}
                        onChange={e => setRenameValue(e.target.value)}
                        onBlur={commitRename}
                      />
                    ) : (
                      <span
                        title="Double-click to rename"
                        className="cursor-text"
                        onClick={e => e.stopPropagation()}
                        onDoubleClick={() => { setRenameValue(runnable.name); setRenaming(true); }}
                      >
                        "{runnable.name}"
                      </span>
                    )}
                    {` · ${runnable.body.type}`}
                  </>
                ) : "Runnable not found — it may have been deleted"
              ) : (
                binding ? `"${snapshot.selected_node?.name}" binding #${target.binding_index} · inline sequence`
                        : "Binding not found — it may have been removed, or the selection changed"
              )}
            </span>
          </div>
          <div className="flex gap-1.5 items-center">
            {ACTION_KINDS.map(kind => (
              <button key={kind} className="tb-btn text-[10px] px-[7px] py-0.5"
                title={`Add a ${kind} ${isTimeline ? "key" : "step"}`}
                onClick={e => { e.stopPropagation(); addStep(kind); }}>
                + {ACTION_ICONS[kind]} {kind}
              </button>
            ))}
            <button className="hud-canvas-done" onClick={onClose}>✕ Done</button>
          </div>
        </div>

        {/* Step/key list */}
        <div className="flex-1 min-h-0 overflow-y-auto px-3 py-2" onClick={e => e.stopPropagation()}>
          {!found && (
            <div className="hud-canvas-no-panel">
              {target.type === "Runnable" ? "Runnable not found" : "Binding not found"} — close this editor
            </div>
          )}
          {found && (isTimeline ? keys.length === 0 : steps.length === 0) && (
            <div className="hud-library-empty">
              No {isTimeline ? "keys" : "steps"} yet. Use the buttons above to add one.
            </div>
          )}
          {found && (isTimeline ? keys : steps).map((item, i) => {
            const action = isTimeline ? (item as XrdsTimelineKeyDto).action : (item as XrdsAction);
            const atSecs = isTimeline ? (item as XrdsTimelineKeyDto).at_secs : null;
            const isSel = selected === i;
            return (
              <div key={i}
                className={`hud-library-row cursor-pointer ${isSel ? "outline outline-2 outline-blue" : "outline-none"}`}
                onClick={() => setSelected(i)}
              >
                <span className="hud-library-name">
                  {atSecs !== null && <span className="text-overlay0 mr-2">t={atSecs.toFixed(2)}s</span>}
                  {ACTION_ICONS[action.kind] ?? "?"} {summarizeAction(action)}
                </span>
              </div>
            );
          })}
        </div>

        {/* Selected step/key editor bar */}
        {draftAction && selected !== null ? (
          <div className="hud-slot-editor flex-wrap" onClick={e => e.stopPropagation()}>
            <span className="hud-slot-editor-label">{ACTION_ICONS[draftAction.kind]} {draftAction.kind}</span>

            {isTimeline && (
              <span className="inline-flex items-center gap-[3px]">
                <label className="text-[10px] text-overlay0">at_secs</label>
                <input type="number" step={0.05} min={0} value={draftAtSecs} className={numCls}
                  onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                  onChange={e => setDraftAtSecs(+e.target.value)}
                  onBlur={() => commitDraft(draftAction, draftAtSecs)} />
              </span>
            )}

            {draftAction.kind === "SetVisible" && (
              <Checkbox
                label="Visible"
                checked={draftAction.data}
                onCheckedChange={v => {
                  const next: XrdsAction = { kind: "SetVisible", data: v };
                  setDraftAction(next);
                  commitDraft(next, draftAtSecs);
                }}
              />
            )}

            {draftAction.kind === "Teleport" && (
              <span className="inline-flex items-center gap-[3px]">
                <label className="text-[10px] text-overlay0">Destination</label>
                {[0, 1, 2].map(axis => (
                  <input key={axis} type="number" step={0.1} value={draftAction.data.destination[axis]} className={numCls}
                    onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                    onChange={e => {
                      const dest = [...draftAction.data.destination] as [number, number, number];
                      dest[axis] = +e.target.value;
                      setDraftAction({ kind: "Teleport", data: { destination: dest } });
                    }}
                    onBlur={() => commitDraft(draftAction, draftAtSecs)} />
                ))}
              </span>
            )}

            {draftAction.kind === "Wait" && (
              <span className="inline-flex items-center gap-[3px]">
                <label className="text-[10px] text-overlay0">Seconds</label>
                <input type="number" step={0.05} min={0} value={draftAction.data.seconds} className={numCls}
                  onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                  onChange={e => setDraftAction({ kind: "Wait", data: { seconds: +e.target.value } })}
                  onBlur={() => commitDraft(draftAction, draftAtSecs)} />
                {isTimeline && (
                  <span className="text-[10px] text-red">⚠ meaningless in a timeline — skipped at runtime</span>
                )}
              </span>
            )}

            {draftAction.kind === "ModifyHealth" && (
              <>
                <span className="inline-flex items-center gap-[3px]">
                  <label className="text-[10px] text-overlay0">Target</label>
                  <Select
                    value={draftAction.data.target.type}
                    onValueChange={t => {
                      const target: ActionTarget = t === "Node" ? { type: "Node", id: 0 } :
                        t === "TriggerSource" ? { type: "TriggerSource" } : { type: "SelfNode" };
                      const next: XrdsAction = { kind: "ModifyHealth", data: { ...draftAction.data, target } };
                      setDraftAction(next);
                      commitDraft(next, draftAtSecs);
                    }}
                    options={[
                      { value: "SelfNode", label: "Self" },
                      { value: "TriggerSource", label: "Trigger source" },
                      { value: "Node", label: "Node id…" },
                    ]}
                  />
                  {draftAction.data.target.type === "Node" && (
                    <input type="number" step={1} value={draftAction.data.target.id} className={numCls}
                      onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                      onChange={e => setDraftAction({ kind: "ModifyHealth", data: { ...draftAction.data, target: { type: "Node", id: +e.target.value } } })}
                      onBlur={() => commitDraft(draftAction, draftAtSecs)} />
                  )}
                </span>
                <span className="inline-flex items-center gap-[3px]">
                  <label className="text-[10px] text-overlay0">Delta</label>
                  <Select
                    value={draftAction.data.delta.type}
                    onValueChange={t => {
                      const delta: ActionValue = t === "FromTriggerSource" ? { type: "FromTriggerSource" } : { type: "Fixed", value: 0 };
                      const next: XrdsAction = { kind: "ModifyHealth", data: { ...draftAction.data, delta } };
                      setDraftAction(next);
                      commitDraft(next, draftAtSecs);
                    }}
                    options={[
                      { value: "Fixed", label: "Fixed value…" },
                      { value: "FromTriggerSource", label: "From trigger source" },
                    ]}
                  />
                  {draftAction.data.delta.type === "Fixed" && (
                    <input type="number" step={1} value={draftAction.data.delta.value} className={numCls}
                      onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                      onChange={e => setDraftAction({ kind: "ModifyHealth", data: { ...draftAction.data, delta: { type: "Fixed", value: +e.target.value } } })}
                      onBlur={() => commitDraft(draftAction, draftAtSecs)} />
                  )}
                </span>
              </>
            )}

            {draftAction.kind === "FireCustomEvent" && (
              <span className="inline-flex items-center gap-[3px] flex-1 min-w-[120px]">
                <label className="text-[10px] text-overlay0">Name</label>
                <input type="text" value={draftAction.data.name}
                  className="flex-1 text-text bg-surface0 rounded px-2 py-1 border border-transparent focus:outline focus:outline-1 focus:outline-blue"
                  onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                  onChange={e => setDraftAction({ kind: "FireCustomEvent", data: { name: e.target.value } })}
                  onBlur={() => commitDraft(draftAction, draftAtSecs)} />
              </span>
            )}

            {draftAction.kind === "Run" && (
              <>
                <RunnablePicker
                  value={draftAction.data.runnable}
                  runnableNames={runnableNames.filter(n => n !== ownRunnableName)}
                  onChange={v => {
                    const next: XrdsAction = { kind: "Run", data: { ...draftAction.data, runnable: v } };
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }}
                />
                <Checkbox
                  label="Wait"
                  title="Block this sequence until the runnable finishes. Ignored inside a timeline."
                  checked={draftAction.data.wait}
                  disabled={isTimeline}
                  onCheckedChange={v => {
                    const next: XrdsAction = { kind: "Run", data: { ...draftAction.data, wait: v } };
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }}
                />
                {draftAction.data.runnable === ownRunnableName && (
                  <span className="text-[10px] text-red">⚠ this runs itself — see Run's cycle-detection note</span>
                )}
              </>
            )}

            {draftAction.kind === "PlayGltfAnimation" && (
              <>
                <span className="inline-flex items-center gap-[3px]">
                  <label className="text-[10px] text-overlay0">Clip index</label>
                  <input type="number" step={1} min={0} value={draftAction.data.clip_index} className={numCls}
                    onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                    onChange={e => setDraftAction({ kind: "PlayGltfAnimation", data: { ...draftAction.data, clip_index: Math.max(0, Math.round(+e.target.value)) } })}
                    onBlur={() => commitDraft(draftAction, draftAtSecs)} />
                </span>
                <span className="inline-flex items-center gap-[3px]">
                  <label className="text-[10px] text-overlay0">Speed</label>
                  <input type="number" step={0.1} value={draftAction.data.speed} className={numCls}
                    onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                    onChange={e => setDraftAction({ kind: "PlayGltfAnimation", data: { ...draftAction.data, speed: +e.target.value } })}
                    onBlur={() => commitDraft(draftAction, draftAtSecs)} />
                </span>
                <span className="inline-flex items-center gap-[3px]">
                  <label className="text-[10px] text-overlay0">Repeat</label>
                  <Select
                    value={draftAction.data.repeat}
                    onValueChange={v => {
                      const next: XrdsAction = { kind: "PlayGltfAnimation", data: { ...draftAction.data, repeat: v } };
                      setDraftAction(next);
                      commitDraft(next, draftAtSecs);
                    }}
                    options={[
                      { value: "Once", label: "Once" },
                      { value: "Loop", label: "Loop" },
                    ]}
                  />
                </span>
                <Checkbox
                  label="Start paused"
                  checked={draftAction.data.start_paused}
                  onCheckedChange={v => {
                    const next: XrdsAction = { kind: "PlayGltfAnimation", data: { ...draftAction.data, start_paused: v } };
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }}
                />
              </>
            )}

            {draftAction.kind === "StopGltfAnimation" && (
              <span className="text-[11px] text-overlay0">No fields — stops whatever glTF animation is playing on this node.</span>
            )}

            {draftAction.kind === "Unknown" && (
              <span className="text-[11px] text-overlay0">
                Unrecognized action — probably authored by a newer editor build. Skipped at runtime.
              </span>
            )}

            <span className="ml-auto inline-flex gap-1 items-center">
              {!isTimeline && (
                <>
                  <label className="text-[10px] text-overlay0" title="Execution order">Order</label>
                  <button className="tb-btn px-1.5 text-[10px]" disabled={selected === 0}
                    title="Move earlier" onClick={() => moveSelected(-1)}>▲</button>
                  <button className="tb-btn px-1.5 text-[10px]" disabled={selected === steps.length - 1}
                    title="Move later" onClick={() => moveSelected(1)}>▼</button>
                </>
              )}
              <button className="tb-btn text-red text-[10px] whitespace-nowrap"
                onClick={removeSelected}>✕ Remove</button>
            </span>
          </div>
        ) : found && (
          <div className="hud-slot-editor hud-slot-editor--hint">
            Click a {isTimeline ? "key" : "step"} to select it and edit its fields
          </div>
        )}

        {/* Timeline-only bottom bar: duration + looping */}
        {isTimeline && runnable && runnable.body.type === "Timeline" && (
          <div className="hud-canvas-bottom" onClick={e => e.stopPropagation()}>
            <label>Duration (s)</label>
            <input type="number" step={0.1} min={0}
              value={runnable.body.duration_secs ?? ""}
              placeholder="last key's at_secs"
              className="w-[90px] text-text bg-surface0 rounded px-2 py-1 border border-transparent focus:outline focus:outline-1 focus:outline-blue font-mono"
              onKeyDown={e => e.stopPropagation()}
              onChange={e => {
                const v = e.target.value === "" ? null : +e.target.value;
                send({ type: "SetTimelineDuration", payload: { name: runnable.name, duration_secs: v } });
              }} />

            <Checkbox
              label="Loop"
              className="ml-3"
              checked={runnable.body.looping}
              onCheckedChange={v => send({ type: "SetTimelineLooping", payload: { name: runnable.name, looping: v } })}
            />
          </div>
        )}
      </div>
    </div>
  );
}
