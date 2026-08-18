import { useEffect, useRef, useState } from "react";
import type {
  ActionTarget, ActionValue, EditorCommand, EditorSnapshot, NamedTrackDto, XrdsAction,
} from "../types/bridge";
import { TEXTURE_SLOTS } from "../types/bridge";
import { Select } from "./ui/Select";
import { Checkbox } from "./ui/Checkbox";

/** Action kinds an author may place on a row.
 *
 * `Wait`, `Run` and `FireCustomEvent` are absent because they no longer exist:
 * an event carries its own time, and a Track cannot start another Track.
 * `Teleport` is absent because it *was* a zero-duration `SetTransform` — the
 * Mode control below now expresses that as duration 0 vs > 0. */
export const ACTION_KINDS = [
  "SetTransform", "SetVisible", "SetMaterial",
  "PlayGltfAnimation", "StopGltfAnimation", "ModifyHealth",
  "PlayEffect", "StopEffect",
] as const;

export const ACTION_ICONS: Record<string, string> = {
  SetVisible: "👁", SetTransform: "🎬", SetMaterial: "🎨",
  ModifyHealth: "❤", PlayGltfAnimation: "🎞", StopGltfAnimation: "⏹",
  PlayEffect: "💥", StopEffect: "🛑", Unknown: "?",
};

/** Lane-category colour, matching the mockup's per-track dot colours — one
 * hue per action family so a key is identifiable at a glance on its lane. */
export const ACTION_COLOR: Record<string, string> = {
  SetTransform: "var(--teal)", SetVisible: "var(--mauve)",
  SetMaterial: "var(--flamingo)", ModifyHealth: "var(--red)",
  PlayGltfAnimation: "var(--blue)", StopGltfAnimation: "var(--surface1)",
  PlayEffect: "var(--peach)", StopEffect: "var(--surface1)",
  Unknown: "var(--surface1)",
};

/** `<input type="color">` only speaks hex RGB — no alpha — so `base_color`'s
 * alpha channel gets its own slider next to the swatch rather than folding it
 * into the hex string. */
function rgbToHex([r, g, b]: [number, number, number, number]): string {
  const byte = (c: number) => Math.round(Math.max(0, Math.min(1, c)) * 255).toString(16).padStart(2, "0");
  return `#${byte(r)}${byte(g)}${byte(b)}`;
}

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

export function summarizeAction(a: XrdsAction): string {
  switch (a.kind) {
    case "SetVisible": return `SetVisible(${a.data})`;
    case "SetTransform":
      return a.data.duration_secs > 0
        ? `Move · ${a.data.duration_secs}s ${a.data.ease}`
        : "Move (instant)";
    case "SetMaterial": return "SetMaterial";
    case "ModifyHealth": return "ModifyHealth";
    case "PlayGltfAnimation": return `PlayGltfAnimation(clip ${a.data.clip_index})`;
    case "StopGltfAnimation": return "StopGltfAnimation";
    // Spell out the default rather than showing "PlayEffect(null)": the count
    // falling back to the node's own Burst Count is the common case.
    case "PlayEffect":
      return a.data.count === null ? "PlayEffect (authored count)" : `PlayEffect × ${a.data.count}`;
    case "StopEffect": return "StopEffect (fade out)";
    // Element actions show their value: on a panel row the value *is* the point,
    // unlike SetMaterial where the detail lives in the editor below.
    case "SetElementText": return `Text "${a.data.text}"`;
    case "SetElementValue": return `Value ${a.data.value}`;
    case "SetElementEnabled": return a.data.enabled ? "Enable" : "Disable";
    case "Unknown": return "Unrecognized (newer editor)";
  }
}


// Radix Select.Item forbids an empty-string value; map to/from these
// sentinels at the boundary — same convention as every other picker here.
const ADD_STEP_SENTINEL = "__add__";
/** Radix Select.Item forbids `value=""`, and `null` is the domain's own way of
 *  saying "clear this slot" — so the empty choice needs a stand-in value. */
const TEXTURE_CLEAR_SENTINEL = "__clear__";

const DRAG_THRESHOLD_PX = 4;

/** A `<input type="number">` that can also be scrubbed: click it and it
 * behaves exactly like a normal number field (focus, type, arrow keys); drag
 * left/right past a small threshold and it adjusts the value by `step` per
 * pixel instead, Blender/Unity-style. Only wired into the Position/Rotation/
 * Scale fields below — the rest of the inspector's numeric inputs (health
 * deltas, timestamps, node ids) are typed rarely enough that scrubbing them
 * would not pay for the extra affordance to discover.
 *
 * The threshold is what lets one element do both: `setPointerCapture` is
 * deferred until a drag is actually detected, so a plain click never
 * captures the pointer and the browser's normal focus/caret behavior fires
 * unhindered. Values are computed from the *drag's own* start position each
 * move, not accumulated per-event, so a dropped or batched event during a
 * fast drag cannot cause drift. */
function DragNumber({ value, step, onChange, onCommit, className }: {
  value: number; step: number; onChange: (v: number) => void; onCommit: () => void; className?: string;
}) {
  const drag = useRef<null | { startX: number; startValue: number; dragging: boolean }>(null);

  return (
    <input type="number" step={step} value={value} className={className}
      title="Drag left/right to scrub"
      onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
      onChange={e => onChange(+e.target.value)}
      onBlur={onCommit}
      onPointerDown={e => {
        drag.current = { startX: e.clientX, startValue: value, dragging: false };
      }}
      onPointerMove={e => {
        const d = drag.current;
        if (!d) return;
        const dx = e.clientX - d.startX;
        if (!d.dragging) {
          if (Math.abs(dx) < DRAG_THRESHOLD_PX) return;
          d.dragging = true;
          (e.target as HTMLInputElement).setPointerCapture(e.pointerId);
          document.body.classList.add("seq-dragging-number");
        }
        onChange(Math.round((d.startValue + dx * step) * 10000) / 10000);
      }}
      onPointerUp={() => {
        const d = drag.current;
        drag.current = null;
        if (d?.dragging) {
          document.body.classList.remove("seq-dragging-number");
          onCommit();
        }
      }} />
  );
}

/** Mockup-style labelled field group (the uppercase mono caption above each
 * control block — see docs/Sequencer_Editor.dc.html's inspector column). */
function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="seq-field-label">{label}{hint && <span className="seq-field-hint"> ({hint})</span>}</div>
      {children}
    </div>
  );
}

/** Which event the inspector is editing: a row within a Track, and a key
 * within that row. A single index is no longer enough — events belong to
 * rows. */
export interface SelectedEvent {
  assetIndex: number;
  keyIndex: number;
}

interface Props {
  track: NamedTrackDto;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  selected: SelectedEvent | null;
  onSelectedChange: (s: SelectedEvent | null) => void;
  /** Which row a newly-added event should go on when nothing is selected. */
  activeAssetIndex: number | null;
  /** Time chosen by clicking a lane, or null to place after the row's last
   *  event (the original behaviour). */
  insertAtSecs: number | null;
  /** Trigger kinds that fire this Track, for the read-only "FIRES WHEN"
   * block — empty when nothing references it. */
  firesWhen: string[];
}

/** The Sequencer workspace's right-hand inspector column: add/remove, and
 * every field of the selected event. Split out of SequencerWorkspace so
 * neither file becomes unreadable; every mutation command lives here because
 * they are only ever issued from these controls. */
export function SequencerInspector({
  track, snapshot, send, selected, onSelectedChange: setSelected,
  activeAssetIndex, insertAtSecs, firesWhen,
}: Props) {
  const [draftAction, setDraftAction] = useState<XrdsAction | null>(null);
  const [draftAtSecs, setDraftAtSecs] = useState(0);
  // Switching a SetTransform to Instant sets duration 0, which would otherwise
  // lose the authored duration. Remembering it makes the Mode toggle
  // non-destructive to flip back and forth.
  const [lastDuration, setLastDuration] = useState(1);

  // Only textures can go in a texture slot — the catalog also holds glTF,
  // audio and environment maps, and offering those would produce an event
  // that fails to resolve at runtime.
  const textureAssets = snapshot.asset_catalog.filter(a => a.kind === "Texture");

  const selectedKey =
    selected === null
      ? null
      : track.assets[selected.assetIndex]?.keys[selected.keyIndex] ?? null;

  // Commands round-trip through Rust before `track` actually changes, so the
  // draft has to re-sync once the real data lands rather than only at click
  // time. Depending on the serialised key (not just the indices) does that.
  const selectedItemKey = JSON.stringify(selectedKey);
  useEffect(() => {
    if (selectedKey === null) { setDraftAction(null); return; }
    setDraftAction(selectedKey.action);
    setDraftAtSecs(selectedKey.at_secs);
    if (selectedKey.action.kind === "SetTransform" && selectedKey.action.data.duration_secs > 0) {
      setLastDuration(selectedKey.action.data.duration_secs);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [track.name, selectedItemKey]);

  function commitDraft(action: XrdsAction, atSecs: number) {
    if (selected === null) return;
    send({
      type: "SetTrackKey",
      payload: {
        track: track.name,
        asset_index: selected.assetIndex,
        key_index: selected.keyIndex,
        key: { at_secs: atSecs, action },
      },
    });
  }

  function addStep(kind: string) {
    // An event needs a row to live on. Prefer the selected row, else whatever
    // row the workspace says is active, else the first — and refuse outright
    // when the Track has no rows yet, since there is nowhere to put it.
    const assetIndex = selected?.assetIndex ?? activeAssetIndex ?? 0;
    if (track.assets.length === 0) return;
    // Place a new event just after the last one on that row, so successive
    // adds lay out along the ruler instead of stacking at t=0.
    const row = track.assets[assetIndex];
    // An explicit lane click wins; otherwise fall back to just after the row's
    // last event so successive adds lay out along the ruler instead of stacking.
    const at = insertAtSecs !== null
      ? insertAtSecs
      : row.keys.length === 0
        ? 0
        : +(Math.max(...row.keys.map(k => k.at_secs)) + 0.5).toFixed(3);
    send({
      type: "AddTrackKey",
      payload: { track: track.name, asset_index: assetIndex, at_secs: at, kind },
    });
    // Rust keeps rows sorted, so the new event lands last only because `at` is
    // the largest time on the row.
    setSelected({ assetIndex, keyIndex: row.keys.length });
  }

  function removeSelected() {
    if (selected === null) return;
    send({
      type: "RemoveTrackKey",
      payload: {
        track: track.name,
        asset_index: selected.assetIndex,
        key_index: selected.keyIndex,
      },
    });
    setSelected(null);
  }

  const numCls = "w-[70px] text-[11.5px] text-text px-2 py-1 rounded bg-well border border-surface0 focus:outline focus:outline-1 focus:outline-blue font-mono";

  function optionalVec3Field(
    label: string, value: [number, number, number] | null, step: number,
    buildNext: (v: [number, number, number] | null) => XrdsAction,
  ) {
    return (
      <div className="flex items-center gap-1.5 flex-wrap" key={label}>
        <Checkbox checked={value !== null} onCheckedChange={on => {
          const next = buildNext(on ? [0, 0, 0] : null);
          setDraftAction(next);
          commitDraft(next, draftAtSecs);
        }} />
        <label className="text-[10.5px] text-overlay0 w-16">{label}</label>
        {value !== null && [0, 1, 2].map(axis => (
          <DragNumber key={axis} step={step} value={value[axis]} className={numCls}
            onChange={next => {
              const v = [...value] as [number, number, number];
              v[axis] = next;
              setDraftAction(buildNext(v));
            }}
            onCommit={() => { if (draftAction) commitDraft(draftAction, draftAtSecs); }} />
        ))}
      </div>
    );
  }

  return (
    <div className="seq-ws-inspector">
      <div className="seq-ws-col-head">
        <span className="text-[11.5px] font-semibold text-text">Inspector</span>
        <span className="seq-tag">EVENT</span>
        <span className="flex-1" />
        <span className="text-[9.5px] text-overlay0 font-mono">
          {selected === null ? "none selected" : "1 selected"}
        </span>
      </div>

      <div className="flex items-center gap-1.5 px-3 py-2 border-b border-surface0">
        <label className="text-[10px] text-overlay0">Add</label>
        <Select
          value={ADD_STEP_SENTINEL}
          onValueChange={v => { if (v !== ADD_STEP_SENTINEL) addStep(v); }}
          options={[
            { value: ADD_STEP_SENTINEL, label: "+ event…" },
            ...ACTION_KINDS.map(kind => ({ value: kind, label: `${ACTION_ICONS[kind]} ${kind}` })),
          ]}
        />
        <span className="flex-1" />
        {selected !== null && (
          <button className="tb-btn text-red text-[10px]" onClick={removeSelected}>✕ Remove</button>
        )}
      </div>

      {!draftAction || selected === null ? (
        <div className="seq-ws-inspector-hint">
          {track.assets.length === 0
            ? "Add an asset row above, then click its lane to place an event."
            : insertAtSecs !== null
              ? `Next event will be added at t=${insertAtSecs.toFixed(2)}s — pick an action above.`
              : "Click a lane to choose where an event goes, or an existing event to edit it."}
        </div>
      ) : (
        <div className="flex-1 min-h-0 overflow-y-auto p-3 flex flex-col gap-3.5">
          {/* Selected-item summary strip, mirroring the mockup's header block */}
          <div className="seq-ws-sel-head">
            <span className="seq-dot" style={{ background: ACTION_COLOR[draftAction.kind] }} />
            <span className="text-[12px] font-semibold text-text">{draftAction.kind}</span>
            <span className="flex-1" />
            <span className="text-[10px] text-blue font-mono">@ {draftAtSecs.toFixed(2)}s</span>
          </div>

          {/* Time *is* the ordering: rows are kept sorted by `at_secs`, so
            * there is nothing to reorder by hand. */}
          <Field label="TIME" hint="seconds from the Track's start">
            <input type="number" step={0.05} min={0} value={draftAtSecs} className={numCls}
              onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
              onChange={e => setDraftAtSecs(+e.target.value)}
              onBlur={() => commitDraft(draftAction, draftAtSecs)} />
          </Field>

          {draftAction.kind === "SetVisible" && (
            <Field label="VISIBILITY">
              <Checkbox label="Visible" checked={draftAction.data}
                onCheckedChange={v => {
                  const next: XrdsAction = { kind: "SetVisible", data: v };
                  setDraftAction(next);
                  commitDraft(next, draftAtSecs);
                }} />
            </Field>
          )}

          {draftAction.kind === "SetTransform" && (
            <>
              {/* The Mode control. This used to switch between two action
                * variants (`Teleport` / `AnimateTransform`); `Teleport` was
                * deleted once it turned out to be exactly a zero-duration
                * `SetTransform`, so the same two-mode UX now just sets the
                * duration. Instant keeps the last non-zero duration in
                * `lastDuration` so toggling back does not lose it. */}
              <Field label="MODE">
                <Select
                  value={draftAction.data.duration_secs > 0 ? "interpolate" : "instant"}
                  onValueChange={mode => {
                    const next: XrdsAction = {
                      kind: "SetTransform",
                      data: {
                        ...draftAction.data,
                        duration_secs: mode === "instant" ? 0 : (lastDuration || 1),
                      },
                    };
                    if (mode === "interpolate" && draftAction.data.duration_secs > 0) return;
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }}
                  options={[
                    { value: "instant", label: "Instant" },
                    { value: "interpolate", label: "Interpolate" },
                  ]}
                />
                <div className="text-[10.5px] text-overlay0 leading-snug">
                  Instant is duration 0 — it draws as a dot. Interpolating draws as a bar
                  spanning its duration.
                </div>
              </Field>

              {draftAction.kind === "SetTransform" && (
                <>
                  <Field label="TRANSFORM" hint="unchecked = leave alone">
                    {optionalVec3Field("Position", draftAction.data.position, 0.1, v =>
                      ({ kind: "SetTransform", data: { ...draftAction.data, position: v } }))}
                    {optionalVec3Field("Rotation°", draftAction.data.rotation, 1, v =>
                      ({ kind: "SetTransform", data: { ...draftAction.data, rotation: v } }))}
                    {optionalVec3Field("Scale", draftAction.data.scale, 0.05, v =>
                      ({ kind: "SetTransform", data: { ...draftAction.data, scale: v } }))}
                  </Field>
                  <Field label="DURATION" hint="interpolate only">
                    <div className="flex items-center gap-2">
                      <input type="number" step={0.05} min={0} value={draftAction.data.duration_secs} className={numCls}
                        onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                        onChange={e => setDraftAction({ kind: "SetTransform", data: { ...draftAction.data, duration_secs: +e.target.value } })}
                        onBlur={() => commitDraft(draftAction, draftAtSecs)} />
                      <span className="text-[10px] text-overlay0">s</span>
                      <span className="flex-1" />
                      <label className="text-[10.5px] text-overlay0">Ease</label>
                      <Select
                        value={draftAction.data.ease}
                        onValueChange={ease => {
                          const next: XrdsAction = { kind: "SetTransform", data: { ...draftAction.data, ease } };
                          setDraftAction(next);
                          commitDraft(next, draftAtSecs);
                        }}
                        options={[
                          { value: "Linear", label: "Linear" },
                          { value: "Quad", label: "Quad" },
                          { value: "Cubic", label: "Cubic" },
                        ]}
                      />
                    </div>
                    {draftAction.data.duration_secs <= 0 && (
                      <span className="text-[10px] text-red">⚠ duration ≤ 0 — applies instantly, ease has no effect</span>
                    )}
                  </Field>
                </>
              )}
            </>
          )}

          {draftAction.kind === "SetMaterial" && (
            <>
              {/* No TARGET field any more: this action always applies to
                * whichever asset row it's on, same as SetTransform/SetVisible.
                * It used to carry its own target, independent of the row —
                * removed as a leftover from before rows were asset-scoped. */}
              <Field label="MATERIAL" hint="unchecked = leave alone">
                <div className="flex items-center gap-1.5 flex-wrap">
                  <Checkbox checked={draftAction.data.base_color !== null} onCheckedChange={on => {
                    const next: XrdsAction = { kind: "SetMaterial", data: { ...draftAction.data, base_color: on ? [1, 1, 1, 1] : null } };
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }} />
                  <label className="text-[10.5px] text-overlay0 w-16">Color</label>
                  {draftAction.data.base_color !== null && (
                    <>
                      <input type="color" value={rgbToHex(draftAction.data.base_color)}
                        className="w-7 h-6 rounded border border-surface0 bg-well p-0 cursor-pointer"
                        onChange={e => {
                          const [r, g, b] = hexToRgb(e.target.value);
                          const c: [number, number, number, number] = [r, g, b, draftAction.data.base_color![3]];
                          const next: XrdsAction = { kind: "SetMaterial", data: { ...draftAction.data, base_color: c } };
                          setDraftAction(next);
                          commitDraft(next, draftAtSecs);
                        }} />
                      <input type="range" min={0} max={1} step={0.01} value={draftAction.data.base_color[3]}
                        className="w-16 accent-[var(--blue)]" title="Opacity"
                        onChange={e => {
                          const c = [...draftAction.data.base_color!] as [number, number, number, number];
                          c[3] = +e.target.value;
                          const next: XrdsAction = { kind: "SetMaterial", data: { ...draftAction.data, base_color: c } };
                          setDraftAction(next);
                          commitDraft(next, draftAtSecs);
                        }} />
                      <span className="text-[10px] text-overlay0 font-mono w-9">
                        {Math.round(draftAction.data.base_color[3] * 100)}%
                      </span>
                    </>
                  )}
                </div>
                <div className="flex items-center gap-1.5">
                  <Checkbox checked={draftAction.data.metallic !== null} onCheckedChange={on => {
                    const next: XrdsAction = { kind: "SetMaterial", data: { ...draftAction.data, metallic: on ? 0 : null } };
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }} />
                  <label className="text-[10.5px] text-overlay0 w-16">Metallic</label>
                  {draftAction.data.metallic !== null && (
                    <>
                      <input type="range" min={0} max={1} step={0.01} value={draftAction.data.metallic}
                        className="w-16 accent-[var(--blue)]"
                        onChange={e => {
                          const next: XrdsAction = { kind: "SetMaterial", data: { ...draftAction.data, metallic: +e.target.value } };
                          setDraftAction(next);
                          commitDraft(next, draftAtSecs);
                        }} />
                      <span className="text-[10px] text-overlay0 font-mono w-9">
                        {Math.round(draftAction.data.metallic * 100)}%
                      </span>
                    </>
                  )}
                </div>
                <div className="flex items-center gap-1.5">
                  <Checkbox checked={draftAction.data.roughness !== null} onCheckedChange={on => {
                    const next: XrdsAction = { kind: "SetMaterial", data: { ...draftAction.data, roughness: on ? 0.5 : null } };
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }} />
                  <label className="text-[10.5px] text-overlay0 w-16">Roughness</label>
                  {draftAction.data.roughness !== null && (
                    <>
                      <input type="range" min={0} max={1} step={0.01} value={draftAction.data.roughness}
                        className="w-16 accent-[var(--blue)]"
                        onChange={e => {
                          const next: XrdsAction = { kind: "SetMaterial", data: { ...draftAction.data, roughness: +e.target.value } };
                          setDraftAction(next);
                          commitDraft(next, draftAtSecs);
                        }} />
                      <span className="text-[10px] text-overlay0 font-mono w-9">
                        {Math.round(draftAction.data.roughness * 100)}%
                      </span>
                    </>
                  )}
                </div>
              </Field>

              <Field label="TEXTURE" hint="one slot per event">
                <div className="flex items-center gap-1.5 flex-wrap">
                  <Checkbox checked={draftAction.data.texture !== null} onCheckedChange={on => {
                    const next: XrdsAction = {
                      kind: "SetMaterial",
                      data: {
                        ...draftAction.data,
                        texture: on ? { slot: "BaseColor", texture_asset_id: null } : null,
                      },
                    };
                    setDraftAction(next);
                    commitDraft(next, draftAtSecs);
                  }} />
                  <label className="text-[10.5px] text-overlay0 w-16">Slot</label>
                  {draftAction.data.texture !== null && (
                    <Select
                      value={draftAction.data.texture.slot}
                      onValueChange={slot => {
                        const next: XrdsAction = {
                          kind: "SetMaterial",
                          data: { ...draftAction.data, texture: { ...draftAction.data.texture!, slot } },
                        };
                        setDraftAction(next);
                        commitDraft(next, draftAtSecs);
                      }}
                      options={TEXTURE_SLOTS.map(s => ({ value: s, label: s }))}
                    />
                  )}
                </div>
                {draftAction.data.texture !== null && (
                  <>
                    <div className="flex items-center gap-1.5 flex-wrap">
                      <span className="w-[18px]" />
                      <label className="text-[10.5px] text-overlay0 w-16">Image</label>
                      <Select
                        value={draftAction.data.texture.texture_asset_id ?? TEXTURE_CLEAR_SENTINEL}
                        onValueChange={v => {
                          const next: XrdsAction = {
                            kind: "SetMaterial",
                            data: {
                              ...draftAction.data,
                              texture: {
                                ...draftAction.data.texture!,
                                texture_asset_id: v === TEXTURE_CLEAR_SENTINEL ? null : v,
                              },
                            },
                          };
                          setDraftAction(next);
                          commitDraft(next, draftAtSecs);
                        }}
                        options={[
                          { value: TEXTURE_CLEAR_SENTINEL, label: "— clear this slot —" },
                          ...textureAssets.map(a => ({ value: a.id, label: a.name })),
                        ]}
                      />
                    </div>
                    {textureAssets.length === 0 && (
                      <span className="text-[10px] text-yellow">
                        ⚠ no texture assets imported yet — this event can only clear the slot
                      </span>
                    )}
                  </>
                )}
              </Field>
            </>
          )}


          {draftAction.kind === "ModifyHealth" && (
            <>
              {/* No TARGET field any more — see the SetMaterial block above
                * for why. */}
              <Field label="DELTA">
                <div className="flex items-center gap-1.5 flex-wrap">
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
                </div>
              </Field>
            </>
          )}



          {draftAction.kind === "PlayGltfAnimation" && (
            <Field label="GLTF ANIMATION">
              <div className="flex items-center gap-1.5 flex-wrap">
                <label className="text-[10.5px] text-overlay0 w-16">Clip</label>
                <input type="number" step={1} min={0} value={draftAction.data.clip_index} className={numCls}
                  onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                  onChange={e => setDraftAction({ kind: "PlayGltfAnimation", data: { ...draftAction.data, clip_index: Math.max(0, Math.round(+e.target.value)) } })}
                  onBlur={() => commitDraft(draftAction, draftAtSecs)} />
              </div>
              <div className="flex items-center gap-1.5 flex-wrap">
                <label className="text-[10.5px] text-overlay0 w-16">Speed</label>
                <input type="number" step={0.1} value={draftAction.data.speed} className={numCls}
                  onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                  onChange={e => setDraftAction({ kind: "PlayGltfAnimation", data: { ...draftAction.data, speed: +e.target.value } })}
                  onBlur={() => commitDraft(draftAction, draftAtSecs)} />
              </div>
              <div className="flex items-center gap-1.5 flex-wrap">
                <label className="text-[10.5px] text-overlay0 w-16">Repeat</label>
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
              </div>
              <Checkbox
                label="Start paused"
                checked={draftAction.data.start_paused}
                onCheckedChange={v => {
                  const next: XrdsAction = { kind: "PlayGltfAnimation", data: { ...draftAction.data, start_paused: v } };
                  setDraftAction(next);
                  commitDraft(next, draftAtSecs);
                }}
              />
            </Field>
          )}

          {draftAction.kind === "StopGltfAnimation" && (
            <div className="text-[11px] text-overlay0">No fields — stops whatever glTF animation is playing on this node.</div>
          )}

          {draftAction.kind === "Unknown" && (
            <div className="text-[11px] text-overlay0">
              Unrecognized action — probably authored by a newer editor build. Skipped at runtime.
            </div>
          )}

          {/* Read-only "FIRES WHEN" — the mockup's binding readout. Authored
            * on the node's Triggers, not here; this only reports it. */}
          <Field label="FIRES WHEN" hint="read-only">
            {firesWhen.length === 0 ? (
              <div className="text-[10.5px] text-overlay0 leading-snug">
                Nothing references this yet — bind it from a node's Triggers in the Inspector.
              </div>
            ) : (
              <div className="seq-fires-when">
                <span className="text-yellow">⚡</span>
                <span className="font-mono text-[10.5px]">{firesWhen.join(", ")}</span>
              </div>
            )}
          </Field>
        </div>
      )}
    </div>
  );
}
