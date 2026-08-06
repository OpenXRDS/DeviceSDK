// Pure, framework-free derivations for the Sequencer — see
// docs/xrds-track-model-plan.md.
//
// Note what is *absent*: there is no lane derivation any more. A Track's rows
// come straight from `NamedTrackDto.assets`, because a row is now a real
// authored thing (one asset, one row) rather than something the editor
// inferred by grouping actions by category. That grouping was the original
// bug — for a registry timeline every action collapsed into a single "Self"
// lane, since no key carried any node identity to group by.
import type {
  ActionTarget, EditorSnapshot, NamedTrackDto, NodeBindingSummary, NodeInspector,
  XrdsAction, XrdsTrackAssetDto, XrdsTrackKeyDto,
} from "../types/bridge";

// ---------------------------------------------------------------------------
// Ruler tick math
// ---------------------------------------------------------------------------

/** Picks a "nice" ruler interval (1/2/5 × 10ⁿ) giving roughly `targetTicks`
 * major gridlines across `duration`. Without this a 2.4 s Track and a 300 s
 * one would get the same arbitrary divisions. */
export function niceStep(duration: number, targetTicks = 8): number {
  if (!(duration > 0)) return 1;
  const raw = duration / targetTicks;
  const mag = Math.pow(10, Math.floor(Math.log10(raw)));
  const norm = raw / mag;
  const mult = norm <= 1 ? 1 : norm <= 2 ? 2 : norm <= 5 ? 5 : 10;
  return mult * mag;
}

/** Ruler tick positions from 0 to `duration` inclusive, at `niceStep`
 * spacing. Rounded to kill float drift so events land on their labels. */
export function rulerTicks(duration: number, targetTicks = 8): number[] {
  const step = niceStep(duration, targetTicks);
  const out: number[] = [];
  for (let t = 0; t <= duration + 1e-9; t += step) out.push(+t.toFixed(6));
  return out;
}

/** `m:ss` timecode, keeping sub-second precision only when the value needs
 * it (so a 5 s step reads "0:05", not "0:05.00"). */
export function fmtTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs - m * 60;
  const whole = s % 1 === 0;
  return `${m}:${s.toFixed(whole ? 0 : 2).padStart(whole ? 2 : 5, "0")}`;
}

// ---------------------------------------------------------------------------
// Asset rows
// ---------------------------------------------------------------------------

/** How long this action itself takes. Non-zero only for `SetTransform` — every
 * other action applies instantly, including glTF playback, which is
 * deliberately fire-and-forget.
 *
 * Mirrors `XrdsAction::self_duration_secs` in Rust: both the ruler span and the
 * dot-vs-bar decision read this, so they cannot disagree about what "instant"
 * means. A `SetTransform` with duration 0 is what the deleted `Teleport` action
 * was, and renders as a dot rather than a bar for exactly that reason. */
export function actionDuration(a: XrdsAction): number {
  return a.kind === "SetTransform" ? Math.max(0, a.data.duration_secs) : 0;
}

// ---------------------------------------------------------------------------
// Lane row layout
//
// This whole block used to live inline in `SequencerWorkspace.tsx`, and it
// shipped visibly broken twice from there — both times in the *wiring*
// (measurement never arriving, then a 0 flowing into a divisor-like role),
// never in `stackKeys` itself, which was correct and unit-tested all along.
// It is pure and lives here now so the numbers the component paints can be
// asserted directly, without a DOM.
// ---------------------------------------------------------------------------

/** Base row height, matching a single-lane row's old fixed height so a row
 * with no overlap looks exactly as it did before stacking existed. */
export const ROW_H = 44;

/** Added per *extra* sub-lane. Deliberately much smaller than `ROW_H`:
 * fitting one more 12px dot should not double a 44px row. */
export const SUBLANE_STEP = 18;

/** A dot's on-screen diameter (`.seq-key`'s width/height in editor.css) plus
 * a little breathing room. */
export const DOT_FOOTPRINT_PX = 16;

export interface RowLayout {
  /** Row height in px — what both the asset-name column and the lane column
   * must use, or the two independently-scrolled columns drift apart. */
  height: number;
  /** Sub-lane index per key, in the input's own order. */
  lanes: number[];
  /** Sub-lane count. Carried explicitly rather than recovered by inverting
   * the `height` arithmetic — that inverse breaks silently the moment
   * `ROW_H`/`SUBLANE_STEP` change. */
  count: number;
}

/** Full vertical layout for one asset row: how tall it must be, and which
 * sub-lane each event sits in.
 *
 * `lanePx` may be 0 (before the first layout measurement) — the footprint
 * fallback below is what keeps that from silently disabling stacking.
 */
export function layoutAssetRow(
  keys: Pick<XrdsTrackKeyDto, "at_secs" | "action">[],
  lanePx: number,
  durationSecs: number,
): RowLayout {
  const { lanes, count } = stackKeys(keys, dotFootprintSecs(DOT_FOOTPRINT_PX, lanePx, durationSecs));
  return { height: ROW_H + (count - 1) * SUBLANE_STEP, lanes, count };
}

/** Vertical centre for a key in sub-lane `lane`, as a px offset within the
 * row. `.seq-key` is `translate(…, -50%)`, so this is a centre, not a top
 * edge — lanes come out evenly distributed across the row's height. */
export function keyTopPx(lane: number, count: number, height: number): number {
  return ((lane + 0.5) / Math.max(count, 1)) * height;
}

/** A dot's minimum footprint in *seconds*, converted from its fixed
 * on-screen pixel size via how wide the lane currently renders.
 *
 * **Never returns 0**, which is the whole reason this is a named function
 * with its own test. A zero footprint silently disables stacking
 * altogether: it makes a dot's interval degenerate (`[at, at]`), and
 * `stackKeys` treats touching endpoints as non-overlapping, so even two
 * dots at the *exact same timestamp* would share a lane. `lanePx` is 0
 * before the first layout measurement, so the fallback is load-bearing,
 * not defensive padding. */
export function dotFootprintSecs(dotPx: number, lanePx: number, durationSecs: number): number {
  const FALLBACK_LANE_PX = 800;
  const width = lanePx > 0 ? lanePx : FALLBACK_LANE_PX;
  return (dotPx / width) * Math.max(durationSecs, 0.0001);
}

/** Greedy interval-scheduling layout for one row's keys, so events that
 * overlap in time stack into separate vertical sub-lanes instead of
 * literally rendering on top of each other.
 *
 * `minSeconds` is every key's minimum *visual* footprint — a dot has zero
 * authored duration but still occupies real pixels on screen, and two dots
 * a fraction of a second apart still look merged even though their
 * intervals `[at, at]`/`[at, at]` don't technically overlap. Callers derive
 * it from the lane's actual rendered pixel width (dot diameter ÷ px-per-
 * second), so "close enough to stack" tracks the current zoom level rather
 * than a fixed time window that would be wrong at any other duration.
 *
 * First-fit, not earliest-fit: each key takes the lowest-numbered lane
 * whose last-placed key already ends by this key's start. That's enough for
 * a *valid* non-overlapping packing (not necessarily the minimum lane
 * count some other packing could achieve, though for typical sparse
 * authoring the two coincide) — lanes are a display concern, not data, so
 * there's nothing to keep "optimal" for. Returns lane indices in `keys`'
 * original order, plus how many lanes were used (the row's required
 * height is `count`, however this is rendered). */
export function stackKeys(
  keys: Pick<XrdsTrackKeyDto, "at_secs" | "action">[],
  minSeconds: number,
): { lanes: number[]; count: number } {
  const spans = keys
    .map((k, i) => ({ i, start: k.at_secs, end: k.at_secs + Math.max(actionDuration(k.action), minSeconds) }))
    .sort((a, b) => a.start - b.start || a.i - b.i);

  const laneEnds: number[] = [];
  const lanes = new Array<number>(keys.length).fill(0);
  for (const span of spans) {
    const EPS = 1e-9; // touching endpoints are not an overlap
    let lane = laneEnds.findIndex(end => end <= span.start + EPS);
    if (lane === -1) {
      lane = laneEnds.length;
      laneEnds.push(span.end);
    } else {
      laneEnds[lane] = span.end;
    }
    lanes[span.i] = lane;
  }
  return { lanes, count: Math.max(1, laneEnds.length) };
}

/** The mockup's two-line row label: a title plus a quieter qualifier.
 *
 * A `Node` row is titled with the node's name (resolved server-side into
 * `node_name`). `SelfNode`/`TriggerSource` rows have no concrete node until
 * the Track is fired, so they say so rather than inventing a name. A `Node`
 * row whose `node_name` is null points at a deleted node — separately
 * diagnosed, but labelled here so it is visibly wrong rather than blank. */
export function assetRowLabel(asset: XrdsTrackAssetDto): { title: string; sub: string } {
  switch (asset.target.type) {
    case "Node":
      return asset.node_name === null
        ? { title: `Node #${asset.target.id}`, sub: "missing — deleted?" }
        : { title: asset.node_name, sub: `node #${asset.target.id}` };
    case "SelfNode":
      return { title: "Self", sub: "whichever node fires this" };
    case "TriggerSource":
      return { title: "Trigger source", sub: "whatever caused the trigger" };
  }
}

/** Which action families a row contains, for the qualifier the mockup shows
 * next to an asset name ("Asset 1 · Transform"). Derived rather than
 * authored — a row can legitimately hold several kinds. */
export function assetRowAspects(asset: XrdsTrackAssetDto): string[] {
  const seen = new Set<string>();
  for (const key of asset.keys) {
    switch (key.action.kind) {
      case "SetTransform":
        seen.add("Transform");
        break;
      case "SetVisible":
        seen.add("Visibility");
        break;
      case "SetMaterial":
        seen.add("Material");
        break;
      case "PlayGltfAnimation":
      case "StopGltfAnimation":
        seen.add("Animation");
        break;
      case "ModifyHealth":
        seen.add("Health");
        break;
      case "Unknown":
        seen.add("Unknown");
        break;
    }
  }
  return [...seen];
}

/** Nodes that may still be added as a row to `track`.
 *
 * Excludes only nodes that already have a row in *this* Track — one row per
 * asset is a hard invariant the command layer also enforces. Nodes used by
 * *other* Tracks are deliberately still offered: sharing is allowed at author
 * time, and merely means the two Tracks cannot run at the same time. The
 * warning for that comes from `track_diagnostics`, not from hiding options. */
export function addableAssets(
  track: NamedTrackDto | null,
  snapshot: EditorSnapshot,
): { id: number; name: string }[] {
  const taken = new Set(
    (track?.assets ?? [])
      .map(a => (a.target.type === "Node" ? a.target.id : null))
      .filter((id): id is number => id !== null),
  );
  const out: { id: number; name: string }[] = [];
  const walk = (nodes: EditorSnapshot["hierarchy"]) => {
    for (const n of nodes) {
      if (!taken.has(n.id)) out.push({ id: n.id, name: n.name });
      walk(n.children);
    }
  };
  walk(snapshot.hierarchy);
  return out;
}

/** Which other Tracks share an asset with `name` — the authoring-time
 * counterpart to the runtime's reject-the-newcomer guard. Two Tracks sharing
 * an asset cannot run concurrently. */
export function conflictingTracks(name: string, tracks: NamedTrackDto[]): string[] {
  const mine = tracks.find(t => t.name === name);
  if (!mine) return [];
  const mineNodes = new Set(
    mine.assets.map(a => (a.target.type === "Node" ? a.target.id : null)).filter(id => id !== null),
  );
  return tracks
    .filter(t => t.name !== name)
    .filter(t =>
      t.assets.some(a => a.target.type === "Node" && mineNodes.has(a.target.id)),
    )
    .map(t => t.name);
}

// ---------------------------------------------------------------------------
// Triggers hierarchy reverse index
// ---------------------------------------------------------------------------

export interface TriggerReverseIndex {
  /** Track name -> every binding (across the whole document) that fires it.
   * Bindings with no Track selected never appear here. */
  byTrack: Map<string, NodeBindingSummary[]>;
  /** Node id -> every binding authored on that node. */
  byNode: Map<number, NodeBindingSummary[]>;
}

/** Builds both directions of the "what fires this Track" lookup from
 * `EditorSnapshot.all_node_bindings`. This is how a Track reports what fires
 * it without holding a back-reference — the resolution the design assessment
 * doc called "framed backwards". */
export function buildTriggerReverseIndex(allNodeBindings: NodeBindingSummary[]): TriggerReverseIndex {
  const byTrack = new Map<string, NodeBindingSummary[]>();
  const byNode = new Map<number, NodeBindingSummary[]>();

  for (const summary of allNodeBindings) {
    const track = summary.binding.track;
    if (track !== null) {
      const list = byTrack.get(track) ?? [];
      list.push(summary);
      byTrack.set(track, list);
    }
    const nodeList = byNode.get(summary.node_id) ?? [];
    nodeList.push(summary);
    byNode.set(summary.node_id, nodeList);
  }

  return { byTrack, byNode };
}

// ---------------------------------------------------------------------------
// Contextual trigger-kind filtering
// ---------------------------------------------------------------------------

/** Every `XrdsTriggerKind` variant name, in picker display order. */
export const ALL_TRIGGER_KINDS = [
  "ZoneEnter", "ZoneExit", "Grabbed", "Dropped", "HoverEnter", "HoverExit",
  "ButtonPress", "ButtonRelease", "SliderChange", "ToggleChange",
  "AnimationComplete", "Custom", "RunawayDetected",
] as const;

/** Kinds whose runtime event carries a hand — mirrors
 * `XrdsTriggerKind::carries_hand()` in Rust, which `track_diagnostics` uses
 * for the hand-filter-on-handless-kind error. Kept in sync deliberately: a
 * document authored before `validKindsFor` existed (or via expert Rust) can
 * still carry one of these on a node the picker would not offer it on, and
 * its hand picker must still render. */
const HAND_CARRYING_KINDS = new Set([
  "Grabbed", "Dropped", "HoverEnter", "HoverExit",
  "ButtonPress", "ButtonRelease", "SliderChange", "ToggleChange",
]);

export function isHandFilterVisible(kind: string): boolean {
  return HAND_CARRYING_KINDS.has(kind);
}

/** Why `kind` cannot fire for `node` right now, in a form short enough to
 * show right next to the kind in a picker — `null` means it is available.
 *
 * Single source of truth for `validKindsFor` below: filtering the kinds a
 * node "can't" use and merely *explaining* why are the same underlying
 * check, and used to be two separately-maintained things (a filter here, a
 * hand-written hint block in `Inspector.tsx`) that could drift apart.
 *
 * **`ButtonPress`/`ButtonRelease`/`SliderChange`/`ToggleChange` are never
 * available, on any node.** Confirmed against the runtime dispatch code: those
 * events target the individual widget's own ephemeral runtime `Entity`, not
 * any document node's `XrdsId`. `consume_triggers` looks for bindings on that
 * exact entity, but widgets are authored as plain data inside a `WorldPanel`'s
 * `widgets` array, never as their own importable node — so no
 * document-authored binding can ever receive them. A real, pre-existing gap;
 * this only decides what the picker offers.
 *
 * `HoverEnter`/`HoverExit` are different: they resolve via `self.panel_id` (a
 * real `XrdsId`), so they do work, scoped to the `WorldPanel` node itself. */
export function unavailableReasonFor(kind: string, node: NodeInspector): string | null {
  switch (kind) {
    case "ZoneEnter":
    case "ZoneExit":
      // InteractionZone has no dedicated payload variant — it lands in the
      // generic `Other { kind }` catch-all.
      return node.payload.type === "Other" && node.payload.kind === "InteractionZone"
        ? null : "needs an Interaction Zone node";
    case "Grabbed":
    case "Dropped":
      return node.grabbable ? null : "needs \"Grabbable\" checked";
    case "HoverEnter":
    case "HoverExit":
      return node.payload.type === "WorldPanel" ? null : "needs a World Panel";
    case "ButtonPress":
    case "ButtonRelease":
    case "SliderChange":
    case "ToggleChange":
      return "not reachable — authored widgets have no bindable node";
    case "AnimationComplete":
      return node.payload.type === "GltfAsset" ? null : "needs a glTF asset";
    case "Custom":
    case "RunawayDetected":
    default:
      return null;
  }
}

/** Which `XrdsTriggerKind`s make sense to offer for `node`, based on what can
 * actually fire them at runtime. See `unavailableReasonFor` for the per-kind
 * logic — this just keeps the ones with no reason. */
export function validKindsFor(node: NodeInspector): string[] {
  return ALL_TRIGGER_KINDS.filter(kind => unavailableReasonFor(kind, node) === null);
}

/** Action kinds an author may place on a Track row, in picker order.
 *
 * `Wait`, `Run` and `FireCustomEvent` are absent because they no longer exist:
 * a key carries its own time, and a Track cannot start another Track.
 * `Teleport` is absent because it was a zero-duration `SetTransform`.
 * `Unknown` is never offered — it only ever arrives from a newer editor. */
export const TRACK_ACTION_KINDS = [
  "SetTransform", "SetVisible", "SetMaterial",
  "PlayGltfAnimation", "StopGltfAnimation", "ModifyHealth",
] as const;

/** Coarse family for a row's target, used only for display grouping. */
export function targetLabel(target: ActionTarget): string {
  switch (target.type) {
    case "SelfNode": return "Self";
    case "Node": return `Node #${target.id}`;
    case "TriggerSource": return "Trigger source";
  }
}
