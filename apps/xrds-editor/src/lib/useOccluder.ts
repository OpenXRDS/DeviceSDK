import { useEffect, useId, useRef } from "react";
import { clearOccluder, trackOccluder } from "./uiOccluders";

/**
 * Keep an element painted on top of the Bevy viewport while it is visible.
 *
 * ## Why this is needed at all
 *
 * On Windows the editor carves a viewport hole with `SetWindowRgn` so Bevy renders
 * natively there. That is a **clip, not a z-order**: inside the hole the WebView is
 * simply not painted, so any UI overlapping the viewport is invisible no matter
 * what its CSS z-index says. Registering the element's rectangle subtracts it from
 * the hole's shape, which is the only way to get UI over the 3D view — see
 * `src-tauri/src/wry_overlay.rs`'s `ui_occluders`.
 *
 * ## Why a hook rather than repeating the effect
 *
 * `ui/Select.tsx` grew this logic for dropdowns during the Radix migration and it
 * was never applied to modals, so **every** dialog that overlapped the viewport
 * vanished behind the 3D scene — the APK export dialog most visibly, since it is
 * centred and therefore always over the viewport. Reported from the editor
 * 2026-08-19. One hook means the next dialog cannot forget.
 *
 * Tracked per animation frame rather than measured once: a dialog can be
 * repositioned by layout, by the window resizing, or by content loading, and a
 * stale rectangle leaves a lit patch of WebView beside a sliced dialog. The
 * cleanup covers unmount-while-open, which would otherwise leave a permanent bite
 * out of the viewport.
 *
 * @param active whether the element is currently displayed
 * @returns a ref to attach to the element that overlaps the viewport
 */
export function useOccluder<T extends HTMLElement>(active: boolean) {
  const ref = useRef<T | null>(null);
  const id = useId();

  useEffect(() => {
    if (!active) {
      clearOccluder(id);
      return;
    }
    let raf = 0;
    const tick = () => {
      trackOccluder(id, ref.current);
      raf = requestAnimationFrame(tick);
    };
    tick();
    return () => {
      cancelAnimationFrame(raf);
      clearOccluder(id);
    };
  }, [active, id]);

  return ref;
}
