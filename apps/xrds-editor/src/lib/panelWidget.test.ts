import { describe, it, expect } from "vitest";
import type { WorldWidget } from "../types/bridge";
import {
  widgetFields,
  fieldValue,
  withField,
  withVec2Component,
  movedTo,
  alphaOf,
  POSITION_FIELD,
} from "./panelWidget";

/** One fully-populated sample per kind. These mirror `WorldWidget` in
 *  `bridge.ts`; the coverage test below compares the field tables against them,
 *  so adding a field to the DTO without adding it here *and* to the form fails. */
const SAMPLES: WorldWidget[] = [
  {
    type: "Label",
    text: "Hi",
    font_size: 16,
    color: [1, 1, 1, 1],
    local_position: [0, 0],
    layout_size: [0.4, 0.1],
  },
  {
    type: "Button",
    label: "OK",
    font_size: 14,
    label_color: [1, 1, 1, 1],
    size: [0.2, 0.08],
    local_position: [0, 0],
    normal_color: [0.2, 0.2, 0.2, 1],
    hover_color: [0.3, 0.3, 0.3, 1],
    pressed_color: [0.1, 0.1, 0.1, 1],
  },
  {
    type: "Image",
    asset_path: "a.png",
    size: [0.2, 0.2],
    local_position: [0, 0],
    tint: [1, 1, 1, 1],
  },
  {
    type: "Slider",
    min: 0,
    max: 1,
    value: 0.5,
    size: [0.3, 0.05],
    local_position: [0, 0],
    track_color: [0.2, 0.2, 0.2, 1],
    fill_color: [0.4, 0.6, 1, 1],
    thumb_color: [1, 1, 1, 1],
    thumb_size: 0.03,
  },
  {
    type: "Toggle",
    checked: false,
    size: [0.1, 0.05],
    local_position: [0, 0],
    track_off_color: [0.2, 0.2, 0.2, 1],
    track_on_color: [0.4, 0.8, 0.4, 1],
    thumb_color: [1, 1, 1, 1],
  },
];

describe("widget property fields", () => {
  it.each(SAMPLES.map(w => [w.type, w] as const))(
    "%s exposes every DTO field, so none is unauthorable",
    (_kind, widget) => {
      const offered = new Set(widgetFields(widget).map(f => f.key));
      const actual = Object.keys(widget).filter(k => k !== "type");
      // The whole point of the data-driven table: a field present on the wire
      // but missing from the form is silently uneditable.
      expect([...actual].sort().filter(k => !offered.has(k))).toEqual([]);
    },
  );

  it.each(SAMPLES.map(w => [w.type, w] as const))(
    "%s offers no field the DTO does not have",
    (_kind, widget) => {
      // The reverse mistake: a stale field name writes a key the Rust side
      // ignores, so the edit appears to work and does nothing.
      for (const f of widgetFields(widget)) {
        expect(Object.keys(widget)).toContain(f.key);
      }
    },
  );

  it("puts position first for every kind, since it is the one shared field", () => {
    for (const w of SAMPLES) {
      expect(widgetFields(w)[0]).toEqual(POSITION_FIELD);
    }
  });

  it("declares each field the kind the DTO actually uses", () => {
    for (const widget of SAMPLES) {
      for (const f of widgetFields(widget)) {
        const v = fieldValue(widget, f.key);
        if (f.kind === "text") expect(typeof v).toBe("string");
        if (f.kind === "number") expect(typeof v).toBe("number");
        if (f.kind === "bool") expect(typeof v).toBe("boolean");
        if (f.kind === "color") expect(v).toHaveLength(4);
        if (f.kind === "vec2") expect(v).toHaveLength(2);
      }
    }
  });

  it("gives an unknown kind position only, rather than throwing", () => {
    // A document from a newer build must not blank the inspector.
    const alien = { type: "Hologram", local_position: [0, 0] } as unknown as WorldWidget;
    expect(widgetFields(alien)).toEqual([POSITION_FIELD]);
  });
});

describe("editing", () => {
  it("never mutates, so a live-preview can still compare against the old value", () => {
    const before = SAMPLES[0];
    const after = withField(before, "text", "Bye");
    expect(after).not.toBe(before);
    expect((before as any).text).toBe("Hi");
    expect((after as any).text).toBe("Bye");
  });

  it("keeps the untouched component when one half of a vec2 changes", () => {
    // Writing the whole pair from one input is how the other half gets zeroed.
    const w = withVec2Component(SAMPLES[1], "size", 1, 0.5);
    expect((w as any).size).toEqual([0.2, 0.5]);
  });

  it("keeps the kind tag through an edit", () => {
    expect(withField(SAMPLES[3], "value", 0.9).type).toBe("Slider");
  });

  it("moves to an absolute position, which is what a drag produces", () => {
    expect((movedTo(SAMPLES[2], 0.1, -0.2) as any).local_position).toEqual([0.1, -0.2]);
  });

  it("reads alpha back so a hex input does not make things opaque", () => {
    const translucent = withField(SAMPLES[0], "color", [1, 1, 1, 0.25]);
    expect(alphaOf(translucent, "color")).toBe(0.25);
  });

  it("defaults a missing alpha to opaque rather than invisible", () => {
    // Guards the direction of the fallback: 0 would make the element vanish.
    expect(alphaOf(SAMPLES[0], "nonexistent")).toBe(1);
  });
});
