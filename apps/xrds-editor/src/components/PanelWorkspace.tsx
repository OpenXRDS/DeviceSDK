import { useEffect, useRef, useState } from "react";
import type { EditorCommand, EditorSnapshot, PanelElementDto, PanelTemplateDto } from "../types/bridge";
import { elementKindName, elementRowLabel, validKindsForElement } from "../lib/sequencer";
import { rgbaToHex, hexToRgba } from "../types/bridge";
import { widgetFields, fieldValue, withField, withVec2Component, movedTo, alphaOf } from "../lib/panelWidget";
import type { FieldSpec } from "../lib/panelWidget";
import { Select } from "./ui/Select";
import { Checkbox } from "./ui/Checkbox";

/** Element kinds an author can add, in picker order. */
const ELEMENT_KINDS = ["Label", "Button", "Image", "Slider", "Toggle"] as const;

const ADD_ELEMENT_SENTINEL = "__add_element__";

/** Canvas pixels per metre. A panel is authored in metres (0.6 × 0.4 is a
 *  typical size), so the canvas needs a scale to be workable on screen. */
const PX_PER_METRE = 520;

/**
 * The Panels workspace — a focused, 2D view for designing reusable panel
 * templates. Third layout alongside Scene and Sequencer.
 *
 * **It hides the 3D viewport rather than resizing it**, which is what makes it
 * "focused" and is the real difference from the Sequencer. Panel design is a 2D
 * task, so a live viewport would contribute nothing and would leave a
 * click-through hole under this UI — the editor punches a real hole in its own
 * window for Bevy, so a full-screen React layout has to close it. Handled by the
 * `set_viewport_hole` effect below, the same mechanism the modal overlays use.
 *
 * Layout follows the Sequencer's language (docs/Sequencer_Editor.dc.html):
 * a library list on the left, the thing being edited in the middle, an inspector
 * on the right.
 */
export function PanelWorkspace({ snapshot, send }: {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}) {
  const [openId, setOpenId] = useState<number | null>(null);
  const [selectedElement, setSelectedElement] = useState<string | null>(null);

  // Close the Bevy viewport hole for as long as this workspace is mounted.
  //
  // Without this the hole stays open behind a full-screen layout, and clicks
  // meant for the canvas land on the 3D scene instead — the same class of bug as
  // an absolutely-positioned overlay swallowing input.
  useEffect(() => {
    const ipc = (window as any).ipc;
    ipc?.postMessage(JSON.stringify({ type: "set_viewport_hole", enabled: false }));
    return () => {
      ipc?.postMessage(JSON.stringify({ type: "set_viewport_hole", enabled: true }));
    };
  }, []);

  const open = snapshot.panel_library.find(t => t.id === openId) ?? null;

  // Follow the document: a template deleted elsewhere must not leave this
  // workspace pointing at a ghost.
  useEffect(() => {
    if (openId !== null && !snapshot.panel_library.some(t => t.id === openId)) {
      setOpenId(null);
      setSelectedElement(null);
    }
  }, [snapshot.panel_library, openId]);

  useEffect(() => setSelectedElement(null), [openId]);

  const element = open?.elements.find(e => e.name === selectedElement) ?? null;

  return (
    <div className="panel-ws">
      <PanelLibrary
        templates={snapshot.panel_library}
        openId={openId}
        onOpen={setOpenId}
        send={send}
      />

      {open === null ? (
        <div className="panel-ws-empty">
          <div className="text-[12.5px] text-subtext0">No panel open</div>
          <div className="text-[11px] text-overlay0">
            Pick one from the <strong>Panels</strong> list, or create one. A panel is a
            reusable template — place it in the scene with a Panel node, or head-lock it
            to a Player Anchor. The only difference is where it ends up.
          </div>
        </div>
      ) : (
        <>
          <ElementList
            template={open}
            selected={selectedElement}
            onSelect={setSelectedElement}
            send={send}
          />
          <PanelCanvas
            template={open}
            selected={selectedElement}
            onSelect={setSelectedElement}
            send={send}
          />
          <ElementInspector
            template={open}
            element={element}
            snapshot={snapshot}
            send={send}
          />
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

function PanelLibrary({ templates, openId, onOpen, send }: {
  templates: PanelTemplateDto[];
  openId: number | null;
  onOpen: (id: number) => void;
  send: (cmd: EditorCommand) => void;
}) {
  const [renaming, setRenaming] = useState<number | null>(null);
  const [draft, setDraft] = useState("");

  function commitRename(id: number) {
    const name = draft.trim();
    if (name) send({ type: "RenamePanelTemplate", payload: { id, name } });
    setRenaming(null);
  }

  return (
    <div className="panel-ws-library">
      <div className="seq-ws-col-head">
        <span className="seq-caption">PANELS</span>
        <span className="flex-1" />
        <button className="tb-btn text-[11px]"
          title="Create a panel template"
          onClick={() => send({ type: "CreatePanelTemplate", payload: { name: "Panel" } })}>
          + New
        </button>
      </div>
      <div className="panel-ws-library-list">
        {templates.length === 0 && (
          <div className="seq-list-empty">
            No panels yet. A panel is authored once and instanced wherever it is needed.
          </div>
        )}
        {templates.map(t => (
          <div key={t.id}
            className={`seq-list-row${openId === t.id ? " active" : ""}`}
            onClick={() => onOpen(t.id)}>
            {renaming === t.id ? (
              <input autoFocus value={draft}
                className="text-[12.5px] text-bright bg-well rounded px-2 py-0.5 border border-surface0 focus:outline focus:outline-1 focus:outline-blue"
                onKeyDown={e => {
                  e.stopPropagation();
                  if (e.key === "Enter") commitRename(t.id);
                  if (e.key === "Escape") setRenaming(null);
                }}
                onChange={e => setDraft(e.target.value)}
                onBlur={() => commitRename(t.id)} />
            ) : (
              <div className="flex flex-col min-w-0 gap-px"
                onDoubleClick={() => { setDraft(t.name); setRenaming(t.id); }}>
                <span className="text-[12.5px] truncate" title="Double-click to rename">
                  {t.name}
                </span>
                <span className="seq-list-row-meta">
                  {t.elements.length} element{t.elements.length === 1 ? "" : "s"}
                  {" · "}
                  {t.size[0]}×{t.size[1]}m
                </span>
              </div>
            )}
            <span className="flex-1" />
            <button className="seq-list-row-del"
              title="Delete this panel template"
              onClick={e => {
                e.stopPropagation();
                if (confirm(`Delete panel "${t.name}"? Anchors linking it will be cleared.`)) {
                  send({ type: "DeletePanelTemplate", payload: { id: t.id } });
                }
              }}>✕</button>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Element list
// ---------------------------------------------------------------------------

function ElementList({ template, selected, onSelect, send }: {
  template: PanelTemplateDto;
  selected: string | null;
  onSelect: (name: string | null) => void;
  send: (cmd: EditorCommand) => void;
}) {
  /** Names an added element can take without colliding. */
  function freshName(kind: string): string {
    const base = kind.toLowerCase();
    let n = 1;
    while (template.elements.some(e => e.name === `${base}${n}`)) n += 1;
    return `${base}${n}`;
  }

  return (
    <div className="panel-ws-elements">
      <div className="seq-ws-col-head">
        <span className="seq-caption">ELEMENTS</span>
        <span className="flex-1" />
        <Select
          value={ADD_ELEMENT_SENTINEL}
          onValueChange={kind => {
            if (kind === ADD_ELEMENT_SENTINEL) return;
            send({
              type: "AddPanelElement",
              payload: { template_id: template.id, kind, name: freshName(kind) },
            });
          }}
          options={[
            { value: ADD_ELEMENT_SENTINEL, label: "+ Element…" },
            ...ELEMENT_KINDS.map(k => ({ value: k, label: k })),
          ]}
        />
      </div>
      <div className="panel-ws-element-list">
        {template.elements.length === 0 && (
          <div className="seq-list-empty">
            No elements yet — add one above, then wire its triggers on the right.
          </div>
        )}
        {template.elements.map(e => {
          const label = elementRowLabel(e);
          return (
            <div key={e.name}
              className={`seq-ws-track-row${selected === e.name ? " active" : ""}`}
              onClick={() => onSelect(e.name)}>
              <span className="seq-dot" style={{ background: kindColor(e) }} />
              <div className="flex flex-col min-w-0 gap-px">
                <span className="text-[11.5px] truncate">{label.title}</span>
                <span className="text-[9px] text-overlay0 font-mono truncate">{label.sub}</span>
              </div>
              <span className="flex-1" />
              <button className="seq-list-row-del"
                title="Remove this element"
                onClick={ev => {
                  ev.stopPropagation();
                  send({
                    type: "RemovePanelElement",
                    payload: { template_id: template.id, name: e.name },
                  });
                  if (selected === e.name) onSelect(null);
                }}>✕</button>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** One hue per element kind, so the list and canvas agree at a glance. */
function kindColor(e: PanelElementDto): string {
  switch (e.widget.type) {
    case "Button": return "var(--blue)";
    case "Slider": return "var(--teal)";
    case "Toggle": return "var(--green)";
    case "Image": return "var(--flamingo)";
    default: return "var(--mauve)";
  }
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

/** A widget's authored footprint in metres, for drawing it to scale. */
function widgetSize(e: PanelElementDto): [number, number] {
  const w = e.widget;
  switch (w.type) {
    case "Label": return w.layout_size;
    case "Button": return w.size;
    case "Image": return w.size;
    case "Slider": return w.size;
    case "Toggle": return w.size;
  }
}

function widgetPosition(e: PanelElementDto): [number, number] {
  return e.widget.local_position;
}

/** Pixels of cursor travel before a press becomes a drag rather than a click.
 *  Matches `WorldPanelCanvasOverlay`, so selecting an element feels the same in
 *  both canvases. */
const DRAG_THRESHOLD_PX = 4;

/**
 * The panel canvas — elements drawn to scale, draggable to reposition.
 *
 * Drag follows `WorldPanelCanvasOverlay`'s pattern rather than sending a command
 * per pointer-move: the moving position is local React state, and exactly one
 * `SetPanelElementWidget` lands on pointer-up. That is not just about traffic —
 * the editor has an undo stack, and per-move commands would bury the author's
 * previous action under a hundred one-pixel nudges.
 */
function PanelCanvas({ template, selected, onSelect, send }: {
  template: PanelTemplateDto;
  selected: string | null;
  onSelect: (name: string) => void;
  send: (cmd: EditorCommand) => void;
}) {
  const [w, h] = template.size;
  const wrapRef = useRef<HTMLDivElement>(null);

  // The position being dragged, overriding the snapshot until it commits.
  const [draft, setDraft] = useState<{ name: string; pos: [number, number] } | null>(null);
  const dragRef = useRef<{
    name: string;
    startPx: number; startPy: number;
    startWx: number; startWy: number;
    moved: boolean;
    last: [number, number] | null;
  } | null>(null);

  const onPointerDown = (e: React.PointerEvent, el: PanelElementDto) => {
    e.stopPropagation();
    const [wx, wy] = el.widget.local_position;
    dragRef.current = {
      name: el.name,
      startPx: e.clientX, startPy: e.clientY,
      startWx: wx, startWy: wy,
      moved: false, last: null,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent, el: PanelElementDto) => {
    const ds = dragRef.current;
    if (!ds || ds.name !== el.name) return;
    const dpx = e.clientX - ds.startPx;
    const dpy = e.clientY - ds.startPy;
    if (!ds.moved && Math.abs(dpx) < DRAG_THRESHOLD_PX && Math.abs(dpy) < DRAG_THRESHOLD_PX) {
      return;
    }
    ds.moved = true;
    // Canvas Y is up, screen Y is down.
    const pos: [number, number] = [
      ds.startWx + dpx / PX_PER_METRE,
      ds.startWy - dpy / PX_PER_METRE,
    ];
    ds.last = pos;
    setDraft({ name: el.name, pos });
  };

  const onPointerUp = (_e: React.PointerEvent, el: PanelElementDto) => {
    const ds = dragRef.current;
    dragRef.current = null;
    if (!ds || ds.name !== el.name) return;
    if (ds.moved && ds.last) {
      send({
        type: "SetPanelElementWidget",
        payload: {
          template_id: template.id,
          name: el.name,
          widget: movedTo(el.widget, ds.last[0], ds.last[1]),
        },
      });
      // Cleared only after the command goes out; clearing first would snap the
      // element back to its old spot for the frame before the snapshot returns.
      setDraft(null);
    } else {
      onSelect(el.name);
    }
  };

  return (
    <div className="panel-ws-canvas" ref={wrapRef}>
      <div className="seq-ws-col-head">
        <span className="seq-caption">CANVAS</span>
        <span className="text-[10px] text-overlay0 font-mono ml-2">
          {w}×{h}m
        </span>
        <span className="flex-1" />
        <span className="text-[10px] text-overlay0">
          positions are authored in metres from the centre
        </span>
      </div>
      <div className="panel-ws-canvas-area">
        <div
          className="panel-ws-canvas-plane"
          style={{
            width: w * PX_PER_METRE,
            height: h * PX_PER_METRE,
            background: rgba(template.color, template.opacity),
            borderRadius: template.corner_radius * PX_PER_METRE,
          }}
        >
          {/* Centre crosshair: local_position is measured from here, so without
            * it the coordinates on the right have no visible origin. */}
          <div className="panel-ws-axis panel-ws-axis-x" />
          <div className="panel-ws-axis panel-ws-axis-y" />

          {template.elements.map(e => {
            const [ex, ey] = draft?.name === e.name ? draft.pos : widgetPosition(e);
            const [ew, eh] = widgetSize(e);
            return (
              <button key={e.name}
                className={`panel-ws-el${selected === e.name ? " selected" : ""}`}
                style={{
                  // Canvas Y is up, screen Y is down. `.panel-ws-el` supplies
                  // the translate(-50%,-50%) that centres the box on
                  // `local_position`, matching what the runtime does.
                  left: `calc(50% + ${ex * PX_PER_METRE}px)`,
                  top: `calc(50% - ${ey * PX_PER_METRE}px)`,
                  width: Math.max(ew * PX_PER_METRE, 8),
                  height: Math.max(eh * PX_PER_METRE, 8),
                  ["--k" as string]: kindColor(e),
                }}
                title={`${e.name} — ${elementKindName(e)} — drag to move`}
                onPointerDown={ev => onPointerDown(ev, e)}
                onPointerMove={ev => onPointerMove(ev, e)}
                onPointerUp={ev => onPointerUp(ev, e)}>
                <span className="panel-ws-el-label">{e.name}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function rgba(c: [number, number, number, number], opacity: number): string {
  const [r, g, b, a] = c;
  const to255 = (v: number) => Math.round(Math.max(0, Math.min(1, v)) * 255);
  return `rgba(${to255(r)}, ${to255(g)}, ${to255(b)}, ${a * opacity})`;
}

// ---------------------------------------------------------------------------
// Element inspector
// ---------------------------------------------------------------------------

function ElementInspector({ template, element, snapshot, send }: {
  template: PanelTemplateDto;
  element: PanelElementDto | null;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}) {
  const [renameDraft, setRenameDraft] = useState("");
  useEffect(() => setRenameDraft(element?.name ?? ""), [element?.name]);

  // Diagnostics are matched by the quoted names in their prose, so the panel
  // name has to be part of the test — two panels may each have a "Go" button,
  // and showing one's warnings on the other sends the author to the wrong file.
  // Case-insensitive because some details open with "Panel" mid-sentence.
  const diagnostics = element === null ? [] : snapshot.panel_diagnostics.filter(d => {
    // "element named X" is the duplicate-name error's phrasing; without this it
    // is the one element problem the inspector would never show.
    const detail = d.detail.toLowerCase().replace("element named ", "element ");
    return detail.includes(`element ${JSON.stringify(element.name).toLowerCase()}`)
      && detail.includes(`panel ${JSON.stringify(template.name).toLowerCase()}`);
  });

  return (
    <div className="panel-ws-inspector">
      <div className="seq-ws-col-head">
        <span className="text-[11.5px] font-semibold text-text">Inspector</span>
        <span className="seq-tag">ELEMENT</span>
        <span className="flex-1" />
        <span className="text-[9.5px] text-overlay0 font-mono">
          {element === null ? "none selected" : elementKindName(element)}
        </span>
      </div>

      {element === null ? (
        <div className="seq-list-empty">Select an element to edit it.</div>
      ) : (
        <div className="panel-ws-inspector-body">
          <div className="flex flex-col gap-1.5">
            <div className="seq-field-label">NAME</div>
            <input value={renameDraft}
              className="text-[11.5px] text-bright bg-well rounded px-2 py-1 border border-surface0 focus:outline focus:outline-1 focus:outline-blue"
              onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
              onChange={e => setRenameDraft(e.target.value)}
              onBlur={() => {
                const next = renameDraft.trim();
                if (next && next !== element.name) {
                  send({
                    type: "RenamePanelElement",
                    payload: { template_id: template.id, name: element.name, new_name: next },
                  });
                }
              }} />
            <span className="text-[10px] text-overlay0">
              The addressing key — Tracks and <code>set_hud_item</code> find this element by name.
            </span>
          </div>

          <ElementProperties template={template} element={element} send={send} />

          {/* Trigger wiring is not authored here: it belongs to each placed
            * Panel node, since two instances drive different targets. Select the
            * node in the Scene workspace to wire it. */}
          <div className="insp-note">
            Wiring lives on each placed Panel node — select one in the Scene
            workspace to bind its elements to Tracks.
          </div>

          {diagnostics.length > 0 && (
            <div className="flex flex-col gap-0.5 mt-1">
              {diagnostics.map((d, i) => (
                <span key={i}
                  className={`text-[11px] ${d.severity === "error" ? "text-red" : "text-yellow"}`}
                  title={d.detail}>
                  ⚠ {d.title}
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * An element's trigger bindings — the payoff of the whole panel-template plan.
 *
 * The kind picker is driven by `validKindsForElement`, i.e. by the
 * `emittable_triggers` the Rust side computed. That is why a Label offers
 * nothing here instead of offering `ButtonPress` and silently never firing.
 */
// ---------------------------------------------------------------------------
// Element properties
// ---------------------------------------------------------------------------

/**
 * Per-kind widget properties, generated from {@link widgetFields}.
 *
 * Generated rather than five hand-written forms because the five `WorldWidget`
 * variants overlap almost entirely, and a field missing from a bespoke form is
 * an unauthorable property with no error anywhere to point at it. The field
 * tables are asserted against the DTO in `panelWidget.test.ts`.
 *
 * This is what closes the gap that blocked retiring `XrdsHudItemDef`: a HUD text
 * item is a `Label` element, and a Label whose `text` cannot be set is not a
 * replacement for one.
 */
function ElementProperties({ template, element, send }: {
  template: PanelTemplateDto;
  element: PanelElementDto;
  send: (cmd: EditorCommand) => void;
}) {
  const set = (widget: PanelElementDto["widget"]) =>
    send({
      type: "SetPanelElementWidget",
      payload: { template_id: template.id, name: element.name, widget },
    });

  return (
    <div className="flex flex-col gap-2">
      <div className="seq-caption">{elementKindName(element).toUpperCase()} PROPERTIES</div>
      {widgetFields(element.widget).map(spec => (
        <WidgetField key={spec.key} spec={spec} element={element} onChange={set} />
      ))}
    </div>
  );
}

function WidgetField({ spec, element, onChange }: {
  spec: FieldSpec;
  element: PanelElementDto;
  onChange: (w: PanelElementDto["widget"]) => void;
}) {
  const w = element.widget;
  const value = fieldValue(w, spec.key);
  const num = "text-[11.5px] text-bright bg-well rounded px-2 py-1 border border-surface0 w-full focus:outline focus:outline-1 focus:outline-blue";
  // Text inputs must not let keystrokes reach the editor's global shortcut
  // handler, or typing a label name starts triggering commands.
  const stop = (e: React.KeyboardEvent) => e.stopPropagation();

  return (
    <div className="flex flex-col gap-1">
      <div className="seq-field-label">{spec.label}</div>

      {spec.kind === "text" && (
        <input className={num} type="text" value={String(value ?? "")}
          placeholder={spec.placeholder}
          onKeyDown={stop}
          onChange={e => onChange(withField(w, spec.key, e.target.value))} />
      )}

      {spec.kind === "number" && (
        <input className={num} type="number" value={Number(value ?? 0)}
          step={spec.step} min={spec.min}
          onKeyDown={stop}
          onChange={e => {
            // An empty or half-typed box parses as NaN, which would serialise
            // as null and be rejected by the Rust side — keep the old value.
            const n = e.target.valueAsNumber;
            if (!Number.isNaN(n)) onChange(withField(w, spec.key, n));
          }} />
      )}

      {spec.kind === "bool" && (
        <Checkbox checked={Boolean(value)}
          onCheckedChange={v => onChange(withField(w, spec.key, v === true))} />
      )}

      {spec.kind === "color" && (
        <input type="color" value={rgbaToHex(value as [number, number, number, number])}
          onChange={e =>
            // Alpha is preserved explicitly: `<input type="color">` is RGB-only,
            // so rebuilding the colour would otherwise force every translucent
            // element opaque the first time it is touched.
            onChange(withField(w, spec.key, hexToRgba(e.target.value, alphaOf(w, spec.key))))
          } />
      )}

      {spec.kind === "vec2" && (
        <div className="flex gap-1.5">
          {([0, 1] as const).map(i => (
            <input key={i} className={num} type="number" step={0.01}
              value={(value as [number, number] | undefined)?.[i] ?? 0}
              onKeyDown={stop}
              onChange={e => {
                const n = e.target.valueAsNumber;
                if (!Number.isNaN(n)) onChange(withVec2Component(w, spec.key, i, n));
              }} />
          ))}
        </div>
      )}
    </div>
  );
}


// `ElementTriggers` lived here. Trigger wiring moved to the scene Inspector, on
// each placed Panel node — see `PanelInstanceTriggers.tsx`. A template is a
// design (what the panel looks like); what its buttons *do* differs per place it
// is used, which is why the two cannot share one editor.
