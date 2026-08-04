import { useCallback, useRef, useState } from "react";

interface Options {
  axis: "x" | "y";
  initial: number;
  min: number;
  max: number;
  /** The handle sits on one edge of the panel; whether dragging in the
   * positive screen direction (right for x, down for y) should grow or
   * shrink the panel depends on which edge that is. A left-sidebar's
   * handle is on its right edge (drag right → grow, invert: false); an
   * inspector's handle is on its left edge (drag left → grow, invert:
   * true); a bottom-docked panel's handle is on its own top edge (drag up
   * → grow, invert: true). */
  invert?: boolean;
}

/** Shared drag-to-resize behavior for dockable panels (Palette, the left
 * sidebar, Inspector, and the stacked HUD/trigger-action library panels).
 * Returns the current size, whether a drag is in progress (for the
 * handle's hover/active styling), the pointerdown handler to wire onto the
 * handle element, and lock state — the three main panels (sidebar,
 * inspector, palette) expose a lock toggle so an accidental drag can't
 * resize them; locked panels ignore onPointerDown entirely. */
export function useResizable({ axis, initial, min, max, invert }: Options) {
  const [size, setSize] = useState(initial);
  const [dragging, setDragging] = useState(false);
  const [locked, setLocked] = useState(false);
  const start = useRef<{ pos: number; size: number } | null>(null);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (locked) return;
    e.preventDefault();
    start.current = { pos: axis === "x" ? e.clientX : e.clientY, size };
    setDragging(true);
    const onMove = (ev: PointerEvent) => {
      if (!start.current) return;
      const curPos = axis === "x" ? ev.clientX : ev.clientY;
      const delta = (curPos - start.current.pos) * (invert ? -1 : 1);
      setSize(Math.min(max, Math.max(min, start.current.size + delta)));
    };
    const onUp = () => {
      start.current = null;
      setDragging(false);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }, [axis, size, invert, min, max, locked]);

  const toggleLock = useCallback(() => setLocked(l => !l), []);

  return { size, dragging, onPointerDown, locked, toggleLock };
}
