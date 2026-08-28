import { describe, expect, it } from "vitest";
import { EASE_CHOICES, easeValue } from "./ease";

/** These sample points are the same ones `the_ease_curves_are_actually_distinct`
 *  asserts on the Rust side. The two files are hand-mirrored with nothing linking
 *  them, so this is what catches a drift — and a drifted preview is worse than no
 *  preview, because it teaches an author the wrong shape. */
describe("ease curves", () => {
  it("matches the Rust curve at a quarter of the way through", () => {
    // Ease out is already well past a quarter: it starts fast.
    expect(easeValue("Cubic", 0.25)).toBeGreaterThan(0.5);
    expect(easeValue("Quad", 0.25)).toBeGreaterThan(0.4);
    // Ease in has barely left the start.
    expect(easeValue("CubicIn", 0.25)).toBeLessThan(0.02);
    expect(easeValue("QuadIn", 0.25)).toBeLessThan(0.07);
    expect(easeValue("Linear", 0.25)).toBeCloseTo(0.25, 6);
    // In-out is slow at both ends, so a quarter in it lags linear.
    expect(easeValue("CubicInOut", 0.25)).toBeLessThan(0.25);
    expect(easeValue("QuadInOut", 0.25)).toBeLessThan(0.25);
  });

  it("draws every curve as a single rise from start to target", () => {
    // The property the thumbnails exist to show. A curve that dipped would be a
    // picture of the very thing "in-out" was wrongly assumed to mean.
    for (const { value } of EASE_CHOICES) {
      expect(easeValue(value, 0)).toBeCloseTo(0, 6);
      expect(easeValue(value, 1)).toBeCloseTo(1, 6);
      let previous = 0;
      for (let step = 0; step <= 100; step++) {
        const current = easeValue(value, step / 100);
        expect(current).toBeGreaterThanOrEqual(previous - 1e-6);
        previous = current;
      }
    }
  });

  it("previews an unknown curve as the fallback the runtime will play", () => {
    // `ease_curve_from_dto` falls back to Cubic, so the graph must too — otherwise
    // a document from a newer editor previews as one curve and plays as another.
    expect(easeValue("SomeFutureCurve", 0.25)).toBeCloseTo(easeValue("Cubic", 0.25), 6);
  });
});
