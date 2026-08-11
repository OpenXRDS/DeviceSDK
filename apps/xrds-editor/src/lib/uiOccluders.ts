/** Reporting where floating UI sits, so the Bevy viewport hole can be shaped
 *  around it instead of switched off.
 *
 * **Why this replaced closing the hole.** `SetWindowRgn` is a clip, not a
 * z-order: inside the hole the WebView is *removed*, not merely covered. A
 * dropdown extending over the 3D viewport was therefore sliced off. The first fix
 * closed the hole for as long as floating UI was open — which let the WebView
 * paint the whole window, and since the page background is opaque, **the 3D scene
 * went black** for the duration. That is worse than a clipped dropdown, and it
 * showed up on every trigger-candidate list and every add-asset dialog.
 *
 * There is no compositing option: nothing alpha-blends the WebView against Bevy's
 * swap-chain. What can be changed is the hole's *shape*. Each floating element
 * registers its own rectangle here; Rust subtracts those from the hole, so the 3D
 * keeps rendering everywhere except the few hundred pixels actually beneath the
 * dropdown.
 *
 * Keyed by an opaque id so several elements can float at once — a `<Select>`
 * inside a modal is the ordinary case — and so unmount can retract exactly one
 * without guessing at the others. That is also why this is a map and not the
 * reference count it replaced: a count could say *how many* things floated but
 * never *where* they were.
 *
 * Contrast `viewportHole.ts`, which is still right for the Panels workspace:
 * there the viewport is genuinely not on screen, so removing the hole outright is
 * correct rather than a workaround.
 */

export interface OccluderRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const rects = new Map<string, OccluderRect>();

function post(): void {
  // `ipc` is injected by wry; absent in a plain browser and under vitest, where
  // there is no hole to reshape either. Read off `globalThis` rather than
  // `window` so the tests can run in vitest's node environment, where touching
  // `window` is a ReferenceError.
  const host = globalThis as { ipc?: { postMessage(m: string): void } };
  host.ipc?.postMessage(
    JSON.stringify({ type: "set_ui_occluders", rects: [...rects.values()] }),
  );
}

/** Register or move the rectangle occupied by `id`.
 *
 * Called on open and again whenever the element moves or resizes — Radix
 * repositions its content on scroll and on collision, so a stale rectangle would
 * leave a lit patch of WebView beside a sliced dropdown.
 *
 * A zero-area rect is dropped rather than stored: it cannot occlude anything, and
 * forwarding it would make Rust rebuild the region for no reason. An element
 * measured before layout settles reports 0×0, which is exactly this case.
 */
export function setOccluder(id: string, rect: OccluderRect): void {
  if (!(rect.w > 0 && rect.h > 0)) {
    clearOccluder(id);
    return;
  }
  const prev = rects.get(id);
  if (prev && prev.x === rect.x && prev.y === rect.y
           && prev.w === rect.w && prev.h === rect.h) {
    return; // unchanged — don't churn the native region
  }
  rects.set(id, rect);
  post();
}

/** Retract `id`'s rectangle, restoring that part of the hole. */
export function clearOccluder(id: string): void {
  if (!rects.delete(id)) return; // never registered — nothing to restore
  post();
}

/** Measure `el` and register it under `id`. Returns false if it isn't laid out yet. */
export function trackOccluder(id: string, el: Element | null): boolean {
  if (!el) {
    clearOccluder(id);
    return false;
  }
  const r = el.getBoundingClientRect();
  setOccluder(id, { x: r.left, y: r.top, w: r.width, h: r.height });
  return r.width > 0 && r.height > 0;
}

/** Currently registered rectangles — for tests. */
export function occluderRects(): OccluderRect[] {
  return [...rects.values()];
}

/** Test-only reset, so one test's leak cannot alter another's expectations. */
export function __resetOccludersForTests(): void {
  rects.clear();
}
