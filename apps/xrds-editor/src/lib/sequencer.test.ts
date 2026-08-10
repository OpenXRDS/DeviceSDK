import { describe, expect, it } from "vitest";
import {
  ALL_TRIGGER_KINDS, actionDuration, addableAssets, assetRowAspects, assetRowLabel,
  ROW_H, SUBLANE_STEP, buildTriggerReverseIndex, conflictingTracks, dotFootprintSecs, fmtTime,
  isHandFilterVisible, keyTopPx, layoutAssetRow, niceStep, rulerTicks, stackKeys, validKindsFor,
  elementUnavailableReasonFor, validKindsForElement, elementRowLabel, unavailableReasonFor,
  targetLabel, actionUnavailableReasonFor, ELEMENT_ONLY_ACTION_KINDS, TRACK_ACTION_KINDS,
  decodeAddableAsset, encodeAddableAsset,
} from "./sequencer";
import { defaultSnapshot } from "../types/bridge";
import type {
  EditorSnapshot, NamedTrackDto, NodeBindingSummary, NodeInspector, TriggerBindingDto,
  XrdsAction, XrdsTrackAssetDto, XrdsTrackKeyDto, PanelElementDto,
} from "../types/bridge";

// ---------------------------------------------------------------------------
// Asset rows
//
// There is no `deriveLanes` any more: a Track's rows are authored, arriving as
// `NamedTrackDto.assets`, rather than inferred by grouping actions. The helpers
// below are what remain of that job — labelling and offering rows.
// ---------------------------------------------------------------------------

function nodeRow(id: number, name: string | null, keys: XrdsTrackKeyDto[] = []): XrdsTrackAssetDto {
  return { target: { type: "Node", id }, node_name: name, keys };
}

function key(at_secs: number, action: XrdsAction): XrdsTrackKeyDto {
  return { at_secs, action };
}

/** An empty texture-slot set — every node material fixture needs one. */
const NO_TEXTURES = {
  base_color: null, metallic_roughness: null, normal: null, occlusion: null, emissive: null,
};

/** A zero-duration `SetTransform` — what the deleted `Teleport` action was. */
const TELEPORT: XrdsAction = {
  kind: "SetTransform",
  data: { position: [1, 0, 0], rotation: null, scale: null, duration_secs: 0, ease: "Linear" },
};
const animate = (duration_secs: number): XrdsAction => ({
  kind: "SetTransform",
  data: { position: [1, 0, 0], rotation: null, scale: null, duration_secs, ease: "Cubic" },
});

describe("actionDuration", () => {
  it("is non-zero only for interpolation, matching the Rust side", () => {
    expect(actionDuration(animate(1.5))).toBe(1.5);
    expect(actionDuration({ kind: "StopGltfAnimation" })).toBe(0);
  });

  it("reports 0 for an instant SetTransform, so it draws as a dot", () => {
    // This is the case the deleted `Teleport` action used to cover. Keeping it
    // asserted makes the dot-vs-bar behaviour survive that removal.
    expect(actionDuration(TELEPORT)).toBe(0);
  });

  it("never reports a negative duration, so a bar can't render backwards", () => {
    expect(actionDuration(animate(-2))).toBe(0);
  });
});

describe("stackKeys", () => {
  it("puts every key in lane 0 when nothing overlaps", () => {
    const { lanes, count } = stackKeys([key(0, TELEPORT), key(1, TELEPORT), key(2, TELEPORT)], 0.1);
    expect(lanes).toEqual([0, 0, 0]);
    expect(count).toBe(1);
  });

  it("stacks two dots at the same instant into separate lanes", () => {
    const { lanes, count } = stackKeys([key(1, TELEPORT), key(1, TELEPORT)], 0.1);
    expect(new Set(lanes)).toEqual(new Set([0, 1]));
    expect(count).toBe(2);
  });

  it("reuses a lane once its occupant's visual footprint has passed", () => {
    // Two dots 0.05s apart with a 0.1s minimum footprint overlap and must
    // split lanes; a third dot well clear of both can reuse lane 0.
    const { lanes, count } = stackKeys(
      [key(0, TELEPORT), key(0.05, TELEPORT), key(5, TELEPORT)],
      0.1,
    );
    expect(lanes[0]).not.toBe(lanes[1]);
    expect(count).toBe(2);
    expect(lanes[2]).toBe(0);
  });

  it("stacks a dot landing inside a running bar's duration", () => {
    const { lanes, count } = stackKeys([key(0, animate(2)), key(1, TELEPORT)], 0.1);
    expect(lanes[0]).not.toBe(lanes[1]);
    expect(count).toBe(2);
  });

  it("does not stack a dot that lands exactly when a bar ends", () => {
    // Touching endpoints are not an overlap — otherwise a Track authored to
    // hand off cleanly (one event ending exactly when the next starts) would
    // stack for no visual reason.
    const { lanes, count } = stackKeys([key(0, animate(1)), key(1, TELEPORT)], 0);
    expect(lanes).toEqual([0, 0]);
    expect(count).toBe(1);
  });

  it("reports at least 1 lane for an empty row, so height math never divides by 0", () => {
    expect(stackKeys([], 0.1).count).toBe(1);
  });

  it("does NOT stack same-instant dots when the footprint is 0", () => {
    // Documents the trap `dotFootprintSecs` exists to prevent, rather than
    // leaving it as a latent surprise: with no footprint a dot's interval is
    // degenerate (`[at, at]`) and touching endpoints don't overlap, so
    // stacking silently does nothing at all. This shipped broken once —
    // every caller must go through `dotFootprintSecs`, never pass 0.
    expect(stackKeys([key(1, TELEPORT), key(1, TELEPORT)], 0).count).toBe(1);
  });
});

describe("dotFootprintSecs", () => {
  it("is never 0, even before the lane has been measured", () => {
    // The real regression: lane width is 0 until the first layout pass, and
    // a 0 footprint disables stacking entirely (see the test above).
    expect(dotFootprintSecs(16, 0, 2)).toBeGreaterThan(0);
  });

  it("is never 0 for a degenerate duration either", () => {
    expect(dotFootprintSecs(16, 800, 0)).toBeGreaterThan(0);
  });

  it("shrinks as the lane gets wider, since a dot covers less time", () => {
    expect(dotFootprintSecs(16, 1600, 10)).toBeLessThan(dotFootprintSecs(16, 400, 10));
  });

  it("scales with duration, so stacking behaves the same at any zoom", () => {
    // 16px of a 800px lane is 2% of the span either way.
    expect(dotFootprintSecs(16, 800, 10)).toBeCloseTo(0.2);
    expect(dotFootprintSecs(16, 800, 100)).toBeCloseTo(2);
  });

  it("gives same-instant dots separate lanes at a realistic width", () => {
    // End-to-end version of the bug the user actually saw.
    const minSeconds = dotFootprintSecs(16, 1000, 2);
    expect(stackKeys([key(1, TELEPORT), key(1, TELEPORT)], minSeconds).count).toBe(2);
  });
});

// ---------------------------------------------------------------------------
// layoutAssetRow / keyTopPx
//
// The reported bug, as an executable spec: "two events with the same start
// time … are stacked on the same row, so it's not possible to click the
// hidden one." These assert the actual px the component paints, which is
// where both earlier regressions lived — `stackKeys` was never the problem.
// ---------------------------------------------------------------------------

describe("layoutAssetRow", () => {
  const twoAtOnce = [key(1, animate(0.5)), key(1, { kind: "SetVisible", data: false })];

  it("separates two events sharing a start time, so neither is unclickable", () => {
    const layout = layoutAssetRow(twoAtOnce, 1000, 2);
    expect(layout.count).toBe(2);
    const tops = layout.lanes.map(l => keyTopPx(l, layout.count, layout.height));
    expect(tops[0]).not.toBe(tops[1]);
    // Far enough apart that two 12px dots don't visually touch.
    expect(Math.abs(tops[0] - tops[1])).toBeGreaterThan(12);
  });

  it("still separates them when the lane has never been measured", () => {
    // lanePx = 0 is the state before the first layout pass — and was the
    // exact condition under which stacking silently did nothing.
    const layout = layoutAssetRow(twoAtOnce, 0, 2);
    expect(layout.count).toBe(2);
    const tops = layout.lanes.map(l => keyTopPx(l, layout.count, layout.height));
    expect(tops[0]).not.toBe(tops[1]);
  });

  // The two above both contain a 0.5s bar, which overlaps on *authored
  // duration* alone — so they stack even with a broken footprint, and a
  // mutation test proved they did. These use two instant dots, whose only
  // overlap is the pixel footprint, and so are the real guard.
  const twoInstantDots = [key(1, TELEPORT), key(1, { kind: "SetVisible", data: false })];

  it("separates two *instant* events sharing a start time", () => {
    const layout = layoutAssetRow(twoInstantDots, 1000, 2);
    expect(layout.count).toBe(2);
  });

  it("separates two instant events even before the lane is measured", () => {
    const layout = layoutAssetRow(twoInstantDots, 0, 2);
    expect(layout.count).toBe(2);
  });

  it("separates two instant events at any Track duration", () => {
    // Footprint scales with duration, so a long Track must not collapse them.
    for (const duration of [1, 2, 30, 600]) {
      expect(layoutAssetRow(twoInstantDots, 1000, duration).count).toBe(2);
    }
  });

  it("grows the row height per extra lane, and leaves a tidy row untouched", () => {
    expect(layoutAssetRow([key(0, TELEPORT)], 1000, 2).height).toBe(ROW_H);
    expect(layoutAssetRow(twoAtOnce, 1000, 2).height).toBe(ROW_H + SUBLANE_STEP);
  });

  it("keeps every key's top inside the row it belongs to", () => {
    const layout = layoutAssetRow(
      [key(1, TELEPORT), key(1, TELEPORT), key(1, TELEPORT)],
      1000,
      2,
    );
    expect(layout.count).toBe(3);
    for (const lane of layout.lanes) {
      const top = keyTopPx(lane, layout.count, layout.height);
      expect(top).toBeGreaterThan(0);
      expect(top).toBeLessThan(layout.height);
    }
  });

  it("survives a zero count without dividing by zero", () => {
    expect(Number.isFinite(keyTopPx(0, 0, ROW_H))).toBe(true);
  });
});

describe("assetRowLabel", () => {
  it("titles a node row with its resolved name", () => {
    expect(assetRowLabel(nodeRow(7, "crane_arm"))).toEqual({
      title: "crane_arm",
      sub: "node #7",
    });
  });

  it("says a node row is missing rather than rendering blank", () => {
    // node_name is null for a Node target that no longer exists. Silently
    // blank would look like a rendering bug; this makes it legible.
    expect(assetRowLabel(nodeRow(7, null)).sub).toContain("missing");
  });

  it("does not invent a name for SelfNode or TriggerSource rows", () => {
    // Neither has a concrete node until the Track is fired.
    expect(assetRowLabel({ target: { type: "SelfNode" }, node_name: null, keys: [] }).title)
      .toBe("Self");
    expect(assetRowLabel({ target: { type: "TriggerSource" }, node_name: null, keys: [] }).title)
      .toBe("Trigger source");
  });
});

describe("assetRowAspects", () => {
  it("reports each action family once, so a row reads 'Transform, Material'", () => {
    const row = nodeRow(1, "cube", [
      key(0, TELEPORT),
      key(1, animate(0.5)),
      key(2, { kind: "SetMaterial", data: { base_color: null, metallic: null, roughness: null, texture: null } }),
    ]);
    expect(assetRowAspects(row).sort()).toEqual(["Material", "Transform"]);
  });

  it("is empty for a row with no events", () => {
    expect(assetRowAspects(nodeRow(1, "cube"))).toEqual([]);
  });
});

describe("addableAssets", () => {
  const snapshot = {
    ...defaultSnapshot,
    hierarchy: [
      { id: 1, name: "A", kind: "Cube", visible: true, children: [
        { id: 2, name: "B", kind: "Cube", visible: true, children: [] },
      ] },
      { id: 3, name: "C", kind: "Cube", visible: true, children: [] },
    ],
  } as EditorSnapshot;

  const nodeIds = (rows: ReturnType<typeof addableAssets>) =>
    rows.filter(r => r.kind === "node").map(r => (r as { id: number }).id);

  it("walks the whole hierarchy, not just the roots", () => {
    expect(nodeIds(addableAssets(null, snapshot))).toEqual([1, 2, 3]);
  });

  it("excludes nodes that already have a row in THIS Track", () => {
    const track: NamedTrackDto = {
      name: "T",
      assets: [nodeRow(2, "B")],
      duration_secs: null,
      effective_duration_secs: 0,
      looping: false,
    };
    expect(nodeIds(addableAssets(track, snapshot))).toEqual([1, 3]);
  });

  it("still offers a node used by a DIFFERENT Track", () => {
    // Sharing is allowed at author time — it only means the two Tracks cannot
    // run concurrently, which `conflictingTracks` reports. Hiding the option
    // would misrepresent the rule as a prohibition.
    const other: NamedTrackDto = {
      name: "Other",
      assets: [nodeRow(1, "A")],
      duration_secs: null,
      effective_duration_secs: 0,
      looping: false,
    };
    const snap = { ...snapshot, tracks: [other] } as EditorSnapshot;
    expect(nodeIds(addableAssets(null, snap))).toContain(1);
  });
});

describe("conflictingTracks", () => {
  const mk = (name: string, ids: number[]): NamedTrackDto => ({
    name,
    assets: ids.map(id => nodeRow(id, `n${id}`)),
    duration_secs: null,
    effective_duration_secs: 0,
    looping: false,
  });

  it("names every other Track sharing at least one asset", () => {
    const tracks = [mk("A", [1, 2]), mk("B", [2, 3]), mk("C", [4])];
    expect(conflictingTracks("A", tracks)).toEqual(["B"]);
  });

  it("is empty when assets are disjoint — the whole point of the rule", () => {
    expect(conflictingTracks("A", [mk("A", [1]), mk("B", [2])])).toEqual([]);
  });

  it("ignores SelfNode rows, which resolve differently per firing", () => {
    const selfish = (name: string): NamedTrackDto => ({
      name,
      assets: [{ target: { type: "SelfNode" }, node_name: null, keys: [] }],
      duration_secs: null,
      effective_duration_secs: 0,
      looping: false,
    });
    expect(conflictingTracks("A", [selfish("A"), selfish("B")])).toEqual([]);
  });

  it("returns nothing for a Track that is not in the list", () => {
    expect(conflictingTracks("ghost", [mk("A", [1])])).toEqual([]);
  });
});

function binding(overrides: Partial<TriggerBindingDto> = {}): TriggerBindingDto {
  return {
    trigger: { kind: "ZoneEnter" },
    effect: "Fire",
    disabled: false,
    hand: null,
    track: null,
    ...overrides,
  };
}

describe("buildTriggerReverseIndex", () => {
  it("indexes bindings by both the Track they fire and the node they are authored on", () => {
    const all: NodeBindingSummary[] = [
      { node_id: 1, node_name: "Door", binding_index: 0, binding: binding({ track: "open_door" }) },
      { node_id: 2, node_name: "Lever", binding_index: 0, binding: binding({ track: "open_door" }) },
      { node_id: 1, node_name: "Door", binding_index: 1, binding: binding({ track: null }) },
    ];
    const index = buildTriggerReverseIndex(all);

    expect(index.byTrack.get("open_door")?.map(b => b.node_id)).toEqual([1, 2]);
    expect(index.byTrack.has("nonexistent")).toBe(false);
    expect(index.byNode.get(1)?.length).toBe(2);
    expect(index.byNode.get(2)?.length).toBe(1);
  });

  it("never puts an unwired binding (track: null) into byTrack", () => {
    const all: NodeBindingSummary[] = [
      { node_id: 1, node_name: "Door", binding_index: 0, binding: binding({ track: null }) },
    ];
    const index = buildTriggerReverseIndex(all);
    expect(index.byTrack.size).toBe(0);
    expect(index.byNode.get(1)?.length).toBe(1);
  });

  it("returns empty maps for an empty document", () => {
    const index = buildTriggerReverseIndex([]);
    expect(index.byTrack.size).toBe(0);
    expect(index.byNode.size).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// validKindsFor / isHandFilterVisible
// ---------------------------------------------------------------------------

function nodeWith(overrides: Partial<NodeInspector>): NodeInspector {
  return {
    id: 1,
    name: "N",
    visible: true,
    grabbable: false,
    parent_id: null,
    translation: [0, 0, 0],
    rotation_euler_degrees: [0, 0, 0],
    scale: [1, 1, 1],
    payload: { type: "Empty" },
    parent_kind: null,
    triggers: [],
    watchers: [],
    trigger_diagnostics: [],
    ...overrides,
  };
}

describe("validKindsFor", () => {
  it("never offers ButtonPress/ButtonRelease/SliderChange/ToggleChange on a NODE", () => {
    // These target an element's own runtime entity, and a node is not an
    // element — even a Panel node itself. `elementUnavailableReasonFor` covers
    // the element-side rule; this is only the node-side "wrong place" message.
    const panel = nodeWith({ payload: { type: "Panel", template_id: 1, elements: [] } });
    const kinds = validKindsFor(panel);
    expect(kinds).not.toContain("ButtonPress");
    expect(kinds).not.toContain("ButtonRelease");
    expect(kinds).not.toContain("SliderChange");
    expect(kinds).not.toContain("ToggleChange");
  });

  it("offers HoverEnter/HoverExit only on a Panel node", () => {
    // `Panel` is the one payload that gets a pointer surface at runtime
    // (`apply_panel_backdrop_in_world`), so it is the one whose HoverEnter/
    // HoverExit can ever fire.
    const panel = nodeWith({ payload: { type: "Panel", template_id: 1, elements: [] } });
    const cube = nodeWith({ payload: { type: "Cube", material: { base_color: [1, 1, 1, 1], metallic: 0, roughness: 0.5, emissive: [0, 0, 0], textures: NO_TEXTURES }, physics_body: "None", gravity_scale: 1, mass: 1 } });

    expect(validKindsFor(panel)).toContain("HoverEnter");
    expect(validKindsFor(cube)).not.toContain("HoverEnter");
  });

  it("offers ZoneEnter/ZoneExit only on an Other{kind: InteractionZone} node", () => {
    const zone = nodeWith({ payload: { type: "Other", kind: "InteractionZone" } });
    const cube = nodeWith({ payload: { type: "Cube", material: { base_color: [1, 1, 1, 1], metallic: 0, roughness: 0.5, emissive: [0, 0, 0], textures: NO_TEXTURES }, physics_body: "None", gravity_scale: 1, mass: 1 } });

    expect(validKindsFor(zone)).toContain("ZoneEnter");
    expect(validKindsFor(zone)).toContain("ZoneExit");
    expect(validKindsFor(cube)).not.toContain("ZoneEnter");
  });

  it("offers Grabbed/Dropped only when grabbable is true", () => {
    const grabbable = nodeWith({ grabbable: true });
    const notGrabbable = nodeWith({ grabbable: false });

    expect(validKindsFor(grabbable)).toContain("Grabbed");
    expect(validKindsFor(grabbable)).toContain("Dropped");
    expect(validKindsFor(notGrabbable)).not.toContain("Grabbed");
  });

  it("offers AnimationComplete only on a GltfAsset node", () => {
    const gltf = nodeWith({ payload: { type: "GltfAsset", clips: [] } });
    const empty = nodeWith({});

    expect(validKindsFor(gltf)).toContain("AnimationComplete");
    expect(validKindsFor(empty)).not.toContain("AnimationComplete");
  });

  it("always offers Custom and RunawayDetected, on any node", () => {
    const empty = nodeWith({});
    expect(validKindsFor(empty)).toContain("Custom");
    expect(validKindsFor(empty)).toContain("RunawayDetected");
  });

  it("returns a subset of ALL_TRIGGER_KINDS for every node", () => {
    const empty = nodeWith({});
    for (const kind of validKindsFor(empty)) {
      expect(ALL_TRIGGER_KINDS).toContain(kind);
    }
  });
});

describe("isHandFilterVisible", () => {
  it("is true for every hand-carrying kind, matching trigger_diagnostics()'s rule exactly", () => {
    for (const kind of ["Grabbed", "Dropped", "HoverEnter", "HoverExit", "ButtonPress", "ButtonRelease", "SliderChange", "ToggleChange"]) {
      expect(isHandFilterVisible(kind)).toBe(true);
    }
  });

  it("is false for kinds that never report a hand", () => {
    for (const kind of ["ZoneEnter", "ZoneExit", "AnimationComplete", "Custom", "RunawayDetected", "Unknown"]) {
      expect(isHandFilterVisible(kind)).toBe(false);
    }
  });
});

// ---------------------------------------------------------------------------
// Ruler tick math
// ---------------------------------------------------------------------------

describe("niceStep", () => {
  it("picks a 1/2/5 x 10^n interval, never an arbitrary fraction", () => {
    for (const d of [0.4, 2.4, 7, 48, 300, 1440]) {
      const step = niceStep(d);
      const mantissa = step / Math.pow(10, Math.floor(Math.log10(step)));
      expect([1, 2, 5, 10]).toContain(+mantissa.toFixed(6));
    }
  });

  it("scales with duration instead of using one fixed division", () => {
    // The whole point: a short and a long sequence must not get the same step.
    expect(niceStep(2.4)).toBeLessThan(niceStep(300));
  });

  it("lands near the requested tick count", () => {
    for (const d of [2.4, 7, 48, 300]) {
      const count = d / niceStep(d);
      expect(count).toBeGreaterThanOrEqual(3);
      expect(count).toBeLessThanOrEqual(20);
    }
  });

  it("never returns 0 or NaN for a degenerate duration, so % positions stay finite", () => {
    for (const d of [0, -1, NaN]) {
      expect(niceStep(d)).toBe(1);
    }
  });
});

describe("rulerTicks", () => {
  it("starts at 0 and covers the whole duration", () => {
    const ticks = rulerTicks(48);
    expect(ticks[0]).toBe(0);
    expect(ticks[ticks.length - 1]).toBeLessThanOrEqual(48);
    expect(ticks[ticks.length - 1] + niceStep(48)).toBeGreaterThan(48);
  });

  it("is strictly increasing with no float-drift duplicates", () => {
    // 2.4 / 0.2 is exactly the case where naive accumulation yields
    // 0.6000000000000001 and a duplicate-looking label.
    const ticks = rulerTicks(2.4);
    for (let i = 1; i < ticks.length; i++) {
      expect(ticks[i]).toBeGreaterThan(ticks[i - 1]);
    }
    expect(new Set(ticks).size).toBe(ticks.length);
  });

  it("includes the exact endpoint when the duration is a whole multiple of the step", () => {
    expect(rulerTicks(40)).toContain(40);
  });
});

describe("fmtTime", () => {
  it("uses m:ss and drops sub-second noise for whole seconds", () => {
    expect(fmtTime(0)).toBe("0:00");
    expect(fmtTime(5)).toBe("0:05");
    expect(fmtTime(48)).toBe("0:48");
    expect(fmtTime(60)).toBe("1:00");
    expect(fmtTime(72)).toBe("1:12");
  });

  it("keeps two decimals only when the value actually has a fraction", () => {
    expect(fmtTime(2.4)).toBe("0:02.40");
    expect(fmtTime(0.2)).toBe("0:00.20");
  });
});


// ---------------------------------------------------------------------------
// Panel elements
//
// The measurable finish line of the panel-template plan: the four widget
// trigger kinds used to be reported as "not reachable — authored widgets have
// no bindable node". That is no longer true, because an element carries its own
// triggers and they attach to the entity the widget event targets.
// ---------------------------------------------------------------------------

/** No `triggers` parameter: a template element carries no bindings. Those live on
 *  each placed Panel node, so nothing here can have a binding count. */
function element(type: string, emittable: string[]): PanelElementDto {
  return {
    name: "el",
    // Only `type` is read by these helpers; the rest of the widget shape is
    // irrelevant here, so it is cast rather than fully spelled out.
    widget: { type } as PanelElementDto["widget"],
    emittable_triggers: emittable,
  };
}

describe("elementUnavailableReasonFor", () => {
  it("allows the kinds Rust says the element emits", () => {
    const button = element("Button", ["ButtonPress", "ButtonRelease"]);
    expect(elementUnavailableReasonFor("ButtonPress", button)).toBeNull();
    expect(elementUnavailableReasonFor("ButtonRelease", button)).toBeNull();
  });

  it("explains which kinds an element does emit when the asked-for one is wrong", () => {
    const slider = element("Slider", ["SliderChange"]);
    const reason = elementUnavailableReasonFor("ButtonPress", slider);
    expect(reason).toContain("Slider");
    expect(reason).toContain("SliderChange");
  });

  it("says a non-emitting element emits nothing rather than listing an empty set", () => {
    const label = element("Label", []);
    expect(elementUnavailableReasonFor("ButtonPress", label)).toBe("a Label emits nothing");
  });

  it("takes the emittable list from the snapshot rather than re-deriving it", () => {
    // If this helper reimplemented the rule, it would disagree with a snapshot
    // whose Rust side had changed — the drift the server-side field prevents.
    // A deliberately odd list proves the list is what is consulted.
    const odd = element("Button", ["ToggleChange"]);
    expect(elementUnavailableReasonFor("ToggleChange", odd)).toBeNull();
    expect(elementUnavailableReasonFor("ButtonPress", odd)).not.toBeNull();
  });
});

describe("validKindsForElement", () => {
  it("offers exactly what the element emits", () => {
    expect(validKindsForElement(element("Toggle", ["ToggleChange"]))).toEqual(["ToggleChange"]);
    expect(validKindsForElement(element("Label", []))).toEqual([]);
  });
});

describe("widget kinds on a node versus on an element", () => {
  it("no longer claims widget triggers are unreachable anywhere", () => {
    // The string this replaced was the plan's definition of done.
    const cube = nodeWith({
      payload: { type: "Cube", material: { base_color: [1,1,1,1], metallic: 0, roughness: 0.5, emissive: [0,0,0], textures: NO_TEXTURES }, physics_body: "None", gravity_scale: 1, mass: 1 },
    });
    for (const kind of ["ButtonPress", "ButtonRelease", "SliderChange", "ToggleChange"]) {
      const reason = unavailableReasonFor(kind, cube);
      expect(reason).not.toBeNull();
      expect(reason).not.toContain("not reachable");
      expect(reason).toContain("panel element");
    }
  });

  it("keeps node kinds off elements and element kinds off nodes", () => {
    // The two rules are disjoint on purpose.
    const button = element("Button", ["ButtonPress", "ButtonRelease"]);
    expect(elementUnavailableReasonFor("ZoneEnter", button)).not.toBeNull();
    expect(elementUnavailableReasonFor("Grabbed", button)).not.toBeNull();
  });
});

describe("elementRowLabel", () => {
  it("shows the element's kind", () => {
    expect(elementRowLabel(element("Button", ["ButtonPress"]))).toEqual({
      title: "el",
      sub: "Button",
    });
  });

  it("never shows a binding count, because a template has no bindings", () => {
    // Replaces a test that asserted the count and its pluralisation. Bindings
    // moved to each placed Panel node, so a number shown against the template
    // would be right for one instance and wrong for the next.
    for (const kind of ["Button", "Slider", "Toggle", "Label", "Image"]) {
      const { sub } = elementRowLabel(element(kind, []));
      expect(sub).toBe(kind);
      expect(sub).not.toMatch(/trigger/);
    }
  });
});

describe("assetRowLabel for element rows", () => {
  const el = (node_name: string | null) => ({
    target: { type: "Element" as const, panel: 10, name: "go" },
    node_name,
    keys: [],
  });

  it("uses the server-side panel · element join when it resolves", () => {
    // Joined in Rust so the UI never has to cross-reference panel_library, which
    // would go stale on a rename.
    expect(assetRowLabel(el("Console · go"))).toEqual({
      title: "Console · go",
      sub: "element on panel #10",
    });
  });

  it("is visibly wrong rather than blank when the panel is gone", () => {
    const { title, sub } = assetRowLabel(el(null));
    expect(title).toContain("go");
    expect(title).toContain("10");
    expect(sub).toMatch(/missing/);
  });
});

describe("targetLabel for elements", () => {
  it("names both halves, since an element has no id of its own", () => {
    expect(targetLabel({ type: "Element", panel: 7, name: "start" }))
      .toBe("start on panel #7");
  });
});

describe("actionUnavailableReasonFor", () => {
  const node = { type: "Node" as const, id: 1 };
  const element = { type: "Element" as const, panel: 10, name: "go" };

  it("allows every non-element action on any row", () => {
    for (const kind of ["SetTransform", "SetVisible", "SetMaterial", "ModifyHealth"]) {
      expect(actionUnavailableReasonFor(kind, node)).toBeNull();
      expect(actionUnavailableReasonFor(kind, element)).toBeNull();
    }
  });

  it("blocks element actions on a node row, with a reason rather than by hiding", () => {
    // Same principle as the trigger-kind pickers: show the whole menu and say
    // what a greyed entry needs, so a short list is never a mystery.
    for (const kind of ["SetElementText", "SetElementValue", "SetElementEnabled"]) {
      expect(actionUnavailableReasonFor(kind, node)).toMatch(/panel element/);
    }
  });

  it("allows element actions on an element row", () => {
    for (const kind of ["SetElementText", "SetElementValue", "SetElementEnabled"]) {
      expect(actionUnavailableReasonFor(kind, element)).toBeNull();
    }
  });

  it("covers every element action the kind list offers", () => {
    // Guards drift between the offered list and the set that gates it: an
    // element action missing from ELEMENT_ONLY_ACTION_KINDS would be offered on
    // a node row and silently do nothing.
    const offered = TRACK_ACTION_KINDS.filter(k => k.startsWith("SetElement"));
    expect(offered.length).toBeGreaterThan(0);
    for (const kind of offered) {
      expect(ELEMENT_ONLY_ACTION_KINDS.has(kind)).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// Element rows in the asset picker (Phase B3)
// ---------------------------------------------------------------------------

describe("addableAssets with panel elements", () => {
  const withPanels = {
    ...defaultSnapshot,
    hierarchy: [{ id: 1, name: "Cube", kind: "Cube", visible: true, children: [] }],
    panel_instances: [
      {
        node_id: 10,
        node_name: "Floor1",
        elements: [
          { name: "go", kind: "Button" },
          { name: "readout", kind: "Label" },
        ],
      },
      {
        node_id: 11,
        node_name: "Floor3",
        elements: [{ name: "go", kind: "Button" }],
      },
    ],
  } as EditorSnapshot;

  const elementRow = (panel: number, name: string): XrdsTrackAssetDto => ({
    target: { type: "Element", panel, name },
    node_name: null,
    keys: [],
  });

  const track = (assets: XrdsTrackAssetDto[]): NamedTrackDto => ({
    name: "T",
    assets,
    duration_secs: null,
    effective_duration_secs: 0,
    looping: false,
  });

  it("offers every element of every placed panel, after the nodes", () => {
    const rows = addableAssets(null, withPanels);
    const elements = rows.filter(r => r.kind === "element");
    expect(elements).toHaveLength(3);
    // Nodes first so the common case stays at the top of the list.
    expect(rows[0].kind).toBe("node");
  });

  it("labels an element with its panel, name and kind", () => {
    const row = addableAssets(null, withPanels).find(
      r => r.kind === "element" && r.name === "readout",
    );
    expect(row?.label).toBe("Floor1 · readout (Label)");
  });

  it("excludes an element that already has a row in THIS Track", () => {
    const rows = addableAssets(track([elementRow(10, "go")]), withPanels);
    const keys = rows.filter(r => r.kind === "element").map(r => `${(r as any).panel}:${(r as any).name}`);
    expect(keys).not.toContain("10:go");
  });

  it("still offers the SAME element name on a DIFFERENT panel", () => {
    // The point of (panel, name) addressing: floor 1's button and floor 3's
    // button are separate assets, so taking one must not hide the other.
    const rows = addableAssets(track([elementRow(10, "go")]), withPanels);
    const keys = rows.filter(r => r.kind === "element").map(r => `${(r as any).panel}:${(r as any).name}`);
    expect(keys).toContain("11:go");
  });

  it("taking an element row does not hide any node", () => {
    const rows = addableAssets(track([elementRow(10, "go")]), withPanels);
    expect(rows.filter(r => r.kind === "node")).toHaveLength(1);
  });

  it("offers nothing for a panel whose template is missing", () => {
    // The Rust builder sends such a panel with an empty element list rather than
    // omitting it, so the picker shows the panel with nothing to add.
    const snap = {
      ...withPanels,
      panel_instances: [{ node_id: 12, node_name: "Dangling", elements: [] }],
    } as EditorSnapshot;
    expect(addableAssets(null, snap).filter(r => r.kind === "element")).toHaveLength(0);
  });
});

describe("addable asset encoding", () => {
  it("round-trips a node", () => {
    const back = decodeAddableAsset(encodeAddableAsset({ kind: "node", id: 7, label: "x" }));
    expect(back).toMatchObject({ kind: "node", id: 7 });
  });

  it("round-trips an element", () => {
    const back = decodeAddableAsset(
      encodeAddableAsset({ kind: "element", panel: 10, name: "go", label: "x" }),
    );
    expect(back).toMatchObject({ kind: "element", panel: 10, name: "go" });
  });

  it("preserves an element name containing a colon", () => {
    // The naming policy allows printable ASCII, so a colon is legal in a name.
    // Splitting on every colon instead of the first two would truncate it, and
    // the resulting command would silently address a different element.
    const back = decodeAddableAsset(
      encodeAddableAsset({ kind: "element", panel: 3, name: "a:b:c", label: "" }),
    );
    expect(back).toMatchObject({ kind: "element", panel: 3, name: "a:b:c" });
  });

  it("rejects a malformed value rather than inventing a target", () => {
    // A stale frontend or a hand-crafted value must not resolve to node 0 or to
    // an empty element name — either would send a command addressing something
    // real by accident.
    for (const bad of ["", "nonsense", "el:", "el:3", "el:3:", "node:", "node:abc"]) {
      expect(decodeAddableAsset(bad)).toBeNull();
    }
  });
});
