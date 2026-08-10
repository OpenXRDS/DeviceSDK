/** Suppressing the Bevy viewport hole, reference-counted.
 *
 * **Why anything needs this.** On Windows the editor calls `SetWindowRgn` to cut a
 * hole in the WebView so Bevy renders there natively. That is a *clip*, not a
 * z-order: anything the WebView paints inside the hole is removed, not merely
 * covered. So a dropdown that extends over the 3D viewport is sliced off rather
 * than appearing above it — which is exactly what a long trigger-candidate list
 * does. The fix is to close the hole for as long as floating UI is open, which is
 * what `set_viewport_hole` was built for.
 *
 * **Why reference-counted.** Several things can want the hole closed at once — a
 * modal overlay with a `<Select>` inside it is the ordinary case. Plain
 * open/close would let the inner dropdown re-open the hole on close while the
 * modal is still up, slicing the modal instead. Counting means the hole reopens
 * only when the last holder releases.
 *
 * Idempotent per holder is *not* assumed: `acquire`/`release` must be paired, and
 * every caller here pairs them in an effect cleanup.
 */

let holders = 0;

function post(enabled: boolean): void {
  // `ipc` is injected by wry. Absent in a plain browser and under vitest, where
  // there is no hole either — so doing nothing is correct, not a silent failure
  // worth logging.
  //
  // Read off `globalThis`, not `window`: the tests run in vitest's node
  // environment, where touching `window` is a ReferenceError rather than
  // `undefined`. `globalThis` is the same object as `window` in the browser, so
  // this costs nothing there and makes the module testable without a DOM.
  const host = globalThis as { ipc?: { postMessage(m: string): void } };
  host.ipc?.postMessage(JSON.stringify({ type: "set_viewport_hole", enabled }));
}

/** Close the hole (if it is not already closed on someone else's behalf). */
export function acquireViewportHoleSuppression(): void {
  holders += 1;
  if (holders === 1) post(false);
}

/** Release one hold, reopening the hole when the last one goes. */
export function releaseViewportHoleSuppression(): void {
  // Guarded rather than allowed to go negative: an unpaired release would
  // otherwise leave the count below zero and the *next* acquire would not close
  // the hole, which is a far more confusing symptom than the double release.
  if (holders === 0) return;
  holders -= 1;
  if (holders === 0) post(true);
}

/** Current hold count — for tests. */
export function viewportHoleHolders(): number {
  return holders;
}

/** Test-only reset, so one test's leak cannot alter another's arithmetic. */
export function __resetViewportHoleForTests(): void {
  holders = 0;
}
