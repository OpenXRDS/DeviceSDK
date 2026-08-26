import { useState, useEffect, useRef, useCallback } from "react";
import type {
  EditorSnapshot, EditorCommand, NodeInspector, MaterialParams, MaterialTextures,
  AssetCatalogEntry, EnvironmentDto, FogFalloff,
  ObservableDto, ThresholdWatcherDto, XrdsTriggerKind, NodePayload, TriggerEffect,
} from "../types/bridge";
import { rgbaToHex, hexToRgba, MATERIAL_TEXTURE_SLOTS, TRIGGER_EFFECTS } from "../types/bridge";
import { Select } from "./ui/Select";
import { Checkbox } from "./ui/Checkbox";
import { PanelInstanceTriggers } from "./PanelInstanceTriggers";
import { ALL_TRIGGER_KINDS, isHandFilterVisible, unavailableReasonFor } from "../lib/sequencer";

interface Props {
  snapshot: EditorSnapshot;
  send:     (cmd: EditorCommand) => void;
  /** Opens a named Track in the Sequencer workspace. */
  onOpenTrack: (name: string) => void;
  /** Whether to show the scene-environment sections (Fog/Exposure/IBL) in
   * the nothing-selected state. Off in the Sequencer workspace — those are
   * scene-environment settings, not behaviour authoring, so they're only
   * noise there. Defaults on. */
  showEnvironment?: boolean;
}

// ---------------------------------------------------------------------------
// Scrub field — pointer-drag to change, click to type
// ---------------------------------------------------------------------------
interface ScrubProps {
  axis: "x"|"y"|"z";
  value: number;
  onLive: (v: number) => void;
  onCommit: (v: number) => void;
  step?: number;
}
function ScrubField({ axis, value, onLive, onCommit, step = 0.01 }: ScrubProps) {
  const [local, setLocal] = useState(value.toFixed(3));
  const pointerDown = useRef(false);
  const dragging    = useRef(false);
  const startX      = useRef(0);
  const startVal    = useRef(0);
  const wrapRef     = useRef<HTMLDivElement>(null);

  // Sync from outside only when not focused
  useEffect(() => {
    if (document.activeElement !== wrapRef.current?.querySelector("input")) {
      setLocal(value.toFixed(3));
    }
  }, [value]);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    const input = wrapRef.current?.querySelector("input");
    if (document.activeElement === input) return;
    e.preventDefault();
    pointerDown.current = true; dragging.current = false;
    startX.current   = e.clientX;
    startVal.current = parseFloat(local) || 0;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }, [local]);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!pointerDown.current) return;
    const dx = e.clientX - startX.current;
    if (!dragging.current && Math.abs(dx) > 4) dragging.current = true;
    if (!dragging.current) return;
    const v = startVal.current + dx * step;
    setLocal(v.toFixed(3));
    onLive(v);
  }, [step, onLive]);

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    if (!pointerDown.current) return;
    pointerDown.current = false;
    if (dragging.current) {
      onCommit(parseFloat(local) || 0);
      dragging.current = false;
    } else {
      const input = wrapRef.current?.querySelector("input") as HTMLInputElement | null;
      input?.focus(); input?.select();
    }
  }, [local, onCommit]);

  const axisColor = axis === "x" ? "ax-x" : axis === "y" ? "ax-y" : "ax-z";
  const axisLabel = axis.toUpperCase();

  return (
    <div className="scrub-wrap" ref={wrapRef}
         onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp}>
      <span className={`scrub-axis ${axisColor}`}>{axisLabel}</span>
      <input
        type="text"
        value={local}
        onChange={e => { setLocal(e.target.value); onLive(parseFloat(e.target.value) || 0); }}
        onBlur={e => onCommit(parseFloat(e.target.value) || 0)}
        onKeyDown={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); e.stopPropagation(); }}
      />
    </div>
  );
}

// T/R/S row
interface Vec3RowProps {
  rowClass: string; rowLabel: string;
  values: [number,number,number];
  onLive: (v: [number,number,number]) => void;
  onCommit: (v: [number,number,number]) => void;
  step?: number;
}
function Vec3Row({ rowClass, rowLabel, values, onLive, onCommit, step }: Vec3RowProps) {
  const cur = useRef<[number,number,number]>([...values]);
  // Keep cur in sync when values change externally
  useEffect(() => { cur.current = [...values]; }, [values]);

  return (
    <div className="tf-row">
      <span className={`tf-lbl ${rowClass}`}>{rowLabel}</span>
      {(["x","y","z"] as const).map((ax, i) => (
        <ScrubField key={ax} axis={ax} value={values[i]} step={step}
          onLive={v => { cur.current[i] = v; onLive([...cur.current]); }}
          onCommit={v => { cur.current[i] = v; onCommit([...cur.current]); }}
        />
      ))}
    </div>
  );
}

// Slider row
function SliderRow({ label, value, min, max, step, onLive, onCommit, disabled }: {
  label: string; value: number; min: number; max: number; step: number;
  onLive: (v: number) => void; onCommit: (v: number) => void; disabled?: boolean;
}) {
  const [local, setLocal] = useState(value);
  const [text, setText]   = useState(value.toFixed(step < 1 ? 2 : 0));
  useEffect(() => {
    setLocal(value);
    setText(value.toFixed(step < 1 ? 2 : 0));
  }, [value, step]);

  function applyText(raw: string) {
    if (disabled) return;
    const v = parseFloat(raw);
    if (isNaN(v)) { setText(local.toFixed(step < 1 ? 2 : 0)); return; }
    setLocal(v);
    setText(v.toFixed(step < 1 ? 2 : 0));
    onCommit(v);
  }

  return (
    <div className="insp-row" style={{ gap: 4, opacity: disabled ? 0.4 : 1 }}>
      <label>{label}</label>
      <input type="range" min={min} max={max} step={step}
        value={Math.min(max, Math.max(min, local))}  // clamp for display only
        style={{ flex: 1 }}
        disabled={disabled}
        onChange={e => { if (disabled) return; const v = +e.target.value; setLocal(v); setText(v.toFixed(step < 1 ? 2 : 0)); onLive(v); }}
        onMouseUp={e  => { if (!disabled) onCommit(+(e.target as HTMLInputElement).value); }}
      />
      {/* Direct number entry — accepts values outside slider range */}
      <input type="text" value={text}
        style={{ width: 52, background:"var(--surface0)", color:"var(--text)",
                 border:"1px solid var(--surface1)", borderRadius:3,
                 padding:"2px 4px", fontSize:11, fontFamily:"monospace",
                 flexShrink: 0 }}
        disabled={disabled}
        onChange={e => setText(e.target.value)}
        onBlur={e  => applyText(e.target.value)}
        onKeyDown={e => {
          e.stopPropagation();
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        }}
      />
    </div>
  );
}

// Color row
function ColorRow({ label, color, onLive, onCommit }: {
  label: string; color: [number,number,number,number];
  onLive: (c: [number,number,number,number]) => void;
  onCommit: (c: [number,number,number,number]) => void;
}) {
  return (
    <div className="insp-row">
      <label>{label}</label>
      <input type="color" value={rgbaToHex(color)}
        onChange={e => onLive(hexToRgba(e.target.value, color[3]))}
        onBlur={e => onCommit(hexToRgba((e.target as HTMLInputElement).value, color[3]))}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Inspector
// ---------------------------------------------------------------------------
/** Payload kinds that never offer a grabbable checkbox.
 *
 *  Mostly this is "no geometry, so grab can never hit it": grab raycasts against
 *  `Aabb`, which Bevy computes from `Mesh3d`, and a node whose visuals live
 *  entirely on child entities has neither. Listed as an exclusion rather than an
 *  allow-list: every ordinary payload spawns a mesh, so an allow-list would
 *  silently drop grabbing from any future kind that forgot to register — the
 *  failure would be a missing checkbox, which nobody reports.
 *
 *  `Empty` is the honest case: a grouping node has nothing to grab.
 *
 *  `Effect` is the same story for a different reason: `spawn_effect_descriptor`
 *  inserts a `ParticleSpawner` and no `Mesh3d`, so there is no `Aabb` for a grab
 *  raycast to hit. The particles themselves are GPU/CPU-simulated points with no
 *  collision representation. Offering the checkbox would arm `XrGrabbable` on an
 *  entity that can never be picked up.
 *
 *  **`Panel` is deliberately *not* on this list.** It has real geometry — the
 *  backdrop from `apply_panel_backdrop_in_world`, with its `Aabb` backfilled by
 *  `ensure_aabbs_for_unculled_meshes_system` — so grab reaches it fine. The
 *  earlier worry was that arming grab on a panel steals the clicks its elements
 *  need, which was a fair objection to grabbing the *face*, not to grabbing the
 *  panel. It is resolved in the runtime instead: the checkbox arms a handle bar
 *  below the panel and `XrGrabHandleOnly` refuses a grab that starts anywhere
 *  else, so pressing a button can never drag its panel. */
const KINDS_WITHOUT_GEOMETRY = new Set(["Empty", "Player", "PlayerAnchor", "Effect"]);

function canBeGrabbed(payloadKind: string): boolean {
  return !KINDS_WITHOUT_GEOMETRY.has(payloadKind);
}

export function Inspector({ snapshot, send, onOpenTrack,
                            showEnvironment = true }: Props) {
  const [tab, setTab] = useState<"inspector" | "environment">("inspector");
  const node = snapshot.selected_node;
  const prevId = useRef<number | null>(null);

  // Local state for transform (persists during scrubbing)
  const [tVal, setTVal] = useState<[number,number,number]>([0,0,0]);
  const [rVal, setRVal] = useState<[number,number,number]>([0,0,0]);
  const [sVal, setSVal] = useState<[number,number,number]>([1,1,1]);
  // True while a scrub drag is in progress — prevents snapshot from overwriting local state
  const isDragging = useRef(false);

  // Sync from snapshot whenever NOT dragging (covers both node change and gizmo commits)
  useEffect(() => {
    if (!node) { prevId.current = null; return; }
    const nodeChanged = node.id !== prevId.current;
    if (nodeChanged) prevId.current = node.id;
    if (nodeChanged || !isDragging.current) {
      setTVal([...node.translation]);
      setRVal([...node.rotation_euler_degrees]);
      setSVal([...node.scale]);
    }
  }, [node]);

  // Scene settings are a peer of node properties, not the empty state for them.
  //
  // They used to render only when nothing was selected, which had two costs: fog,
  // exposure and IBL could not be touched while a node was selected — you had to
  // deselect first — and any new scene-wide setting landed somewhere an author had
  // no reason to look. Passthrough went in that way and was duly hard to find.
  //
  // In the Sequencer workspace there is no Environment tab at all: that layout is
  // for behaviour authoring, and `showEnvironment` already encoded that intent.
  const tabs = (
    <div className="insp-tabs">
      <button className={`insp-tab${tab === "inspector" ? " active" : ""}`}
        onClick={() => setTab("inspector")}>Inspector</button>
      {showEnvironment && (
        <button className={`insp-tab${tab === "environment" ? " active" : ""}`}
          onClick={() => setTab("environment")}>Environment</button>
      )}
    </div>
  );

  // Falls back to the Inspector tab when Environment is unavailable, so switching
  // into the Sequencer while on Environment does not leave a blank panel.
  const showingEnvironment = tab === "environment" && showEnvironment;

  if (showingEnvironment) {
    return (
      <div className="inspector">
        {tabs}
        <SceneEnvironmentSection
          env={snapshot.environment}
          passthrough={snapshot.xr_passthrough}
          assets={snapshot.asset_catalog}
          send={send}
        />
      </div>
    );
  }

  if (!node) {
    return (
      <div className="inspector">
        {tabs}
        <div className="insp-empty">
          Select a node to inspect it — or to bind a trigger to it under Triggers.
        </div>
      </div>
    );
  }

  const id = node.id;
  const commitTf = (t: [number,number,number], r: [number,number,number], s: [number,number,number]) =>
    send({ type: "CommitTransform", payload: { id, translation: t, rotation_euler_degrees: r, scale: s } });

  return (
    <div className="inspector">
      {tabs}

      {/* Node header */}
      <div className="insp-section">
        <div className="insp-name-row">
          <input type="text" key={node.id} defaultValue={node.name}
            onKeyDown={e => e.stopPropagation()}
            onBlur={e => send({ type: "RenameNode", payload: { id, name: e.target.value } })}
            onKeyUp={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          />
          <input type="checkbox" key={`vis-${node.id}`} defaultChecked={node.visible} title="Visible"
            style={{ accentColor: "var(--blue)", cursor: "pointer", width: 16, height: 16 }}
            onChange={e => send({ type: "SetVisible", payload: { id, visible: e.target.checked } })}
          />
          {/* Hidden for the kinds in KINDS_WITHOUT_GEOMETRY — see there for why
            * each one is on the list. Offering a checkbox that does nothing is
            * worse than offering nothing: the author ticks it and concludes grab
            * is broken.
            *
            * On a `Panel` this means "show the grab handle", not "make the whole
            * surface draggable": the face has to stay clickable for its elements,
            * so a bar appears under the panel and grab only starts there — the
            * Meta Quest model. Head-locked panels get no handle at all, since
            * `head_locked_system` would undo the move. See
            * `sync_panel_grab_handles_system` in the runtime. */}
          {canBeGrabbed(node.payload.type) && (
            <input type="checkbox" key={`grab-${node.id}`} defaultChecked={node.grabbable}
              title="Grabbable (XR trigger)"
              style={{ accentColor: "var(--green)", cursor: "pointer", width: 16, height: 16 }}
              onChange={e => send({ type: "SetGrabbable", payload: { id, grabbable: e.target.checked } })}
            />
          )}
        </div>
        <div className="insp-kind">{node.payload.type}</div>
      </div>

      {/* Transform — hidden for HudText (head-locked) */}
      {node.payload.type !== "HudText" && <div className="insp-section">
        <h4>
          Transform
          {node.parent_id != null && (
            <span style={{ color: node.parent_kind === "PlayerAnchor" ? "var(--blue)" : "var(--overlay0)",
                           fontSize:9, fontWeight:"normal", marginLeft:6, letterSpacing:0 }}>
              {node.parent_kind === "PlayerAnchor" ? "anchor-local offset" : "local to parent"}
            </span>
          )}
        </h4>
        <Vec3Row rowClass="tf-t" rowLabel="T" values={tVal}
          onLive={v  => { isDragging.current = true;  setTVal(v); send({ type: "SetTranslation",   payload: { id, value: v } }); }}
          onCommit={v => { isDragging.current = false; setTVal(v); commitTf(v, rVal, sVal); }}
        />
        <Vec3Row rowClass="tf-r" rowLabel="R" values={rVal} step={0.5}
          onLive={v  => { isDragging.current = true;  setRVal(v); send({ type: "SetRotationEuler", payload: { id, degrees: v } }); }}
          onCommit={v => { isDragging.current = false; setRVal(v); commitTf(tVal, v, sVal); }}
        />
        <Vec3Row rowClass="tf-s" rowLabel="S" values={sVal}
          onLive={v  => { isDragging.current = true;  setSVal(v); send({ type: "SetScale",         payload: { id, value: v } }); }}
          onCommit={v => { isDragging.current = false; setSVal(v); commitTf(tVal, rVal, v); }}
        />
      </div>}

      {/* Payload-specific sections — key forces remount on node change so useState re-initialises */}
      <PayloadSection key={node.id} node={node} send={send} isPlaying={snapshot.is_playing} snapshot={snapshot} />

      {/* Applies regardless of payload kind — any node can carry triggers/watchers. */}
      <TriggersSection node={node} snapshot={snapshot} send={send} onOpenTrack={onOpenTrack} />
      <WatchersSection node={node} send={send} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Triggers / Watchers — applies to every node regardless of payload kind
// (Phase 6, see docs/xrds-trigger-action-editor-plan.md Stages 3/4)
// ---------------------------------------------------------------------------

// "Unknown" isn't in ALL_TRIGGER_KINDS/validKindsFor (sequencer.ts) since
// nothing at runtime ever emits it — but it doubles as this picker's
// "nothing picked yet" placeholder for a freshly added binding (it
// already means "never fires" at runtime, matching a None state exactly;
// the domain type has no real None variant), so it's prepended here
// rather than added to the shared list every other consumer of
// validKindsFor would also get it.
const UNKNOWN_KIND_LABEL: Record<string, string> = {
  Unknown: "— none selected —",
};

// Radix Select.Item forbids an empty-string value, but `null`/"" is this
// section's existing "no restriction/no runnable" convention — map to/from
// these sentinels at the Select boundary instead of touching that convention.
const HAND_ANY_SENTINEL = "__any__";
const TRACK_NONE_SENTINEL = "__none__";
/** Radix Select.Item forbids `value=""`, and `null` is the domain's own "no
 *  texture in this slot" — so the empty choice needs a stand-in value. */
const TEXTURE_NONE_SENTINEL = "__no_texture__";

function triggerKindFromSelectValue(k: string): XrdsTriggerKind {
  if (k === "Custom") return { kind: "Custom", data: "" };
  return { kind: k } as XrdsTriggerKind;
}

function TriggersSection({ node, snapshot, send, onOpenTrack }: {
  node: NodeInspector;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  /** Opens a named Track in the Sequencer workspace. */
  onOpenTrack: (name: string) => void;
}) {
  const id = node.id;
  return (
    <div className="insp-section">
      <h4>Triggers</h4>
      {node.triggers.length === 0 && (
        <div className="hud-library-empty">
          No trigger bindings yet. "+ Add binding" fires an authored sequence when this node's
          trigger kind occurs.
        </div>
      )}
      {node.triggers.map((b, i) => {
        const kind = b.trigger.kind;
        const handDisallowed = b.hand !== null && !isHandFilterVisible(kind);
        const trackMissing = b.track !== null && !snapshot.tracks.some(t => t.name === b.track);
        // Every kind is shown, always — including ones this node can't fire
        // yet — with a trailing hint saying what's missing, rather than
        // silently shortening the list. A short, unexplained list is exactly
        // what prompted this: there was no way to tell "not offered because
        // it doesn't apply" from "missing/broken".
        const kindOptions = ALL_TRIGGER_KINDS.map(k => {
          const reason = unavailableReasonFor(k, node);
          return { value: k, label: k, disabled: reason !== null, hint: reason ?? undefined };
        });
        return (
          <div key={i} className="hud-library-row flex-col items-stretch gap-2">
            <div className="grid grid-cols-[42px_1fr] items-center gap-x-2">
              <label className="text-[11px] text-overlay0">When</label>
              <div className="flex items-center gap-1.5 flex-wrap">
                <Select
                  value={kind}
                  onValueChange={v => send({ type: "SetTriggerBindingTrigger", payload: { node_id: id, index: i, trigger: triggerKindFromSelectValue(v) } })}
                  options={[{ value: "Unknown", label: UNKNOWN_KIND_LABEL.Unknown }, ...kindOptions]}
                />
                {kind === "Custom" && (
                  <input type="text" defaultValue={b.trigger.kind === "Custom" ? b.trigger.data : ""}
                    placeholder="event name"
                    className="w-[100px] text-bright bg-well rounded px-2 py-1 border border-surface0 focus:outline focus:outline-1 focus:outline-blue"
                    key={`${id}-${i}-custom-${b.trigger.kind === "Custom" ? b.trigger.data : ""}`}
                    onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                    onBlur={e => send({ type: "SetTriggerBindingTrigger", payload: { node_id: id, index: i, trigger: { kind: "Custom", data: e.target.value } } })} />
                )}
                {isHandFilterVisible(kind) && (
                  <Select
                    value={b.hand ?? HAND_ANY_SENTINEL}
                    onValueChange={v => send({ type: "SetTriggerBindingHand", payload: { node_id: id, index: i, hand: v === HAND_ANY_SENTINEL ? null : v } })}
                    options={[
                      { value: HAND_ANY_SENTINEL, label: "any hand" },
                      { value: "Left", label: "Left" },
                      { value: "Right", label: "Right" },
                    ]}
                  />
                )}
                <span className="flex-1" />
                <Checkbox
                  label="disabled"
                  checked={b.disabled}
                  onCheckedChange={v => send({ type: "SetTriggerBindingDisabled", payload: { node_id: id, index: i, disabled: v } })}
                />
                <button className="tb-btn text-[11px]"
                  title="Fire this binding right now — there's no real ZoneEnter/Grabbed/etc event in the desktop editor to wait for"
                  disabled={b.disabled || handDisallowed}
                  onClick={() => send({ type: "PreviewFireTrigger", payload: { node_id: id, index: i } })}>
                  ▶ Fire
                </button>
                <button className="tb-btn text-red text-[11px]"
                  title="Remove this binding"
                  onClick={() => send({ type: "RemoveTriggerBinding", payload: { node_id: id, index: i } })}>✕</button>
              </div>
            </div>

            {handDisallowed && (
              <div className="grid grid-cols-[42px_1fr] gap-x-2">
                <span />
                <span className="text-[11px] text-red">
                  ⚠ {kind} never reports a hand — this binding can never fire
                </span>
              </div>
            )}

            {/* A binding names a Track. There is no inline alternative, so
              * this is a plain picker rather than an inline-vs-named choice. */}
            <div className="grid grid-cols-[42px_1fr] items-center gap-x-2">
              {/* Label follows the effect so the row reads as one sentence —
                * "Stops → Open" — rather than a fixed "Fires" beside a picker
                * that says Stop. */}
              <label className="text-[11px] text-overlay0">
                {b.effect === "Stop" ? "Stops" : "Fires"}
              </label>
              <div className="flex items-center gap-1.5 flex-wrap">
                <Select
                  value={b.effect}
                  onValueChange={v => send({
                    type: "SetTriggerBindingEffect",
                    payload: { node_id: id, index: i, effect: v as TriggerEffect },
                  })}
                  options={TRIGGER_EFFECTS.map(e => ({ value: e, label: e }))}
                />
                <Select
                  value={b.track ?? TRACK_NONE_SENTINEL}
                  onValueChange={v => send({
                    type: "SetTriggerBindingTrack",
                    payload: { node_id: id, index: i, track: v === TRACK_NONE_SENTINEL ? null : v },
                  })}
                  options={[
                    { value: TRACK_NONE_SENTINEL, label: "— nothing —" },
                    ...snapshot.tracks.map(t => ({ value: t.name, label: t.name })),
                  ]}
                />
                {b.track === null && (
                  <span className="text-[11px] text-yellow">
                    ⚠ fires nothing yet
                  </span>
                )}
                {trackMissing && (
                  <span className="text-[11px] text-red">⚠ "{b.track}" is not in this document</span>
                )}
                {b.track !== null && !trackMissing && (
                  <button className="tb-btn text-[11px] py-px px-[7px]"
                    title={`Open "${b.track}" in the Sequencer`}
                    onClick={() => onOpenTrack(b.track!)}>
                    Open ↗
                  </button>
                )}
              </div>
            </div>
          </div>
        );
      })}
      <button className="tb-btn text-[11px] px-2 py-0.5"
        onClick={() => send({ type: "AddTriggerBinding", payload: { node_id: id } })}>
        + Add binding
      </button>
      {node.trigger_diagnostics.length > 0 && (
        <div className="flex flex-col gap-0.5 mt-1">
          {node.trigger_diagnostics.map((d, i) => (
            <span key={i} className={`text-[11px] ${d.severity === "error" ? "text-red" : "text-overlay0"}`} title={d.detail}>
              ⚠ {d.title}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

const OBSERVABLE_KINDS = ["Height", "ScaleMagnitude", "RotationDegrees", "DistanceTo"] as const;

function defaultObservableForKind(kind: string): ObservableDto {
  switch (kind) {
    case "RotationDegrees": return { type: "RotationDegrees", axis: "Y" };
    case "DistanceTo": return { type: "DistanceTo", node: 0 };
    case "ScaleMagnitude": return { type: "ScaleMagnitude" };
    default: return { type: "Height" };
  }
}

function WatchersSection({ node, send }: {
  node: NodeInspector;
  send: (cmd: EditorCommand) => void;
}) {
  const id = node.id;
  const setWatcher = (i: number, watcher: ThresholdWatcherDto) =>
    send({ type: "SetWatcher", payload: { node_id: id, index: i, watcher } });
  const numCls = "text-bright bg-well rounded px-2 py-1 border border-surface0 focus:outline focus:outline-1 focus:outline-blue font-mono";
  const textCls = "text-bright bg-well rounded px-2 py-1 border border-surface0 focus:outline focus:outline-1 focus:outline-blue";

  return (
    <div className="insp-section">
      <h4>Threshold Watchers</h4>
      {node.watchers.length === 0 && (
        <div className="hud-library-empty">
          No watchers yet. A watcher turns a continuous value (height, distance…) into a Custom
          trigger crossing a threshold.
        </div>
      )}
      {node.watchers.map((w, i) => (
        <div key={i} className="hud-library-row flex-col items-stretch gap-1">
          <div className="flex gap-1 items-center flex-wrap">
            <Select
              value={w.observable.type}
              onValueChange={v => setWatcher(i, { ...w, observable: defaultObservableForKind(v) })}
              options={OBSERVABLE_KINDS.map(k => ({ value: k, label: k }))}
            />
            {w.observable.type === "RotationDegrees" && (
              <Select
                value={w.observable.axis}
                onValueChange={v => setWatcher(i, { ...w, observable: { type: "RotationDegrees", axis: v } })}
                options={[
                  { value: "X", label: "X" },
                  { value: "Y", label: "Y" },
                  { value: "Z", label: "Z" },
                ]}
              />
            )}
            {w.observable.type === "DistanceTo" && (
              <input type="number" step={1} className={`w-[60px] ${numCls}`}
                key={`${id}-${i}-distnode-${w.observable.node}`}
                defaultValue={w.observable.node}
                onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                onBlur={e => setWatcher(i, { ...w, observable: { type: "DistanceTo", node: +e.target.value } })} />
            )}
            <Select
              value={w.crossing}
              onValueChange={v => setWatcher(i, { ...w, crossing: v })}
              options={[
                { value: "Above", label: "Above" },
                { value: "Below", label: "Below" },
                { value: "Either", label: "Either" },
              ]}
            />
            <button className="tb-btn text-red text-[10px] ml-auto"
              title="Remove this watcher"
              onClick={() => send({ type: "RemoveWatcher", payload: { node_id: id, index: i } })}>✕</button>
          </div>

          <div className="flex gap-1.5 items-center flex-wrap">
            <label className="text-[10px] text-overlay0">value</label>
            <input type="number" step={0.1} className={`w-16 ${numCls}`}
              key={`${id}-${i}-value-${w.value}`} defaultValue={w.value}
              onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
              onBlur={e => setWatcher(i, { ...w, value: +e.target.value })} />

            <label className="text-[10px] text-overlay0"
              title="Deadband the value must clear before it can fire again in the other direction">
              hysteresis
            </label>
            <input type="number" step={0.05} min={0} className={`w-16 ${numCls}`}
              key={`${id}-${i}-hyst-${w.hysteresis}`} defaultValue={w.hysteresis}
              onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
              onBlur={e => setWatcher(i, { ...w, hysteresis: +e.target.value })} />

            <label className="text-[10px] text-overlay0">fires</label>
            <input type="text" className={`w-[90px] ${textCls}`}
              key={`${id}-${i}-fires-${w.fires}`} defaultValue={w.fires}
              placeholder="Custom event name"
              onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
              onBlur={e => setWatcher(i, { ...w, fires: e.target.value })} />

            <Checkbox
              label="disabled"
              checked={w.disabled}
              onCheckedChange={v => setWatcher(i, { ...w, disabled: v })}
            />
          </div>
        </div>
      ))}
      <button className="tb-btn text-[10px] px-2 py-0.5"
        onClick={() => send({ type: "AddWatcher", payload: { node_id: id } })}>
        + Add watcher
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Audio clip
// ---------------------------------------------------------------------------

/** The gain curve, mirroring `XrdsAudioDistanceModel::gain` in xrds-components.
 *  Kept in step with it by hand; if the Rust changes, change this too — a preview
 *  that lies is worse than no preview. */
function audioGain(model: string, d: number, min: number, max: number, rolloff: number): number {
  min = Math.max(min, 1e-6);
  rolloff = Math.max(rolloff, 0);
  if (d > max) return 0;
  if (d <= min) return 1;
  let g: number;
  if (model === "Linear")           g = 1 - rolloff * (d - min) / (max - min);
  else if (model === "Exponential") g = Math.pow(d / min, -rolloff);
  else                              g = min / (min + rolloff * (d - min));
  // Edge taper: the last half of the band fades out, so no model steps off a
  // cliff at max_distance.
  const band = (max - min) * 0.5;
  let t = band > 0 ? Math.min(Math.max((max - d) / band, 0), 1) : 1;
  t = t * t * (3 - 2 * t);
  return Math.min(Math.max(g * t, 0), 1);
}

/** Falloff curve plotted in **decibels**, not amplitude.
 *
 *  This is the whole point of the preview. An amplitude plot of `Linear` looks
 *  like a gentle straight line while sounding like a switch, because the ear
 *  hears dB: the last stretch runs -14 dB to silence. Two implementations were
 *  tuned and rejected by ear before that was understood — see
 *  docs/done/small-phases-plan.md S1. Plotting dB makes the audible shape visible. */
function FalloffCurve({ model, min, max, rolloff }:
  { model: string; min: number; max: number; rolloff: number }) {
  const W = 200, H = 64, FLOOR_DB = -40;
  const pts: string[] = [];
  for (let i = 0; i <= W; i++) {
    const d = (i / W) * max * 1.1;
    const g = audioGain(model, d, min, max, rolloff);
    const db = g > 0 ? Math.max(20 * Math.log10(g), FLOOR_DB) : FLOOR_DB;
    const y = H - ((db - FLOOR_DB) / -FLOOR_DB) * H;
    pts.push(`${i},${y.toFixed(1)}`);
  }
  const minX = (min / (max * 1.1)) * W;
  return (
    <div style={{ padding: "4px 10px 8px" }}>
      <svg width="100%" viewBox={`0 0 ${W} ${H}`} style={{ display: "block" }}>
        <rect x={0} y={0} width={W} height={H} fill="var(--mantle, #1e1e2e)" rx={3} />
        {/* Full-volume radius: everything left of this line is at 0 dB. */}
        <line x1={minX} y1={0} x2={minX} y2={H} stroke="var(--overlay0, #6c7086)"
          strokeWidth={1} strokeDasharray="3 3" />
        <polyline points={pts.join(" ")} fill="none" stroke="var(--blue, #89b4fa)" strokeWidth={1.5} />
      </svg>
      <div style={{ display:"flex", justifyContent:"space-between", fontSize:9, color:"var(--overlay0)" }}>
        <span>0 m</span><span>{FLOOR_DB} dB floor</span><span>{(max * 1.1).toFixed(0)} m</span>
      </div>
    </div>
  );
}

function AudioClipSection({ id, a, assets, send }: {
  id: number;
  a: Extract<NodePayload, { type: "AudioClip" }>;
  assets: AssetCatalogEntry[];
  send: (c: EditorCommand) => void;
}) {
  // Every field goes in one command so a slider drag is one undo entry.
  const commit = (patch: Partial<typeof a>) => send({
    type: "SetAudioClipParams",
    payload: {
      id,
      asset_id:       patch.asset_id       ?? a.asset_id,
      volume:         patch.volume         ?? a.volume,
      looped:         patch.looped         ?? a.looped,
      spatial:        patch.spatial        ?? a.spatial,
      autoplay:       patch.autoplay       ?? a.autoplay,
      distance_model: patch.distance_model ?? a.distance_model,
      min_distance:   patch.min_distance   ?? a.min_distance,
      max_distance:   patch.max_distance   ?? a.max_distance,
      rolloff_factor: patch.rolloff_factor ?? a.rolloff_factor,
    },
  });

  const audioAssets = assets.filter(x => x.kind === "Audio");

  return (
    <>
      <div className="insp-section">
        <h4>Audio</h4>
        {/* Audition. Before the runtime gained play/stop, `autoplay` was the only
            way a clip ever sounded, so checking a falloff meant exporting an APK
            and putting a headset on — about an hour per adjustment.
            Styled with `tb-btn` and the Toolbar's own ▶/■ glyphs so it reads as
            the same kind of control as the viewport's play button, rather than as
            two unstyled browser buttons. */}
        <div className="insp-row">
          <label>Preview</label>
          <div className="flex gap-1">
            <button className="tb-btn text-[11px] px-2 py-0.5"
              title="Play this clip from where it sits in the scene"
              onClick={() => send({ type: "PreviewAudioClip", payload: { id, playing: true } })}>
              ▶ Play
            </button>
            <button className="tb-btn text-[11px] px-2 py-0.5"
              title="Stop and rewind to the start"
              onClick={() => send({ type: "PreviewAudioClip", payload: { id, playing: false } })}>
              ■ Stop
            </button>
          </div>
        </div>
        <div className="insp-note">
          Heard from the editor camera through the clip's own falloff, so the curve
          below is what you are listening to.
        </div>
        {/* The shared Select, not a raw <select>: it punches its open list out of
            the Bevy viewport hole. A native dropdown here would be sliced off
            wherever it overlapped the 3D view — the same clipping that hid the
            APK dialog. */}
        <div className="insp-row">
          <label>Clip</label>
          <Select
            value={a.asset_id}
            onValueChange={v => commit({ asset_id: v })}
            options={audioAssets.map(x => ({ value: x.id, label: x.name }))}
            placeholder="No sound imported"
          />
        </div>
        <SliderRow label="Volume" value={a.volume} min={0} max={1} step={0.01}
          onLive={() => {}} onCommit={v => commit({ volume: v })} />
        <Toggle label="Loop"     value={a.looped}   onChange={v => commit({ looped: v })} />
        <Toggle label="Autoplay" value={a.autoplay} onChange={v => commit({ autoplay: v })} />
        <Toggle label="Spatial"  value={a.spatial}  onChange={v => commit({ spatial: v })} />
        {!a.spatial && (
          <div className="insp-note">
            Non-spatial: plays at the same volume everywhere, and the falloff below
            is ignored.
          </div>
        )}
      </div>

      {a.spatial && (
        <div className="insp-section">
          <h4>Distance falloff</h4>
          <FalloffCurve model={a.distance_model} min={a.min_distance}
            max={a.max_distance} rolloff={a.rolloff_factor} />
          <div className="insp-row">
            <label>Model</label>
            <Select
              value={a.distance_model}
              onValueChange={v => commit({ distance_model: v })}
              options={[
                { value: "Inverse",     label: "Inverse",     hint: "natural" },
                { value: "Linear",      label: "Linear" },
                { value: "Exponential", label: "Exponential" },
              ]}
            />
          </div>
          <SliderRow label="Full volume within" value={a.min_distance} min={0.1} max={20} step={0.1}
            onLive={() => {}} onCommit={v => commit({ min_distance: v })} />
          <SliderRow label="Silent beyond" value={a.max_distance} min={1} max={200} step={1}
            onLive={() => {}} onCommit={v => commit({ max_distance: v })} />
          <SliderRow label="Rolloff" value={a.rolloff_factor} min={0} max={5} step={0.1}
            onLive={() => {}} onCommit={v => commit({ rolloff_factor: v })} />
          {/* Named for what it does rather than what it is called in the payload:
              "min_distance" sounds like a floor, but it is the reference radius —
              raising it flattens the whole curve, and it is the first thing to
              reach for when a sound dies too fast. That cost real device time to
              work out. */}
          <div className="insp-note">
            If a sound fades too quickly, raise <b>Full volume within</b> before
            touching Rolloff — it sets the reference radius and gentles the whole
            curve, not just the near part.
          </div>
        </div>
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Scene environment (shown when nothing is selected)
// ---------------------------------------------------------------------------

function Toggle({ label, value, onChange, disabled }:
  { label: string; value: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <div className="insp-row">
      {/* Dimmed alongside the input so the row reads as unavailable rather than
          as a checkbox that ignores clicks. */}
      <label style={disabled ? { color: "var(--surface1)" } : undefined}>{label}</label>
      <input type="checkbox" checked={value} disabled={disabled}
        style={{ accentColor:"var(--blue)", cursor: disabled ? "default" : "pointer", width:16, height:16 }}
        onChange={e => onChange(e.target.checked)} />
    </div>
  );
}

function SceneEnvironmentSection({ env, passthrough, assets, send }:
  { env: EnvironmentDto | null; passthrough: boolean; assets: AssetCatalogEntry[];
    send: (c: EditorCommand) => void }) {
  // Both the skybox and IBL consume environment maps, so the filter is shared.
  const envMaps = assets.filter(a => a.kind === "EnvironmentMap");

  // IBL needs two *different* maps, and enabling with empty ids is refused by
  // document validation. Guessing by name makes the common case one click — the
  // SDK ships `diffuse.ktx2` and `specular.ktx2`, and anything produced by an IBL
  // baker is named the same way — while still being only a starting point the
  // author can change.
  const defaultIbl = {
    diffuse:  envMaps.find(a => /diffuse|irradiance/i.test(a.id))?.id  ?? envMaps[0]?.id ?? "",
    specular: envMaps.find(a => /specular|radiance/i.test(a.id))?.id ?? envMaps[1]?.id ?? envMaps[0]?.id ?? "",
  };
  // Bevy's default ev100 is 9.7 (outdoor daylight). We display exposure as a
  // "brightness" offset where 0 = Bevy default, positive = brighter, negative = darker.
  // Mapping: displayed_brightness = BEVY_EV100 - stored_ev100  →  brighter = lower ev100.
  const BEVY_EV100 = 9.7;
  const toBrightness = (ev: number) => BEVY_EV100 - ev;
  const toEv100 = (b: number) => BEVY_EV100 - b;

  const e = env ?? { fog_enabled:false, fog_color:[1,0.4,0.1,1] as [number,number,number,number],
                     fog_falloff:{ mode:"Linear", start:2, end:30 } as FogFalloff,
                     exposure_enabled:false, ev100:BEVY_EV100,
                     ibl_enabled:false, ibl_diffuse:"", ibl_specular:"", ibl_intensity:1000,
                     skybox_enabled:false, skybox_asset:"", skybox_brightness:1000, skybox_yaw_deg:0, atmosphere_enabled:false };

  const [fogColor, setFogColor]     = useState<[number,number,number,number]>(e.fog_color);
  const [fogFalloff, setFogFalloff] = useState<FogFalloff>(e.fog_falloff);
  /** One place that knows the shape of a fog write, so the colour row, the mode
   *  picker and four sliders cannot drift apart. */
  const sendFog = (color: [number,number,number,number], falloff: FogFalloff) =>
    send({ type:"SetFog", payload:{ color, falloff } });
  // brightness = BEVY_EV100 - ev100  (0 = default, positive = brighter)
  const [brightness, setBrightness] = useState(toBrightness(e.ev100));
  const isDragging = useRef(false);

  // Sync only when env CONTENT changes and user isn't dragging.
  // JSON.stringify prevents the effect from firing every 16 ms on the same values.
  const envKey = JSON.stringify(env);
  useEffect(() => {
    if (isDragging.current) return;
    if (env) {
      setFogColor(env.fog_color); setFogFalloff(env.fog_falloff);
      setBrightness(toBrightness(env.ev100));
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [envKey]);

  return (
    <>
      <div className="insp-empty" style={{ fontSize:10, color:"var(--overlay0)", padding:"8px 10px 2px" }}>
        Select a node to inspect it
      </div>

      {/* XR */}
      <div className="insp-section">
        <h4>XR</h4>
        <Toggle label="Passthrough" value={passthrough}
          onChange={enabled => send({ type:"SetXrPassthrough", payload:{ enabled } })} />
        {/* Said out loud because otherwise this looks broken: passthrough is a
            compositor layer on the headset, and the desktop viewport has no
            compositor. An author toggling it and seeing no change would
            reasonably conclude it does nothing. */}
        <div className="insp-note">
          Shows the real world behind the scene on an XR device. No effect in this
          viewport — the editor has no XR compositor.
        </div>
        {passthrough && (
          <>
            <div className="insp-note">
              Only visible where the scene is transparent. A ground plane or other
              full-coverage geometry will hide the room behind it.
            </div>
            {/* The skybox is not deletable geometry — it is a scene-environment
                setting attached to the camera — so it needs saying separately from
                "delete the opaque nodes". The runtime suppresses it rather than
                letting it silently paint over the real world. */}
            {e.skybox_enabled && (
              <div className="insp-note">
                The skybox below is <b>disabled on device</b> while passthrough is
                on — it would paint over the real world. Turn passthrough off to see
                it again.
              </div>
            )}
          </>
        )}
      </div>

      {/* Fog */}
      <div className="insp-section">
        <h4>Fog</h4>
        <Toggle label="Enable" value={e.fog_enabled}
          onChange={on => on
            ? send({ type:"SetFog", payload:{ color:fogColor, falloff:fogFalloff } })
            : send({ type:"ClearFog" })} />
        {e.fog_enabled && <>
          <ColorRow label="Color" color={fogColor}
            onLive={v  => { isDragging.current = true;  setFogColor(v); }}
            onCommit={v => { isDragging.current = false; setFogColor(v); sendFog(v, fogFalloff); }} />

          <div className="insp-row">
            <label>Falloff</label>
            <Select value={fogFalloff.mode} onValueChange={m => {
              // Each mode is authored with different numbers, so switching carries
              // a sensible default across rather than an empty field. Distances
              // are not interchangeable between them — a linear `end` is where fog
              // is total, a visibility is where it is merely thick.
              const next: FogFalloff =
                m === "Linear"             ? { mode:"Linear", start:2, end:30 }
              : m === "Exponential"        ? { mode:"Exponential", visibility:60 }
              :                              { mode:"ExponentialSquared", visibility:60 };
              setFogFalloff(next); sendFog(fogColor, next);
            }}
              options={[
                { value:"Linear",             label:"Linear" },
                { value:"Exponential",        label:"Exponential" },
                { value:"ExponentialSquared", label:"Exponential²" },
              ]} />
          </div>

          {fogFalloff.mode === "Linear" ? <>
            <SliderRow label="Start" value={fogFalloff.start} min={0} max={500} step={1}
              onLive={v  => { isDragging.current = true;  setFogFalloff({ ...fogFalloff, start:v }); }}
              onCommit={v => { isDragging.current = false; const f: FogFalloff = { ...fogFalloff, start:v }; setFogFalloff(f); sendFog(fogColor, f); }} />
            <SliderRow label="End" value={fogFalloff.end} min={1} max={2000} step={1}
              onLive={v  => { isDragging.current = true;  setFogFalloff({ ...fogFalloff, end:v }); }}
              onCommit={v => { isDragging.current = false; const f: FogFalloff = { ...fogFalloff, end:v }; setFogFalloff(f); sendFog(fogColor, f); }} />
            <div className="insp-note">
              Clear until Start, fully fogged at End. Not physical, but the only
              mode with a distance where fog is exactly absent — which is what you
              want when hiding a draw-distance boundary.
            </div>
          </> : <>
            <SliderRow label="Visibility" value={fogFalloff.visibility} min={1} max={2000} step={1}
              onLive={v  => { isDragging.current = true;  setFogFalloff({ ...fogFalloff, visibility:v }); }}
              onCommit={v => { isDragging.current = false; const f: FogFalloff = { ...fogFalloff, visibility:v }; setFogFalloff(f); sendFog(fogColor, f); }} />
            {/* Visibility rather than density, because a density is a number
              * nobody can picture. See XrdsSceneFogFalloff. */}
            <div className="insp-note">
              Roughly the distance at which things fade into the fog colour, in
              metres. {fogFalloff.mode === "Exponential"
                ? "Exponential is how real haze behaves — no clear zone, thickening steadily."
                : "Exponential² stays clearer up close, then thickens faster — a heavier bank of fog."}
            </div>
          </>}
        </>}
      </div>

      {/* Exposure */}
      <div className="insp-section">
        <h4>Exposure</h4>
        <Toggle label="Enable" value={e.exposure_enabled}
          onChange={on => on ? send({ type:"SetExposure", payload:{ ev100: toEv100(brightness) } }) : send({ type:"ClearExposure" })} />
        {e.exposure_enabled && (
          // Display as "Brightness" offset: 0 = Bevy default (ev100=9.7), +5 = brighter, -5 = darker
          <SliderRow label="Brightness" value={brightness} min={-5} max={5} step={0.1}
            onLive={v  => { isDragging.current = true;  setBrightness(v); }}
            onCommit={v => { isDragging.current = false; setBrightness(v); send({ type:"SetExposure", payload:{ ev100: toEv100(v) } }); }} />
        )}
      </div>

      {/* Atmosphere — a computed sky rather than an image.
          Placed above Skybox because for an outdoor scene it is usually the better
          answer: the sun comes from the scene's own directional light, so sky and
          shadows agree and moving the light moves the sun. A captured panorama
          cannot do that. See docs/done/editor-task-queue-and-hdr-conversion.md 0b. */}
      <div className="insp-section">
        <h4>Atmosphere <span style={{color:"var(--overlay0)",fontSize:9,fontWeight:"normal",marginLeft:4}}>procedural sky · desktop</span></h4>
        <Toggle label="Enable" value={e.atmosphere_enabled}
          onChange={enabled => send({ type:"SetAtmosphere", payload:{ enabled } })} />
        {e.atmosphere_enabled && (
          <>
            <div className="insp-note">
              The sun is your directional light — move it to change the time of day.
              With no directional light in the scene there is no sun, and the sky
              renders flat.
            </div>
            {/* The measured number, not a vague caution. It was "verify on device
                before relying on this" until the device answered: 13.0 ms -> 31.3 ms
                on a Quest 3, against a 13.9 ms budget at 72 Hz. An author deciding
                this deserves the figure, not an adjective. */}
            <div className="insp-note" style={{ color: "var(--yellow)" }}>
              <b>Costs ~18 ms/frame on a Quest 3</b> — more than the entire 13.9 ms
              budget at 72 Hz, measured. It renders correctly there, but halves the
              frame rate. Fine for desktop and desktop exports; avoid in scenes meant
              for a headset.
            </div>
            {passthrough && (
              <div className="insp-note">
                Suppressed on device while Passthrough is on — a computed sky would
                paint over the real world, exactly as a skybox would.
              </div>
            )}
          </>
        )}
      </div>

      {/* IBL.
          Had the same disease as the Skybox section: a toggle, and a note telling
          the author to "set diffuse/specular asset IDs" with no control to set them
          — while enabling with empty ids fails document validation
          (`EmptySceneIblAssetId`), so the checkbox silently refused to tick. */}
      <div className="insp-section">
        <h4>IBL <span style={{color:"var(--overlay0)",fontSize:9,fontWeight:"normal",marginLeft:4}}>image-based lighting</span></h4>
        <Toggle label="Enable" value={e.ibl_enabled} disabled={envMaps.length === 0}
          onChange={on => on
            ? send({ type:"SetIbl", payload:{
                diffuse_asset_id:  e.ibl_diffuse  || defaultIbl.diffuse,
                specular_asset_id: e.ibl_specular || defaultIbl.specular,
                intensity: e.ibl_intensity,
              } })
            : send({ type:"ClearIbl" })} />
        {envMaps.length === 0 ? (
          <div className="insp-note">
            Import environment maps first (Ctrl+I). IBL needs two: an irradiance map
            for diffuse light and a prefiltered map for reflections.
            <code>assets/environment_maps/</code> has both.
          </div>
        ) : e.ibl_enabled && (
          <>
            <div className="insp-row">
              <label>Diffuse</label>
              <Select
                value={e.ibl_diffuse}
                onValueChange={v => send({ type:"SetIbl", payload:{ diffuse_asset_id: v, specular_asset_id: e.ibl_specular, intensity: e.ibl_intensity } })}
                options={envMaps.map(x => ({ value: x.id, label: x.name }))}
                placeholder="Irradiance map"
              />
            </div>
            <div className="insp-row">
              <label>Specular</label>
              <Select
                value={e.ibl_specular}
                onValueChange={v => send({ type:"SetIbl", payload:{ diffuse_asset_id: e.ibl_diffuse, specular_asset_id: v, intensity: e.ibl_intensity } })}
                options={envMaps.map(x => ({ value: x.id, label: x.name }))}
                placeholder="Prefiltered map"
              />
            </div>
            <SliderRow label="Intensity" value={e.ibl_intensity} min={0} max={5000} step={50}
              onLive={() => {}}
              onCommit={v => send({ type:"SetIbl", payload:{ diffuse_asset_id: e.ibl_diffuse, specular_asset_id: e.ibl_specular, intensity: v } })} />
            {e.ibl_diffuse === e.ibl_specular && (
              <div className="insp-note">
                Both slots use the same map. They are meant to differ — the diffuse
                map is heavily blurred irradiance, the specular one a sharp chain
                prefiltered by roughness — so lighting will look wrong.
              </div>
            )}
          </>
        )}
      </div>

      {/* Skybox.
          The document, the runtime and the SetSkybox/ClearSkybox commands have all
          existed for a long time; only this section was missing, so the feature was
          unreachable and unknown. */}
      <div className="insp-section">
        <h4>Skybox</h4>
        {/* Disabled with no environment map, rather than left clickable.
            A skybox with an empty texture id fails document validation
            (`EmptySceneSkyboxAssetId`), so the click would be silently rejected and
            the checkbox would simply refuse to tick with no explanation — which is
            exactly how this was reported. */}
        <Toggle label="Enable" value={e.skybox_enabled} disabled={envMaps.length === 0}
          onChange={on => on
            ? send({ type:"SetSkybox", payload:{
                texture_asset_id: e.skybox_asset || envMaps[0]?.id || "",
                // No `|| 1000` fallback here: the default lives in the DTO
                // (`build_environment_dto`), where it is a single source of truth.
                // The fallback that used to be here never fired anyway — the DTO's
                // old default of 1.0 is truthy in JS — and 1 cd/m² renders as a
                // black sky, which is exactly how it was reported.
                brightness: e.skybox_brightness,
                yaw_deg: e.skybox_yaw_deg,
              } })
            : send({ type:"ClearSkybox" })} />
        {envMaps.length === 0 ? (
          <div className="insp-note">
            Import an environment map first (Ctrl+I) — a skybox needs a cubemap
            texture. <code>assets/environment_maps/specular.ktx2</code> ships with
            the SDK.
          </div>
        ) : e.skybox_enabled && (
          <>
            <div className="insp-row">
              <label>Texture</label>
              <Select
                value={e.skybox_asset}
                onValueChange={v => send({ type:"SetSkybox", payload:{ texture_asset_id: v, brightness: e.skybox_brightness, yaw_deg: e.skybox_yaw_deg } })}
                options={envMaps.map(x => ({ value: x.id, label: x.name }))}
                placeholder="Choose an environment map"
              />
            </div>
            {/* Absolute luminance in cd/m², judged against the camera's exposure —
                not a 0..1 factor. A sky at single digits is black, which is why the
                range starts where it does rather than at 0. */}
            <SliderRow label="Brightness" value={e.skybox_brightness} min={0} max={5000} step={50}
              onLive={() => {}}
              onCommit={v => send({ type:"SetSkybox", payload:{ texture_asset_id: e.skybox_asset, brightness: v, yaw_deg: e.skybox_yaw_deg } })} />
            {/* Turning the sky is how the sun gets placed: a cubemap arrives in
                whatever orientation it was captured, and this is the one adjustment
                an author actually makes to it. Live-updates rather than
                commit-only, because aiming a sun is something you do by eye. */}
            <SliderRow label="Rotation" value={e.skybox_yaw_deg} min={-180} max={180} step={1}
              onLive={v => send({ type:"SetSkybox", payload:{ texture_asset_id: e.skybox_asset, brightness: e.skybox_brightness, yaw_deg: v } })}
              onCommit={v => send({ type:"SetSkybox", payload:{ texture_asset_id: e.skybox_asset, brightness: e.skybox_brightness, yaw_deg: v } })} />
            {passthrough && (
              <div className="insp-note">
                Suppressed on device while Passthrough is on — see the XR section.
              </div>
            )}
          </>
        )}
      </div>
    </>
  );
}

function PayloadSection({ node, send, isPlaying, snapshot }: { node: NodeInspector; send: (c: EditorCommand) => void; isPlaying: boolean; snapshot: EditorSnapshot }) {
  const { id, payload } = node;

  // Capsule is the only primitive with an editor-side dimensions UI so far —
  // Cube/Sphere/Cylinder/Plane still only expose material + physics here;
  // their geometry is Rust-only via set_*_geometry. See
  // docs/adding-primitive-type.md §"There is currently no editor UI for a
  // primitive's own dimensions" for why this gap exists and how to close it
  // for another shape.
  if (payload.type === "Capsule") {
    return <>
      <CapsuleGeometrySection id={id} radius={payload.radius} length={payload.length} send={send} />
      <PrimitiveSection id={id} mat={payload.material} assets={snapshot.asset_catalog} physics_body={payload.physics_body} gravity_scale={payload.gravity_scale} mass={payload.mass} send={send} />
    </>;
  }
  if (payload.type === "Effect") {
    return <EffectParamsSection id={id} fx={payload} send={send} />;
  }
  if (payload.type === "AudioClip") {
    return <AudioClipSection id={id} a={payload} assets={snapshot.asset_catalog} send={send} />;
  }
  // Tetrahedron is mapped to Cube DTO on the Rust side
  if (payload.type === "Cube" || payload.type === "Sphere" || payload.type === "Cylinder" ||
      payload.type === "Plane") {
    return <PrimitiveSection id={id} mat={payload.material} assets={snapshot.asset_catalog} physics_body={payload.physics_body} gravity_scale={payload.gravity_scale} mass={payload.mass} send={send} />;
  }
  if (payload.type === "PointLight") {
    return <PointLightSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "DirectionalLight") {
    return <DirLightSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "AmbientLight") {
    return <AmbientSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "SpotLight") {
    return <SpotLightSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "Camera") {
    return <CameraSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "GltfAsset") {
    return <GltfSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "HudText") {
    return <HudTextSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "Text") {
    return <TextSection id={id} p={payload} parentKind={node.parent_kind} send={send} />;
  }
  if (payload.type === "ExtrudedText") {
    return <ExtrudedTextSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "Player") {
    return null;
  }
  if (payload.type === "PlayerAnchor") {
    return <PlayerAnchorSection id={id} p={payload} send={send} isPlaying={isPlaying} snapshot={snapshot} />;
  }
  if (payload.type === "PlayerSpawnZone") {
    return <SpawnZoneSection id={id} p={payload} send={send} snapshot={snapshot} />;
  }
  if (payload.type === "Panel") {
    return <PanelInstanceSection id={id} p={payload} send={send} snapshot={snapshot} />;
  }
  return null;
}

/** A scene-placed panel instance: which template, and nothing else.
 *
 * Deliberately thin. Size, background and elements all belong to the template —
 * putting any of them here would be the mistake `XrdsHudTemplate::depth` made in
 * reverse, with per-instance overrides quietly diverging from the shared
 * definition. Content is edited once, in the Panels workspace. */
function PanelInstanceSection({ id, p, send, snapshot }: {
  id: number;
  p: Extract<NodePayload, { type: "Panel" }>;
  send: (c: EditorCommand) => void;
  snapshot: EditorSnapshot;
}) {
  const templates = snapshot.panel_library;
  const current = templates.find(t => t.id === p.template_id);
  return (
    <div className="insp-section">
      <h4>Panel</h4>
      <div className="insp-row">
        <label>Template</label>
        <select value={current ? String(p.template_id) : ""}
          style={{ flex: 1, fontSize: 11 }}
          onChange={e => {
            if (e.target.value === "") return;
            send({ type: "SetPanelInstanceTemplate", payload: { id, template_id: Number(e.target.value) } });
          }}>
          {/* Shown only when the current id resolves to nothing, so the select has
            * something to display instead of silently snapping to another
            * template and hiding the problem. */}
          {!current && <option value="">— missing template {p.template_id} —</option>}
          {templates.map(t => (
            <option key={t.id} value={t.id}>{t.name}</option>
          ))}
        </select>
      </div>
      {!current ? (
        <div className="insp-note" style={{ color: "var(--red)" }}>
          ⚠ Template {p.template_id} is not in this document — nothing will spawn.
        </div>
      ) : (
        <div className="insp-note">
          {current.elements.length === 0
            ? "This template has no elements yet. Add some in the Panels workspace."
            : `${current.elements.length} element${current.elements.length === 1 ? "" : "s"}, ` +
              `${current.size[0]}×${current.size[1]}m. Appearance is edited in the Panels ` +
              `workspace and shared by every node using this template; the wiring below is ` +
              `this node's alone.`}
        </div>
      )}

      {/* Wiring is per-node, which is what lets floor 1's button open floor 1's
        * door while floor 3's opens its own. Authoring it beside the template
        * would make all instances share one target. */}
      <h4 style={{ marginTop: 10 }}>Wiring</h4>
      <PanelInstanceTriggers
        nodeId={id}
        elements={p.elements}
        snapshot={snapshot}
        send={send}
      />
    </div>
  );
}

const PHYSICS_BODY_OPTIONS = ["None", "Static", "Dynamic"] as const;

/** `radius`/`length` for a `Capsule` node — `length` excludes the two
 * hemispherical caps, matching `XrdsCapsule::length` and both Bevy's
 * `Capsule3d` and avian3d's `Collider::capsule`. Total visible extent is
 * `length + 2 * radius`.
 *
 * `SetCapsuleGeometry` doc-edits and live-previews on every drag frame —
 * same one-command shape as `SetGravityScale`/`SetMass` beside it, not the
 * separate live/commit split `SetMaterial`/`CommitMaterial` use. */
function CapsuleGeometrySection({ id, radius, length, send }: {
  id: number; radius: number; length: number; send: (c: EditorCommand) => void;
}) {
  return (
    <div className="insp-section">
      <h4>Geometry</h4>
      <SliderRow label="Radius" value={radius} min={0.01} max={5} step={0.01}
        onLive={v  => send({ type: "SetCapsuleGeometry", payload: { id, radius: v, length } })}
        onCommit={v => send({ type: "SetCapsuleGeometry", payload: { id, radius: v, length } })}
      />
      <SliderRow label="Length" value={length} min={0} max={10} step={0.01}
        onLive={v  => send({ type: "SetCapsuleGeometry", payload: { id, radius, length: v } })}
        onCommit={v => send({ type: "SetCapsuleGeometry", payload: { id, radius, length: v } })}
      />
      <div className="insp-note">
        Length excludes the rounded caps — total height is length + 2 × radius.
      </div>
    </div>
  );
}

function EffectParamsSection({ id, fx, send }: {
  id: number;
  fx: Extract<NodePayload, { type: "Effect" }>;
  send: (c: EditorCommand) => void;
}) {
  // Every edit re-sends the whole parameter set, matching SetEffectParams on the
  // Rust side. Keeps the backend free of partial-merge logic, at the cost of the
  // slightly verbose spread below.
  const push = (patch: Partial<Omit<typeof fx, "type">>) =>
    send({
      type: "SetEffectParams",
      payload: {
        id,
        kind: fx.kind,
        auto_play: fx.auto_play,
        burst_count: fx.burst_count,
        spawn_rate: fx.spawn_rate,
        lifetime_secs: fx.lifetime_secs,
        size_min: fx.size_min,
        size_max: fx.size_max,
        color_start: fx.color_start,
        color_end: fx.color_end,
        speed_min: fx.speed_min,
        speed_max: fx.speed_max,
        omnidirectional: fx.omnidirectional,
        spread_deg: fx.spread_deg,
        gravity: fx.gravity,
        emission_radius: fx.emission_radius,
        blend: fx.blend,
        size_end: fx.size_end,
        drag: fx.drag,
        fade_edge: fx.fade_edge,
        fade_scene: fx.fade_scene,
        ...patch,
      },
    });

  const isBurst = fx.kind === "Burst";

  return (
    <div className="insp-section">
      <h4>Effect</h4>

      <div className="insp-row">
        <span className="insp-label">Kind</span>
        <select value={fx.kind} onChange={e => push({ kind: e.target.value })}>
          <option value="Burst">Burst (one-shot)</option>
          <option value="Trail">Trail (continuous)</option>
        </select>
      </div>

      <div className="insp-row">
        <span className="insp-label">Auto Play</span>
        <input type="checkbox" checked={fx.auto_play}
          onChange={e => push({ auto_play: e.target.checked })} />
      </div>
      {isBurst && !fx.auto_play && (
        <div className="insp-note">
          Idle until fired by a Track — nothing is drawn in the viewport. This is
          the right setting for a trigger-driven burst.
        </div>
      )}
      {isBurst && fx.auto_play && (
        <div className="insp-note">
          Fires once when the scene loads and cannot be re-fired. Turn Auto Play
          off to drive it from a Track instead.
        </div>
      )}

      {/* Only the field the current kind actually reads is shown; the other is
          ignored by the runtime, and showing both invites tuning a dead value. */}
      {isBurst ? (
        <SliderRow label="Burst Count" value={fx.burst_count} min={1} max={2000} step={1}
          onLive={v => push({ burst_count: Math.round(v) })}
          onCommit={v => push({ burst_count: Math.round(v) })}
        />
      ) : (
        <SliderRow label="Rate (per sec)" value={fx.spawn_rate} min={1} max={1000} step={1}
          onLive={v => push({ spawn_rate: v })}
          onCommit={v => push({ spawn_rate: v })}
        />
      )}

      <SliderRow label="Lifetime (s)" value={fx.lifetime_secs} min={0.1} max={10} step={0.05}
        onLive={v => push({ lifetime_secs: v })}
        onCommit={v => push({ lifetime_secs: v })}
      />
      <SliderRow label="Size Min" value={fx.size_min} min={0.005} max={1} step={0.005}
        onLive={v => push({ size_min: v })}
        onCommit={v => push({ size_min: v })}
      />
      <SliderRow label="Size Max" value={fx.size_max} min={0.005} max={1} step={0.005}
        onLive={v => push({ size_max: v })}
        onCommit={v => push({ size_max: v })}
      />
      <SliderRow label="Speed Min" value={fx.speed_min} min={0} max={20} step={0.05}
        onLive={v => push({ speed_min: v })}
        onCommit={v => push({ speed_min: v })}
      />
      <SliderRow label="Speed Max" value={fx.speed_max} min={0} max={20} step={0.05}
        onLive={v => push({ speed_max: v })}
        onCommit={v => push({ speed_max: v })}
      />

      <div className="insp-row">
        <span className="insp-label">Omnidirectional</span>
        <input type="checkbox" checked={fx.omnidirectional}
          onChange={e => push({ omnidirectional: e.target.checked })} />
      </div>
      {!fx.omnidirectional && (
        <SliderRow label="Spread (°)" value={fx.spread_deg} min={0} max={179} step={1}
          onLive={v => push({ spread_deg: v })}
          onCommit={v => push({ spread_deg: v })}
        />
      )}

      <SliderRow label="Emit Radius" value={fx.emission_radius} min={0} max={5} step={0.01}
        onLive={v => push({ emission_radius: v })}
        onCommit={v => push({ emission_radius: v })}
      />
      <SliderRow label="Gravity Y" value={fx.gravity[1]} min={-20} max={20} step={0.1}
        onLive={v => push({ gravity: [fx.gravity[0], v, fx.gravity[2]] })}
        onCommit={v => push({ gravity: [fx.gravity[0], v, fx.gravity[2]] })}
      />

      <div className="insp-row">
        <span className="insp-label">Blend</span>
        {/* Left enabled but labelled as inert: the value is stored and travels
            correctly to the backend, which simply ignores it today (bevy_firework
            0.8 hardcodes alpha blending in its pipeline and its shader never reads
            the alpha_mode uniform). Verified on a Quest 3 — all three modes looked
            identical. Better to say so than to present a control that quietly
            does nothing, which is the same trap as the grabbable checkbox. */}
        <select value={fx.blend} onChange={e => push({ blend: e.target.value })}>
          <option value="Blend">Blend (normal)</option>
          <option value="Add">Add (glow)</option>
          <option value="Multiply">Multiply (darken)</option>
        </select>
      </div>
      <div className="insp-note">
        No visible effect yet — the particle backend ignores blend mode in its
        current version, so all three look the same. The setting is saved and will
        apply once that is fixed upstream.
      </div>

      <SliderRow label="End Size ×" value={fx.size_end} min={0} max={4} step={0.05}
        onLive={v => push({ size_end: v })}
        onCommit={v => push({ size_end: v })}
      />
      <SliderRow label="Drag" value={fx.drag} min={0} max={5} step={0.05}
        onLive={v => push({ drag: v })}
        onCommit={v => push({ drag: v })}
      />
      <SliderRow label="Edge Softness" value={fx.fade_edge} min={0} max={1} step={0.01}
        onLive={v => push({ fade_edge: v })}
        onCommit={v => push({ fade_edge: v })}
      />
      <SliderRow label="Scene Fade" value={fx.fade_scene} min={0} max={5} step={0.05}
        onLive={v => push({ fade_scene: v })}
        onCommit={v => push({ fade_scene: v })}
      />
      <div className="insp-note">
        End Size scales a particle over its life (1 = constant, 0 = shrink away).
        Drag settles motion — low for sparks, higher for smoke. Scene Fade softens
        where particles intersect geometry, removing the hard line where a plume
        meets the floor.
      </div>

      <div className="insp-row">
        <span className="insp-label">Start Colour</span>
        <input type="color" value={rgbaToHex(fx.color_start)}
          onChange={e => push({ color_start: hexToRgba(e.target.value, fx.color_start[3]) })} />
      </div>
      <div className="insp-row">
        <span className="insp-label">End Colour</span>
        <input type="color" value={rgbaToHex(fx.color_end)}
          onChange={e => push({ color_end: hexToRgba(e.target.value, fx.color_end[3]) })} />
      </div>
      <SliderRow label="End Alpha" value={fx.color_end[3]} min={0} max={1} step={0.01}
        onLive={v => push({ color_end: [fx.color_end[0], fx.color_end[1], fx.color_end[2], v] })}
        onCommit={v => push({ color_end: [fx.color_end[0], fx.color_end[1], fx.color_end[2], v] })}
      />
      <div className="insp-note">
        Particles fade from Start to End over their lifetime. An End Alpha of 0
        makes them fade out rather than pop. Colours are capped at full
        brightness — the XR cameras have no bloom pass, so brighter values would
        just render white.
      </div>
    </div>
  );
}

function PrimitiveSection({ id, mat, assets, physics_body, gravity_scale, mass, send }: { id: number; mat: MaterialParams; assets: AssetCatalogEntry[]; physics_body: string; gravity_scale: number; mass: number; send: (c: EditorCommand) => void }) {
  const [local, setLocal] = useState<MaterialParams>(mat);
  const isDragging = useRef(false);
  useEffect(() => { if (!isDragging.current) setLocal(mat); }, [mat]);
  const upd    = (m: MaterialParams) => { isDragging.current = true;  send({ type: "SetMaterial",   payload: { id, params: m } }); };
  const commit = (m: MaterialParams) => { isDragging.current = false; send({ type: "CommitMaterial", payload: { id, params: m } }); };
  const isDynamic = physics_body === "Dynamic";
  return (
    <div className="insp-section">
      <h4>Physics</h4>
      <div className="insp-row">
        <span className="insp-label">Body</span>
        <select value={physics_body} onChange={e => send({ type: "SetPhysicsBody", payload: { id, physics_body: e.target.value } })}>
          {PHYSICS_BODY_OPTIONS.map(o => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {isDynamic && <>
        <SliderRow label="Gravity Scale" value={gravity_scale} min={0} max={3} step={0.01}
          onLive={v  => send({ type: "SetGravityScale", payload: { id, value: v } })}
          onCommit={v => send({ type: "SetGravityScale", payload: { id, value: v } })}
        />
        <SliderRow label="Mass (kg)" value={mass} min={0.01} max={100} step={0.01}
          onLive={v  => send({ type: "SetMass", payload: { id, value: v } })}
          onCommit={v => send({ type: "SetMass", payload: { id, value: v } })}
        />
      </>}
      <h4>Material</h4>
      <ColorRow label="Base Color" color={local.base_color}
        onLive={c => { const m = {...local, base_color: c}; setLocal(m); upd(m); }}
        onCommit={c => { const m = {...local, base_color: c}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Metallic" value={local.metallic} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, metallic: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, metallic: v}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Roughness" value={local.roughness} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, roughness: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, roughness: v}; setLocal(m); commit(m); }}
      />
      <VideoSlotRow id={id} textures={mat.textures} assets={assets} send={send} />
      <TextureSlotRows id={id} textures={mat.textures} assets={assets} send={send} />
    </div>
  );
}

/** Texture-slot pickers for a node's authored material.
 *
 * Writes one slot at a time via `SetNodeMaterialTexture` rather than folding
 * into the `SetMaterial`/`CommitMaterial` params, so assigning a base-colour
 * map cannot drop an authored normal map — and so a colour *drag* never
 * carries texture data at all. Reads come from `mat.textures`, which the
 * snapshot keeps current.
 *
 * Shared by both material sections; `MaterialParams` is all it needs. */
/** VIDEO — a section of its own, above the texture slots.
 *
 * Technically a video *is* a texture: it fills a material slot, named by the same
 * asset id, and only its contents change. That is an implementation truth, not an
 * authoring one — nobody looking for "play a video on this screen" thinks to open
 * TEXTURES and read the base-colour dropdown. So the slots keep working as the
 * expert route, and this is the one an author finds.
 *
 * Writes to the base colour slot, which is what "show this video" means for a
 * surface; the tooltip says so, because it replaces whatever image was there.
 *
 * A clip already bound to another mesh is not offered: one video is one surface,
 * since two meshes would share a decoder and could only ever play in lockstep. */
function VideoSlotRow({ id, textures, assets, send }: {
  id: number;
  textures: MaterialTextures;
  assets: AssetCatalogEntry[];
  send: (c: EditorCommand) => void;
}) {
  const videos = assets.filter(a => a.kind === "Video");
  const boundHere = MATERIAL_TEXTURE_SLOTS
    .map(slot => textures[slot.key])
    .find(assetId => !!assetId && videos.some(v => v.id === assetId)) ?? null;

  // Nothing to offer and nothing bound: stay out of the way rather than showing an
  // empty control on every mesh in a scene with no video in it.
  if (videos.length === 0 && !boundHere) return null;

  const selectable = videos.filter(
    v => v.bound_to_node === null || v.bound_to_node === undefined || v.id === boundHere,
  );

  return (
    <>
      <div className="text-[10px] text-overlay0 mt-1 mb-0.5">VIDEO</div>
      <div className="flex items-center gap-1.5 mb-1">
        <label
          className="text-[10.5px] text-overlay0 w-[86px] shrink-0"
          title="Shows this clip on the surface. It fills the base colour slot, replacing whatever image was there while it plays."
        >
          Clip
        </label>
        <Select
          value={boundHere ?? TEXTURE_NONE_SENTINEL}
          onValueChange={v => send({
            type: "SetNodeMaterialTexture",
            payload: {
              id,
              slot: "BaseColor",
              texture_asset_id: v === TEXTURE_NONE_SENTINEL ? null : v,
            },
          })}
          options={[
            { value: TEXTURE_NONE_SENTINEL, label: "— none —" },
            ...selectable.map(v => ({ value: v.id, label: v.name })),
          ]}
        />
      </div>
      {boundHere && (
        <>
          <div className="flex items-center gap-1.5 mb-1">
            <label className="text-[10.5px] text-overlay0 w-[86px] shrink-0">Preview</label>
            <div className="flex gap-1">
              <button className="tb-btn text-[11px] px-2 py-0.5"
                title="Play this video on the surface"
                onClick={() => send({ type: "PreviewVideo", payload: { asset_id: boundHere, playing: true } })}>
                ▶ Play
              </button>
              <button className="tb-btn text-[11px] px-2 py-0.5"
                title="Stop; the last frame stays on the surface"
                onClick={() => send({ type: "PreviewVideo", payload: { asset_id: boundHere, playing: false } })}>
                ■ Stop
              </button>
            </div>
          </div>
          <div className="insp-note">
            Preview only. In a running scene a video never starts on its own — add a
            <b> PlayVideo</b> event in the Sequencer, since a decoder costs a thread
            or a hardware codec session and every frame of GPU work.
          </div>
        </>
      )}
    </>
  );
}

function TextureSlotRows({ id, textures, assets, send }: {
  id: number;
  textures: MaterialTextures;
  assets: AssetCatalogEntry[];
  send: (c: EditorCommand) => void;
}) {
  // Images only. A video *is* a texture underneath — same slot, same asset id —
  // but offering it here as well as in VIDEO states the same fact in two places,
  // and picking it in one silently changed the other. VIDEO is where a clip is
  // chosen; a slot it occupies shows as taken rather than editable.
  const textureAssets = assets.filter(a => a.kind === "Texture");
  const videoInSlot = (assetId: string | null | undefined): string | null =>
    assetId && assets.some(a => a.id === assetId && a.kind === "Video") ? assetId : null;
  return (
    <>
      <div className="text-[10px] text-overlay0 mt-1 mb-0.5">TEXTURES</div>
      {textureAssets.length === 0 && (
        <div className="text-[10px] text-yellow">
          ⚠ no texture assets imported yet
        </div>
      )}
      {MATERIAL_TEXTURE_SLOTS.map(slot => {
        // A slot a video is playing into is shown, not offered: editing it here
        // would silently unbind a clip from a section the author is not looking at.
        if (videoInSlot(textures[slot.key])) {
          return (
            <div key={slot.key} className="flex items-center gap-1.5 mb-1">
              <label className="text-[10.5px] text-overlay0 w-[86px] shrink-0">{slot.label}</label>
              <span className="text-[10.5px] text-overlay0 italic"
                title="This slot is showing a video. Change it in the VIDEO section above.">
                ▶ video — set in VIDEO
              </span>
            </div>
          );
        }
        return (
        <div key={slot.key} className="flex items-center gap-1.5 mb-1">
          <label className="text-[10.5px] text-overlay0 w-[86px] shrink-0">{slot.label}</label>
          <Select
            value={textures[slot.key] ?? TEXTURE_NONE_SENTINEL}
            onValueChange={v => send({
              type: "SetNodeMaterialTexture",
              payload: {
                id,
                slot: slot.wire,
                texture_asset_id: v === TEXTURE_NONE_SENTINEL ? null : v,
              },
            })}
            options={[
              { value: TEXTURE_NONE_SENTINEL, label: "— none —" },
              ...textureAssets.map(a => ({ value: a.id, label: a.name })),
            ]}
          />
        </div>
        );
      })}
    </>
  );
}

function MaterialSection({ id, mat, assets, send }: { id: number; mat: MaterialParams; assets: AssetCatalogEntry[]; send: (c: EditorCommand) => void }) {
  const [local, setLocal] = useState<MaterialParams>(mat);
  const isDragging = useRef(false);
  // Sync from snapshot only when not dragging (prevents overwrite during slider drag)
  useEffect(() => { if (!isDragging.current) setLocal(mat); }, [mat]);
  const upd    = (m: MaterialParams) => { isDragging.current = true;  send({ type: "SetMaterial",   payload: { id, params: m } }); };
  const commit = (m: MaterialParams) => { isDragging.current = false; send({ type: "CommitMaterial", payload: { id, params: m } }); };
  return (
    <div className="insp-section">
      <h4>Material</h4>
      <ColorRow label="Base Color" color={local.base_color}
        onLive={c => { const m = {...local, base_color: c}; setLocal(m); upd(m); }}
        onCommit={c => { const m = {...local, base_color: c}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Metallic" value={local.metallic} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, metallic: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, metallic: v}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Roughness" value={local.roughness} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, roughness: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, roughness: v}; setLocal(m); commit(m); }}
      />
      {/* Reads `mat`, not `local`: texture writes are their own command and
        * never part of the drag draft, so the snapshot is the truth. */}
      <VideoSlotRow id={id} textures={mat.textures} assets={assets} send={send} />
      <TextureSlotRows id={id} textures={mat.textures} assets={assets} send={send} />
    </div>
  );
}

function PointLightSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [intensity, setI] = useState(p.intensity); const [range, setR] = useState(p.range);
  const upd    = (color: any, i: number, r: number) => send({ type: "SetPointLight",  payload: { id, color, intensity: i, range: r } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Point Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v, intensity, range); }} onCommit={v => { setC(v); upd(v, intensity, range); commit(); }} />
      <SliderRow label="Intensity" value={intensity} min={0} max={100000} step={100} onLive={v => { setI(v); upd(c, v, range); }} onCommit={v => { setI(v); upd(c, v, range); commit(); }} />
      <SliderRow label="Range"     value={range}     min={0} max={100}    step={0.1} onLive={v => { setR(v); upd(c, intensity, v); }} onCommit={v => { setR(v); upd(c, intensity, v); commit(); }} />
      <div className="insp-note">ℹ Light visible when geometry is nearby.</div>
    </div>
  );
}

function DirLightSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [lux, setL] = useState(p.illuminance);
  const upd    = (color: any, illuminance: number) => send({ type: "SetDirectionalLight", payload: { id, color, illuminance } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Directional Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v, lux); }} onCommit={v => { setC(v); upd(v, lux); commit(); }} />
      <SliderRow label="Illuminance" value={lux} min={0} max={150000} step={500} onLive={v => { setL(v); upd(c, v); }} onCommit={v => { setL(v); upd(c, v); commit(); }} />
    </div>
  );
}

function AmbientSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [brightness, setB] = useState(p.brightness);
  const upd    = (color: any, b: number) => send({ type: "SetAmbientLight", payload: { id, color, brightness: b } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Ambient Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v, brightness); }} onCommit={v => { setC(v); upd(v, brightness); commit(); }} />
      <SliderRow label="Brightness" value={brightness} min={0} max={2000} step={10} onLive={v => { setB(v); upd(c, v); }} onCommit={v => { setB(v); upd(c, v); commit(); }} />
    </div>
  );
}

function SpotLightSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [i, setI] = useState(p.intensity);
  const [r, setR] = useState(p.range); const [inn, setInn] = useState(p.inner_angle); const [out, setOut] = useState(p.outer_angle);
  const upd    = (color: any, intensity: number, range: number, inner: number, outer: number) =>
    send({ type: "SetSpotLight", payload: { id, color, intensity, range, inner_angle: inner, outer_angle: outer } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Spot Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v,i,r,inn,out); }} onCommit={v => { setC(v); upd(v,i,r,inn,out); commit(); }} />
      <SliderRow label="Intensity" value={i} min={0} max={100000} step={100} onLive={v => { setI(v); upd(c,v,r,inn,out); }} onCommit={v => { setI(v); upd(c,v,r,inn,out); commit(); }} />
      <SliderRow label="Range"     value={r} min={0} max={100}    step={0.1} onLive={v => { setR(v); upd(c,i,v,inn,out); }} onCommit={v => { setR(v); upd(c,i,v,inn,out); commit(); }} />
    </div>
  );
}

function CameraSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [fov, setFov]   = useState<number>(p.fov);
  const [near, setNear] = useState<number>(p.near);
  const [far, setFar]   = useState<number>(p.far);

  useEffect(() => { setFov(p.fov); setNear(p.near); setFar(p.far); }, [p.fov, p.near, p.far]);

  const sendLive   = (f: number) => send({ type: "SetCameraParams",    payload: { id, fov: f, near, far } });
  const sendCommit = (f: number, n: number, fa: number) =>
    send({ type: "CommitCameraParams", payload: { id, fov: f, near: n, far: fa } });

  return (
    <div className="insp-section">
      <h4>Camera</h4>
      <SliderRow label="FOV"  value={fov}  min={10}  max={170} step={0.5}
        onLive={v => { setFov(v);  sendLive(v); }}
        onCommit={v => { setFov(v);  sendCommit(v, near, far); }} />
      <SliderRow label="Near" value={near} min={0.01} max={10}  step={0.01}
        onLive={v => { setNear(v); }}
        onCommit={v => { setNear(v); sendCommit(fov, v, far); }} />
      <SliderRow label="Far"  value={far}  min={10}  max={5000} step={1}
        onLive={v => { setFar(v); }}
        onCommit={v => { setFar(v);  sendCommit(fov, near, v); }} />
    </div>
  );
}

const HUD_ANCHORS = ["TopLeft","TopCenter","TopRight","MiddleLeft","Center","MiddleRight","BottomLeft","BottomCenter","BottomRight"];

function HudTextSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [text, setText]     = useState(p.text);
  const [size, setSize]     = useState(p.font_size);
  const [col, setCol]       = useState(p.color);
  const [anchor, setAnchor] = useState(p.anchor);
  const [offset, setOffset] = useState<[number,number]>(p.offset);

  const commit = () => send({ type: "SetHudText", payload: { id, text, font_size: size, color: col, anchor, offset } });

  return (
    <div className="insp-section">
      <h4>HUD Text <span style={{color:"var(--overlay0)", fontSize:9, fontWeight:"normal", marginLeft:4}}>screen-space</span></h4>

      <div className="insp-row">
        <label>Text</label>
        <input type="text" value={text} className="full-input"
          onKeyDown={e => e.stopPropagation()}
          onChange={e => setText(e.target.value)}
          onBlur={commit}
          onKeyUp={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
      </div>

      <SliderRow label="Font Size" value={size} min={8} max={96} step={1}
        onLive={v => setSize(v)} onCommit={v => { setSize(v); commit(); }} />

      <ColorRow label="Color" color={col}
        onLive={v => setCol(v)} onCommit={v => { setCol(v); commit(); }} />

      <div className="insp-row">
        <label>Anchor</label>
        <select value={anchor} className="full-input"
          onChange={e => {
            const a = e.target.value;
            setAnchor(a);
            // Send immediately with the new value — React state is async so
            // commit() would still see the old anchor if called here.
            send({ type: "SetHudText", payload: { id, text, font_size: size, color: col, anchor: a, offset } });
          }}>
          {HUD_ANCHORS.map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>

      {/* Pixel offset from anchor */}
      <div style={{ marginTop: 4 }}>
        <div className="tf-row">
          <span className="insp-tf-lbl" style={{ width: 28, color:"var(--overlay0)", fontSize:10 }}>Off</span>
          {(["ax-x","ax-y"] as const).map((ax, i) => (
            <ScrubField key={ax} axis={ax === "ax-x" ? "x" : "y"} value={offset[i]} step={1}
              onLive={v => { const o: [number,number] = [...offset]; o[i] = v; setOffset(o); }}
              onCommit={v => { const o: [number,number] = [...offset]; o[i] = v; setOffset(o); commit(); }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function GltfSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const clips: { index: number; name: string }[] = p.clips ?? [];
  const [clipIndex, setClipIndex] = useState(0);
  const [speed, setSpeed] = useState(1.0);
  return (
    <div className="insp-section">
      <h4>GLTF Asset</h4>
      {clips.length > 0 ? (
        <div className="insp-row">
          <label>Clip</label>
          <select value={clipIndex} onChange={e => setClipIndex(+e.target.value)}>
            {clips.map(cl => <option key={cl.index} value={cl.index}>{cl.name || `Clip ${cl.index}`}</option>)}
          </select>
        </div>
      ) : (
        <div className="insp-row"><label>Clip</label><span style={{color:"var(--overlay0)", fontSize:11}}>No clips</span></div>
      )}
      <SliderRow label="Speed" value={speed} min={0.1} max={4} step={0.1} onLive={v => setSpeed(v)} onCommit={v => setSpeed(v)} />
      <div style={{ display:"flex", gap:6, marginTop:4 }}>
        <button className="tb-btn" onClick={() => send({ type:"PlayGltfAnimation", payload:{ id, clip_index: clipIndex, speed, repeat:"Loop" } })}>▶ Play</button>
        <button className="tb-btn" onClick={() => send({ type:"StopGltfAnimation", payload:{ id } })}>■ Stop</button>
      </div>
    </div>
  );
}

function collectByKind(nodes: import("../types/bridge").HierarchyNode[], kind: string): { id: number; name: string }[] {
  const result: { id: number; name: string }[] = [];
  for (const n of nodes) {
    if (n.kind === kind) result.push({ id: n.id, name: n.name });
    result.push(...collectByKind(n.children, kind));
  }
  return result;
}

function SpawnZoneSection({ id, p, send, snapshot }: { id: number; p: any; send: (c: EditorCommand) => void; snapshot: EditorSnapshot }) {
  const [size, setSize] = useState<[number, number, number]>(p.size ?? [4.0, 0.1, 4.0]);
  useEffect(() => { setSize(p.size ?? [4.0, 0.1, 4.0]); }, [p.size]);

  const playerNodeId: number | null = p.player_node_id ?? null;
  const playerNodes = collectByKind(snapshot.hierarchy, "Player");

  const commit = (s: [number, number, number]) =>
    send({ type: "SetSpawnZoneSize", payload: { id, size: s } });

  return (
    <div className="insp-section">
      <h4>Spawn Zone <span style={{ color: "var(--overlay0)", fontSize: 9, fontWeight: "normal", marginLeft: 4 }}>W × H × D (metres)</span></h4>
      <SliderRow label="Width"  value={size[0]} min={0.1} max={50} step={0.1}
        onLive={v  => setSize([v, size[1], size[2]])}
        onCommit={v => { const s: [number,number,number] = [v, size[1], size[2]]; setSize(s); commit(s); }} />
      <SliderRow label="Height" value={size[1]} min={0.01} max={10} step={0.01}
        onLive={v  => setSize([size[0], v, size[2]])}
        onCommit={v => { const s: [number,number,number] = [size[0], v, size[2]]; setSize(s); commit(s); }} />
      <SliderRow label="Depth"  value={size[2]} min={0.1} max={50} step={0.1}
        onLive={v  => setSize([size[0], size[1], v])}
        onCommit={v => { const s: [number,number,number] = [size[0], size[1], v]; setSize(s); commit(s); }} />

      <div className="insp-row" style={{ marginTop: 6 }}>
        <label>Player</label>
        <select
          value={playerNodeId ?? ""}
          style={{ flex: 1, fontSize: 11 }}
          onChange={e => {
            const val = e.target.value;
            send({ type: "SetSpawnZonePlayer", payload: { id, player_node_id: val === "" ? null : Number(val) } });
          }}
        >
          <option value="">— shared (any player) —</option>
          {playerNodes.map(p => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      </div>
      {playerNodeId !== null && playerNodes.find(p => p.id === playerNodeId) == null && (
        <div className="insp-note" style={{ color: "var(--red)" }}>
          ⚠ Designated player node was deleted or renamed
        </div>
      )}
      <div className="insp-note">
        Players teleport to a random XZ position within this box on load.
      </div>
    </div>
  );
}

function PlayerAnchorSection({ id, p, send, isPlaying, snapshot }: { id: number; p: any; send: (c: EditorCommand) => void; isPlaying: boolean; snapshot: EditorSnapshot }) {
  const BEVY_EV100 = 9.7;
  const toBrightness = (ev: number) => BEVY_EV100 - ev;
  const toEv100 = (b: number) => BEVY_EV100 - b;

  const [fov, setFov] = useState<number>(p.fov_deg ?? 60);
  // exposure: null = inherit scene-wide; number = override (stored as ev100)
  const [expEnabled, setExpEnabled] = useState<boolean>(p.exposure != null);
  const [brightness, setBrightness] = useState<number>(p.exposure != null ? toBrightness(p.exposure) : 0);

  useEffect(() => { setFov(p.fov_deg ?? 60); }, [p.fov_deg]);
  useEffect(() => {
    setExpEnabled(p.exposure != null);
    setBrightness(p.exposure != null ? toBrightness(p.exposure) : 0);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.exposure]);

  const templates = snapshot.panel_library;

  return (
    <div className="insp-section">
      <h4>Player Anchor</h4>
      <SliderRow label="FOV (°)" value={fov} min={10} max={170} step={1}
        disabled={isPlaying}
        onLive={v => { if (!isPlaying) { setFov(v); send({ type: "SetPlayerAnchorFov", payload: { id, fov_deg: v } }); } }}
        onCommit={v => { if (!isPlaying) { setFov(v); send({ type: "SetPlayerAnchorFov", payload: { id, fov_deg: v } }); } }} />
      {isPlaying && (
        <div style={{ fontSize: 10, color: "var(--overlay0)", marginBottom: 4 }}>
          FOV applies on next anchor switch
        </div>
      )}
      <Toggle label="Initial spawn anchor" value={p.is_initial ?? false}
        onChange={v => send({ type: "SetPlayerAnchorInitial", payload: { id, is_initial: v } })} />

      {/* Per-anchor exposure override */}
      <div style={{ marginTop: 8 }}>
        <Toggle label="Override exposure" value={expEnabled}
          onChange={on => {
            setExpEnabled(on);
            send({ type: "SetPlayerAnchorExposure", payload: { id, ev100: on ? toEv100(brightness) : null } });
          }} />
        {expEnabled && (
          <SliderRow label="Brightness" value={brightness} min={-5} max={5} step={0.1}
            onLive={v  => setBrightness(v)}
            onCommit={v => { setBrightness(v); send({ type: "SetPlayerAnchorExposure", payload: { id, ev100: toEv100(v) } }); }} />
        )}
        {expEnabled && (
          <div className="insp-note">Overrides scene exposure when this anchor is active.</div>
        )}
      </div>

      {/* Head-locked panels are **parented**, not linked: a child Panel node
        * carries its own element wiring and a full transform. */}
      <div className="insp-row" style={{ marginTop: 8 }}>
        <label>Head-locked Panel</label>
        <button className="tb-btn" style={{ flex: 1, fontSize: 11 }}
          disabled={templates.length === 0}
          title={templates.length === 0
            ? "Create a panel template in the Panels workspace first"
            : "Adds a Panel node under this anchor — head-locked, and wirable in its own Inspector"}
          onClick={() => send({
            type: "SpawnPrimitive",
            payload: { kind: "Panel", parent_id: id },
          })}>
          + Add panel child
        </button>
      </div>
      <div className="insp-note">
        A Panel node under this anchor is head-locked. Its own transform sets where
        it sits in front of the lens, and its Inspector is where its buttons are
        wired.
      </div>
    </div>
  );
}

const TEXT_ANCHORS = ["World","Billboard","HeadLocked","BodyLocked","ComfortPinned","Cylindrical"];
const CAMERA_RELATIVE_ANCHORS = new Set(["HeadLocked","BodyLocked","ComfortPinned","Cylindrical"]);

function TextSection({ id, p, parentKind, send }: { id: number; p: any; parentKind?: string | null; send: (c: EditorCommand) => void }) {
  const [text, setText]     = useState(p.text);
  const [size, setSize]     = useState(p.font_size);
  const [col, setCol]       = useState(p.color);
  const [align, setAlign]   = useState(p.alignment);
  const [anchor, setAnchor] = useState<string>(p.anchor ?? "World");
  const [anchorParam, setAnchorParam] = useState<number>(p.anchor_param ?? 1.0);

  const commit = (overrides?: Partial<{ align: string; anchor: string; anchor_param: number }>) =>
    send({ type: "SetTextContent", payload: {
      id, text, font_size: size, color: col,
      alignment:    overrides?.align        ?? align,
      anchor:       overrides?.anchor       ?? anchor,
      anchor_param: overrides?.anchor_param ?? anchorParam,
    }});

  return (
    <div className="insp-section">
      <h4>Text</h4>
      <div className="insp-row"><label>Text</label>
        <input type="text" value={text}
          onChange={e => setText(e.target.value)} onBlur={() => commit()}
          onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
      </div>
      <SliderRow label="Font Size" value={size} min={4} max={200} step={0.5} onLive={v => setSize(v)} onCommit={v => { setSize(v); commit(); }} />
      <ColorRow label="Color" color={col} onLive={v => setCol(v)} onCommit={v => { setCol(v); commit(); }} />
      <div className="insp-row"><label>Align</label>
        <select value={align} onChange={e => {
          const a = e.target.value; setAlign(a); commit({ align: a });
        }}>
          {["Left","Center","Right"].map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
      <div className="insp-row"><label>Anchor</label>
        <select value={anchor} onChange={e => {
          const a = e.target.value;
          // When switching to a param-bearing anchor with no prior param, seed a visible default.
          const needsParam = a === "ComfortPinned" || a === "Cylindrical";
          const newParam = needsParam && anchorParam < 0.05 ? 1.0 : anchorParam;
          setAnchor(a);
          if (newParam !== anchorParam) setAnchorParam(newParam);
          commit({ anchor: a, anchor_param: newParam });
        }}>
          {TEXT_ANCHORS.map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
      {(anchor === "ComfortPinned") && (
        <SliderRow label="Depth (m)" value={anchorParam} min={0.1} max={10} step={0.05}
          onLive={v => setAnchorParam(v)}
          onCommit={v => { setAnchorParam(v); commit({ anchor_param: v }); }} />
      )}
      {(anchor === "Cylindrical") && (
        <SliderRow label="Radius (m)" value={anchorParam} min={0.1} max={10} step={0.05}
          onLive={v => setAnchorParam(v)}
          onCommit={v => { setAnchorParam(v); commit({ anchor_param: v }); }} />
      )}
      {CAMERA_RELATIVE_ANCHORS.has(anchor) && parentKind === "Player" && (
        <div style={{ marginTop:6, padding:"5px 7px", background:"rgba(250,179,135,0.12)",
                      border:"1px solid var(--peach)", borderRadius:3, fontSize:11,
                      color:"var(--peach)", lineHeight:1.4 }}>
          ⚠ Camera-relative text must be under a <strong>PlayerAnchor</strong>, not Player.
          Move this node under a PlayerAnchor child — it will not follow the camera at runtime.
        </div>
      )}
    </div>
  );
}

function ExtrudedTextSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [text, setText] = useState(p.text); const [size, setSize] = useState(p.font_size);
  const [depth, setDepth] = useState(p.depth); const [col, setCol] = useState(p.color);
  const [align, setAlign] = useState(p.alignment);
  const commit = () => send({ type: "SetExtrudedText", payload: { id, text, font_size: size, depth, color: col, alignment: align } });
  return (
    <div className="insp-section">
      <h4>Extruded Text</h4>
      <div className="insp-row"><label>Text</label>
        <input type="text" value={text}
          onChange={e => setText(e.target.value)} onBlur={commit}
          onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
      </div>
      <SliderRow label="Font Size" value={size}  min={4}   max={200} step={0.5}  onLive={v => setSize(v)}  onCommit={v => { setSize(v);  commit(); }} />
      <SliderRow label="Depth"     value={depth} min={0.01} max={5}  step={0.05} onLive={v => setDepth(v)} onCommit={v => { setDepth(v); commit(); }} />
      <ColorRow label="Color" color={col}
        onLive={v => { setCol(v); send({ type: "SetExtrudedTextColor", payload: { id, color: v } }); }}
        onCommit={v => { setCol(v); commit(); }}
      />
      <div className="insp-row"><label>Align</label>
        <select value={align} onChange={e => {
          const a = e.target.value; setAlign(a);
          send({ type:"SetExtrudedText", payload:{ id, text, font_size:size, depth, color:col, alignment:a } });
        }}>
          {["Left","Center","Right"].map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
    </div>
  );
}
