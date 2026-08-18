import { beforeEach, describe, expect, it } from "vitest";
import {
  acquireViewportHoleSuppression,
  releaseViewportHoleSuppression,
  viewportHoleHolders,
  __resetViewportHoleForTests,
} from "./viewportHole";

/** Captures the `set_viewport_hole` messages actually posted over IPC. */
function installIpcSpy(): { enabled: boolean[] } {
  const seen: boolean[] = [];
  (globalThis as unknown as { ipc: unknown }).ipc = {
    postMessage: (m: string) => {
      const parsed = JSON.parse(m);
      if (parsed.type === "set_viewport_hole") seen.push(parsed.enabled);
    },
  };
  return { get enabled() { return seen; } };
}

beforeEach(() => {
  __resetViewportHoleForTests();
  installIpcSpy();
});

describe("viewport hole suppression", () => {
  it("closes the hole on the first acquire and reopens on the last release", () => {
    const spy = installIpcSpy();
    acquireViewportHoleSuppression();
    expect(spy.enabled).toEqual([false]);
    releaseViewportHoleSuppression();
    expect(spy.enabled).toEqual([false, true]);
  });

  it("does not reopen the hole while another holder still wants it shut", () => {
    // The case this exists for: a modal has the hole closed, a <Select> inside it
    // opens and then closes. Without counting, that close would reopen the hole
    // and the clip would slice the modal itself.
    const spy = installIpcSpy();
    acquireViewportHoleSuppression(); // modal
    acquireViewportHoleSuppression(); // dropdown inside it
    releaseViewportHoleSuppression(); // dropdown closes
    // Still shut: the modal holds it.
    expect(spy.enabled).toEqual([false]);
    expect(viewportHoleHolders()).toBe(1);

    releaseViewportHoleSuppression(); // modal closes
    expect(spy.enabled).toEqual([false, true]);
  });

  it("posts nothing on a nested acquire, so the region is not thrashed", () => {
    const spy = installIpcSpy();
    acquireViewportHoleSuppression();
    acquireViewportHoleSuppression();
    acquireViewportHoleSuppression();
    expect(spy.enabled).toEqual([false]);
  });

  it("ignores an unpaired release rather than going negative", () => {
    // A negative count is the dangerous state: the *next* acquire would not close
    // the hole, and a dropdown would then be silently clipped — much harder to
    // trace back than the stray release itself.
    const spy = installIpcSpy();
    releaseViewportHoleSuppression();
    expect(viewportHoleHolders()).toBe(0);
    expect(spy.enabled).toEqual([]);

    acquireViewportHoleSuppression();
    // The next acquire must still work.
    expect(spy.enabled).toEqual([false]);
  });

  it("survives an environment with no ipc bridge", () => {
    // vitest and a plain browser have no wry `ipc`. There is no hole there either,
    // so doing nothing is correct — it must not throw.
    delete (globalThis as unknown as { ipc?: unknown }).ipc;
    expect(() => {
      acquireViewportHoleSuppression();
      releaseViewportHoleSuppression();
    }).not.toThrow();
  });

  it("balances across an interleaved open/close pair", () => {
    // Two independent dropdowns overlapping in time, which happens when one is
    // opened from inside another's content.
    const spy = installIpcSpy();
    acquireViewportHoleSuppression(); // A opens
    acquireViewportHoleSuppression(); // B opens
    releaseViewportHoleSuppression(); // A closes
    releaseViewportHoleSuppression(); // B closes
    expect(spy.enabled).toEqual([false, true]);
    expect(viewportHoleHolders()).toBe(0);
  });
});

describe("the message shape the Rust side parses", () => {
  it("sends exactly set_viewport_hole with a boolean enabled", () => {
    // `wry_overlay.rs` reads `msg["enabled"].as_bool()`, defaulting to true. A
    // wrong key or a stringified boolean would silently mean "reopen the hole",
    // i.e. the clip stays and the dropdown is sliced.
    const posted: string[] = [];
    (globalThis as unknown as { ipc: unknown }).ipc = {
      postMessage: (m: string) => posted.push(m),
    };
    acquireViewportHoleSuppression();
    expect(JSON.parse(posted[0])).toEqual({ type: "set_viewport_hole", enabled: false });
    releaseViewportHoleSuppression();
    expect(JSON.parse(posted[1])).toEqual({ type: "set_viewport_hole", enabled: true });
  });
});
