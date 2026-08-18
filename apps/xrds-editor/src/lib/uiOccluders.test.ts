import { beforeEach, describe, expect, it } from "vitest";
import {
  __resetOccludersForTests,
  clearOccluder,
  occluderRects,
  setOccluder,
  trackOccluder,
} from "./uiOccluders";

/** Captures the `set_ui_occluders` payloads actually posted over IPC. */
function captureIpc(): { rects: unknown[][]; posts: number } {
  const captured = { rects: [] as unknown[][], posts: 0 };
  (globalThis as Record<string, unknown>).ipc = {
    postMessage(m: string) {
      const parsed = JSON.parse(m);
      if (parsed.type === "set_ui_occluders") {
        captured.posts += 1;
        captured.rects.push(parsed.rects);
      }
    },
  };
  return captured;
}

const R = (x: number, y: number, w = 100, h = 50) => ({ x, y, w, h });

beforeEach(() => {
  __resetOccludersForTests();
  delete (globalThis as Record<string, unknown>).ipc;
});

describe("occluder registry", () => {
  it("reports a registered rectangle to Rust", () => {
    const ipc = captureIpc();
    setOccluder("a", R(10, 20));
    expect(ipc.rects[ipc.rects.length - 1]).toEqual([{ x: 10, y: 20, w: 100, h: 50 }]);
  });

  it("keeps several floating elements at once", () => {
    // A <Select> inside a modal is the ordinary case; the earlier refcount could
    // say how many things floated but never where they were.
    const ipc = captureIpc();
    setOccluder("modal", R(0, 0, 400, 300));
    setOccluder("dropdown", R(50, 60));
    expect(ipc.rects[ipc.rects.length - 1]).toHaveLength(2);
  });

  it("restores only the released rectangle, leaving the others", () => {
    const ipc = captureIpc();
    setOccluder("modal", R(0, 0, 400, 300));
    setOccluder("dropdown", R(50, 60));
    clearOccluder("dropdown");
    expect(ipc.rects[ipc.rects.length - 1]).toEqual([{ x: 0, y: 0, w: 400, h: 300 }]);
  });

  it("posts an empty list once the last holder goes, restoring the plain hole", () => {
    const ipc = captureIpc();
    setOccluder("a", R(10, 20));
    clearOccluder("a");
    expect(ipc.rects[ipc.rects.length - 1]).toEqual([]);
  });

  it("does not re-post an unchanged rectangle", () => {
    // The rAF tracker re-measures every frame; forwarding an identical rect would
    // rebuild the native region 60 times a second for no reason.
    const ipc = captureIpc();
    setOccluder("a", R(10, 20));
    const after = ipc.posts;
    setOccluder("a", R(10, 20));
    setOccluder("a", R(10, 20));
    expect(ipc.posts).toBe(after);
  });

  it("does re-post when the element moves", () => {
    // Radix repositions on scroll and on collision — a stale rect would leave a lit
    // patch of WebView beside a sliced dropdown.
    const ipc = captureIpc();
    setOccluder("a", R(10, 20));
    const after = ipc.posts;
    setOccluder("a", R(10, 21));
    expect(ipc.posts).toBe(after + 1);
    expect(ipc.rects[ipc.rects.length - 1]).toEqual([{ x: 10, y: 21, w: 100, h: 50 }]);
  });

  it("drops a zero-area rectangle instead of registering it", () => {
    // An element measured before layout settles reports 0×0. It cannot occlude
    // anything, and a zero-area region would be a pointless native round-trip.
    setOccluder("a", { x: 10, y: 20, w: 0, h: 50 });
    expect(occluderRects()).toEqual([]);
  });

  it("treats a collapse to zero area as a release", () => {
    const ipc = captureIpc();
    setOccluder("a", R(10, 20));
    setOccluder("a", { x: 10, y: 20, w: 0, h: 0 });
    expect(occluderRects()).toEqual([]);
    expect(ipc.rects[ipc.rects.length - 1]).toEqual([]);
  });

  it("ignores a release for something never registered", () => {
    // Guarded rather than posting an empty list: an unpaired release from one
    // dropdown would otherwise reshape the hole out from under another.
    const ipc = captureIpc();
    setOccluder("real", R(10, 20));
    const after = ipc.posts;
    clearOccluder("never-registered");
    expect(ipc.posts).toBe(after);
    expect(occluderRects()).toHaveLength(1);
  });

  it("survives a double release", () => {
    const ipc = captureIpc();
    setOccluder("a", R(10, 20));
    clearOccluder("a");
    const after = ipc.posts;
    clearOccluder("a");
    expect(ipc.posts).toBe(after);
  });

  it("does nothing when there is no IPC host", () => {
    // A plain browser and vitest both have no hole to reshape, so silence is
    // correct rather than a swallowed failure.
    expect(() => setOccluder("a", R(10, 20))).not.toThrow();
    expect(occluderRects()).toHaveLength(1);
  });
});

describe("trackOccluder", () => {
  const elementOf = (rect: Partial<DOMRect>) =>
    ({ getBoundingClientRect: () => rect as DOMRect }) as Element;

  it("measures the element and registers it", () => {
    const ok = trackOccluder("a", elementOf({ left: 5, top: 6, width: 70, height: 80 }));
    expect(ok).toBe(true);
    expect(occluderRects()).toEqual([{ x: 5, y: 6, w: 70, h: 80 }]);
  });

  it("releases and reports failure for a null element", () => {
    setOccluder("a", R(10, 20));
    expect(trackOccluder("a", null)).toBe(false);
    expect(occluderRects()).toEqual([]);
  });

  it("reports failure for an element that is not laid out yet", () => {
    expect(trackOccluder("a", elementOf({ left: 0, top: 0, width: 0, height: 0 }))).toBe(false);
    expect(occluderRects()).toEqual([]);
  });
});
