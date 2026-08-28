/** Ease curves, mirroring `ease_out` in
 *  `crates/xrds-runtime/src/xrds_api/trigger_action.rs`.
 *
 *  Kept in step with it by hand, the same arrangement as `audioGain` and for the
 *  same reason: a preview that lies is worse than no preview. If the Rust changes,
 *  change this too — `the_ease_curves_are_actually_distinct` guards the Rust side
 *  and `ease.test.ts` guards this one at the same sample points, so the two tests
 *  disagreeing is the signal that they have drifted.
 *
 *  Every curve is **monotonic**: it runs once from 0 to 1 and stops. In / out /
 *  in-out describe only *when* the change is fast. This was misread once — "in-out"
 *  taken to mean the value returns to where it started — which is the reason the
 *  picker now draws the shape instead of naming the polynomial. */
export function easeValue(curve: string, t: number): number {
  switch (curve) {
    case "Linear": return t;
    case "Quad": return 1 - (1 - t) * (1 - t);
    case "Cubic": return 1 - Math.pow(1 - t, 3);
    case "QuadIn": return t * t;
    case "CubicIn": return t * t * t;
    case "QuadInOut":
      return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
    case "CubicInOut":
      return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
    // Matches `ease_curve_from_dto`'s fallback, so a curve this build does not
    // know still previews as the curve the runtime will actually play.
    default: return 1 - Math.pow(1 - t, 3);
  }
}

/** The pickable curves: wire name, a short label, and the sentence a hover shows.
 *
 *  Labels are deliberately terse — the thumbnail carries the meaning, so the text
 *  only has to distinguish neighbours. The longer explanation lives in `hint`,
 *  where it costs no space until someone wants it. */
export const EASE_CHOICES: { value: string; label: string; hint: string }[] = [
  { value: "Linear", label: "Linear", hint: "Steady — the same speed the whole way." },
  { value: "Quad", label: "Out", hint: "Starts fast, eases into the target." },
  { value: "Cubic", label: "Out+", hint: "Starts fast, eases in — more pronounced. Nearly there by halfway." },
  { value: "QuadIn", label: "In", hint: "Starts slow, accelerates into the target." },
  { value: "CubicIn", label: "In+", hint: "Starts slow, accelerates — more pronounced. Barely moved by halfway." },
  { value: "QuadInOut", label: "In-out", hint: "Slow at both ends, fast through the middle." },
  { value: "CubicInOut", label: "In-out+", hint: "Slow at both ends, fast through the middle — more pronounced." },
];
