/** Property-form description for a panel element's widget.
 *
 * Data-driven rather than five hand-written forms. `WorldWidget` is a
 * five-variant union whose members overlap heavily (every one has a
 * `local_position`, four have a `size`, all but Toggle have a colour), so five
 * bespoke JSX forms would be four opportunities to forget a field — and
 * forgetting one means the property is unauthorable with no error anywhere.
 *
 * Lives here rather than in the component so the field tables can be asserted
 * against the DTO in vitest, which is the only thing that actually catches a
 * missing field. Same reason the sequencer's layout maths moved into
 * `sequencer.ts`.
 */

import type { WorldWidget, RGBA } from "../types/bridge";

export type FieldSpec =
  | { key: string; label: string; kind: "text"; placeholder?: string }
  | { key: string; label: string; kind: "number"; step?: number; min?: number }
  | { key: string; label: string; kind: "color" }
  | { key: string; label: string; kind: "bool" }
  /** Two numbers edited together. `unit` is only a display hint. */
  | { key: string; label: string; kind: "vec2"; unit: "m" };

/** Shared by every kind, so the form can show it above the per-kind fields. */
export const POSITION_FIELD: FieldSpec = {
  key: "local_position",
  label: "POSITION (M, FROM CENTRE)",
  kind: "vec2",
  unit: "m",
};

/** Per-kind fields, excluding `local_position` — see {@link POSITION_FIELD}. */
const FIELDS: Record<WorldWidget["type"], FieldSpec[]> = {
  Label: [
    { key: "text", label: "TEXT", kind: "text", placeholder: "Label text" },
    { key: "font_size", label: "FONT SIZE", kind: "number", step: 1, min: 1 },
    { key: "color", label: "COLOUR", kind: "color" },
    { key: "layout_size", label: "LAYOUT SIZE (M)", kind: "vec2", unit: "m" },
  ],
  Button: [
    { key: "label", label: "LABEL", kind: "text", placeholder: "Button text" },
    { key: "font_size", label: "FONT SIZE", kind: "number", step: 1, min: 1 },
    { key: "label_color", label: "LABEL COLOUR", kind: "color" },
    { key: "size", label: "SIZE (M)", kind: "vec2", unit: "m" },
    { key: "normal_color", label: "NORMAL", kind: "color" },
    { key: "hover_color", label: "HOVER", kind: "color" },
    { key: "pressed_color", label: "PRESSED", kind: "color" },
  ],
  Image: [
    { key: "asset_path", label: "ASSET PATH", kind: "text", placeholder: "textures/icon.png" },
    { key: "size", label: "SIZE (M)", kind: "vec2", unit: "m" },
    { key: "tint", label: "TINT", kind: "color" },
  ],
  Slider: [
    { key: "min", label: "MIN", kind: "number", step: 0.1 },
    { key: "max", label: "MAX", kind: "number", step: 0.1 },
    { key: "value", label: "VALUE", kind: "number", step: 0.1 },
    { key: "size", label: "SIZE (M)", kind: "vec2", unit: "m" },
    { key: "thumb_size", label: "THUMB SIZE", kind: "number", step: 0.01, min: 0 },
    { key: "track_color", label: "TRACK", kind: "color" },
    { key: "fill_color", label: "FILL", kind: "color" },
    { key: "thumb_color", label: "THUMB", kind: "color" },
  ],
  Toggle: [
    { key: "checked", label: "CHECKED", kind: "bool" },
    { key: "size", label: "SIZE (M)", kind: "vec2", unit: "m" },
    { key: "track_off_color", label: "TRACK OFF", kind: "color" },
    { key: "track_on_color", label: "TRACK ON", kind: "color" },
    { key: "thumb_color", label: "THUMB", kind: "color" },
  ],
};

/** Every editable field of `widget`, position first. */
export function widgetFields(widget: WorldWidget): FieldSpec[] {
  return [POSITION_FIELD, ...(FIELDS[widget.type] ?? [])];
}

export function fieldValue(widget: WorldWidget, key: string): unknown {
  return (widget as unknown as Record<string, unknown>)[key];
}

/** `widget` with one field replaced. Never mutates — the caller may still be
 *  holding the old value for a live-preview/commit comparison. */
export function withField(widget: WorldWidget, key: string, value: unknown): WorldWidget {
  return { ...widget, [key]: value } as WorldWidget;
}

/** `widget` with one component of a vec2 field replaced. */
export function withVec2Component(
  widget: WorldWidget,
  key: string,
  index: 0 | 1,
  value: number,
): WorldWidget {
  const cur = fieldValue(widget, key) as [number, number] | undefined;
  const next: [number, number] = [cur?.[0] ?? 0, cur?.[1] ?? 0];
  next[index] = value;
  return withField(widget, key, next);
}

/** Moves `widget` to an absolute position — what a canvas drag produces. */
export function movedTo(widget: WorldWidget, x: number, y: number): WorldWidget {
  return withField(widget, POSITION_FIELD.key, [x, y]);
}

/** Alpha of a colour field, so a hex input can preserve it. `<input
 *  type="color">` speaks RGB only, and defaulting the missing alpha to 1 would
 *  silently make every transparent element opaque on first touch. */
export function alphaOf(widget: WorldWidget, key: string): number {
  const c = fieldValue(widget, key) as RGBA | undefined;
  return c?.[3] ?? 1;
}
