import { EASE_CHOICES, easeValue } from "../../lib/ease";

/** One curve, drawn. Time runs left to right, the animated value bottom to top.
 *
 *  A dashed diagonal shows linear for comparison, so "faster than steady" and
 *  "slower than steady" are visible rather than remembered — that comparison is
 *  the entire difference between the in and out families, and it is invisible in
 *  a curve shown alone. */
function EaseThumb({ curve, active }: { curve: string; active: boolean }) {
  const W = 34, H = 24, PAD = 3;
  const points: string[] = [];
  for (let i = 0; i <= 24; i++) {
    const t = i / 24;
    const x = PAD + t * (W - PAD * 2);
    const y = H - PAD - easeValue(curve, t) * (H - PAD * 2);
    points.push(`${x.toFixed(1)},${y.toFixed(1)}`);
  }
  return (
    <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} style={{ display: "block" }} aria-hidden>
      <line
        x1={PAD} y1={H - PAD} x2={W - PAD} y2={PAD}
        stroke="var(--surface1)" strokeWidth={1} strokeDasharray="2 2"
      />
      <polyline
        points={points.join(" ")}
        fill="none"
        stroke={active ? "var(--blue)" : "var(--overlay0)"}
        strokeWidth={1.5}
        strokeLinecap="round"
      />
    </svg>
  );
}

/** Pick an ease curve by its shape rather than by the name of its polynomial.
 *
 *  This replaced a dropdown of names like "Cubic in-out", which was reported as
 *  unreadable and was: "in-out" got taken to mean the value goes out and comes
 *  *back*. Every thumbnail here rises from bottom-left to top-right and never
 *  turns down, which answers that in a glance — and no sentence had managed to.
 *
 *  Modelled on the audio falloff preview, for the same reason it exists: the
 *  authored numbers do not tell an author what they will get, and the picture
 *  does. */
export function EasePicker({ value, onChange }: {
  value: string;
  onChange: (curve: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1">
      {EASE_CHOICES.map(choice => {
        const active = choice.value === value;
        return (
          <button
            key={choice.value}
            type="button"
            title={choice.hint}
            aria-pressed={active}
            onClick={() => onChange(choice.value)}
            className="flex flex-col items-center gap-0.5 px-1 pt-1 pb-0.5 rounded border transition-colors"
            style={{
              borderColor: active ? "var(--blue)" : "var(--surface0)",
              background: active ? "var(--surface0)" : "var(--well)",
            }}
          >
            <EaseThumb curve={choice.value} active={active} />
            <span
              className="text-[8.5px] leading-none"
              style={{ color: active ? "var(--blue)" : "var(--overlay0)" }}
            >
              {choice.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
