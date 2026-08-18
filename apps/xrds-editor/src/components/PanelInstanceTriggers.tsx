/** Trigger wiring for a placed Panel node, one section per element.
 *
 * **This lives on the node, not the template**, and that is the whole point.
 * When bindings were authored in the Panels workspace they belonged to the
 * template, so one elevator panel instanced on three floors gave every floor's
 * button the same Track and therefore the same door — and nothing could express
 * "my door", because `XrdsActionTarget::TriggerSource` resolves to the button
 * that fired, not to anything near it. Wiring per instance makes that
 * unrepresentable rather than merely diagnosed.
 *
 * The cost, stated plainly for anyone reading this later: twenty instances wired
 * the same way is twenty sets of bindings. That is accepted — it is what buys
 * each instance its own target.
 */

import type {
  EditorCommand,
  EditorSnapshot,
  PanelInstanceElementDto,
  TriggerEffect,
} from "../types/bridge";
import { HAND_ANY_SENTINEL, TRACK_NONE_SENTINEL, TRIGGER_EFFECTS } from "../types/bridge";
import { Select } from "./ui/Select";
import { Checkbox } from "./ui/Checkbox";

export function PanelInstanceTriggers({ nodeId, elements, snapshot, send }: {
  nodeId: number;
  elements: PanelInstanceElementDto[];
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}) {
  if (elements.length === 0) {
    return (
      <div className="insp-note">
        This template has no elements. Add some in the Panels workspace, then wire
        them here.
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {elements.map(el => (
        <ElementWiring key={el.name} nodeId={nodeId} el={el} snapshot={snapshot} send={send} />
      ))}
    </div>
  );
}

function ElementWiring({ nodeId, el, snapshot, send }: {
  nodeId: number;
  el: PanelInstanceElementDto;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}) {
  const kinds = el.emittable_triggers;
  const canEmit = kinds.length > 0;

  return (
    <div className="flex flex-col gap-1.5">
      {/* The name is shown verbatim — not upper-cased, not letter-spaced. It is
        * authored text that may be any case and any script, so mangling it makes
        * the row harder to match against the element list, which shows the real
        * name. The kind is a chip because it is a fixed vocabulary of five and
        * reads faster as a tag than as parenthetical prose. */}
      <div className="panel-el-heading">
        <span className="panel-el-heading-name" title={el.name}>{el.name}</span>
        <span className="panel-el-heading-kind">{el.kind}</span>
        {!canEmit && !el.orphaned && (
          <span className="panel-el-heading-note">emits nothing</span>
        )}
        {el.orphaned && (
          <span className="panel-el-heading-note" style={{ color: "var(--red)" }}>missing</span>
        )}
      </div>

      {/* An orphaned row exists only because the saved document still has wiring
        * for a name the template no longer defines. Shown rather than hidden: the
        * bindings are real and recoverable, and hiding them would make the file
        * disagree with the UI. */}
      {el.orphaned && (
        <div className="text-[11px] text-red">
          ⚠ the template has no element named {JSON.stringify(el.name)} any more.
          These bindings will never fire. Re-add an element with this name to
          recover them, or remove them below.
        </div>
      )}

      {el.triggers.length === 0 && !el.orphaned && (
        <div className="text-[11px] text-overlay0">
          {canEmit
            ? "Nothing bound. A binding fires a Track when this element is used."
            : `Only Buttons, Sliders and Toggles emit anything — a ${el.kind} is display-only.`}
        </div>
      )}

      {el.triggers.map((b, i) => {
        const kindUnavailable = !kinds.includes(b.trigger.kind);
        return (
          <div key={i} className="hud-library-row flex-col items-stretch gap-1">
            <div className="flex items-center gap-1.5 flex-wrap">
              <Select
                value={b.trigger.kind}
                onValueChange={kind => send({
                  type: "SetPanelNodeTriggerKind",
                  payload: { id: nodeId, element: el.name, index: i, trigger: { kind } as any },
                })}
                // The current kind is kept in the list even when unavailable, so
                // an existing binding is never silently rewritten to something
                // else just by rendering the picker.
                options={(kindUnavailable ? [b.trigger.kind, ...kinds] : kinds)
                  .map(k => ({ value: k, label: k }))}
              />
              <span className="flex-1" />
              <Checkbox label="disabled" checked={b.disabled}
                onCheckedChange={disabled => send({
                  type: "SetPanelNodeTriggerDisabled",
                  payload: { id: nodeId, element: el.name, index: i, disabled },
                })} />
              <button className="tb-btn text-red text-[11px]"
                title="Remove this binding"
                onClick={() => send({
                  type: "RemovePanelNodeTrigger",
                  payload: { id: nodeId, element: el.name, index: i },
                })}>✕</button>
            </div>

            {kindUnavailable && !el.orphaned && (
              <span className="text-[11px] text-yellow">
                ⚠ a {el.kind} never emits {b.trigger.kind} — this can never fire
              </span>
            )}

            <div className="flex items-center gap-1.5 flex-wrap">
              {/* Fire/Stop sits beside the Track picker because the two read as
                * one sentence: "Stop → Open". A stop button is the reason this
                * exists; a running Track otherwise cannot be interrupted from
                * authored content at all. */}
              <label className="text-[10.5px] text-overlay0 w-10">
                {b.effect === "Stop" ? "Stops" : "Fires"}
              </label>
              <Select
                value={b.effect}
                onValueChange={v => send({
                  type: "SetPanelNodeTriggerEffect",
                  payload: { id: nodeId, element: el.name, index: i, effect: v as TriggerEffect },
                })}
                options={TRIGGER_EFFECTS.map(e => ({ value: e, label: e }))}
              />
              <Select
                value={b.track ?? TRACK_NONE_SENTINEL}
                onValueChange={v => send({
                  type: "SetPanelNodeTriggerTrack",
                  payload: {
                    id: nodeId, element: el.name, index: i,
                    track: v === TRACK_NONE_SENTINEL ? null : v,
                  },
                })}
                options={[
                  { value: TRACK_NONE_SENTINEL, label: "— nothing —" },
                  ...snapshot.tracks.map(t => ({ value: t.name, label: t.name })),
                ]}
              />
              {b.track === null && (
                <span className="text-[11px] text-yellow">⚠ fires nothing yet</span>
              )}
            </div>

            <div className="flex items-center gap-1.5 flex-wrap">
              <label className="text-[10.5px] text-overlay0 w-10">Hand</label>
              <Select
                value={b.hand ?? HAND_ANY_SENTINEL}
                onValueChange={v => send({
                  type: "SetPanelNodeTriggerHand",
                  payload: {
                    id: nodeId, element: el.name, index: i,
                    hand: v === HAND_ANY_SENTINEL ? null : v,
                  },
                })}
                options={[
                  { value: HAND_ANY_SENTINEL, label: "any hand" },
                  { value: "Left", label: "Left" },
                  { value: "Right", label: "Right" },
                ]}
              />
            </div>
          </div>
        );
      })}

      {!el.orphaned && (
        <button className="tb-btn text-[11px] px-2 py-0.5 self-start"
          disabled={!canEmit}
          title={canEmit
            ? "Add a trigger binding for this element on this node"
            : `A ${el.kind} emits nothing, so a binding could never fire`}
          onClick={() => send({
            type: "AddPanelNodeTrigger",
            payload: { id: nodeId, element: el.name },
          })}>
          + Add binding
        </button>
      )}
    </div>
  );
}
