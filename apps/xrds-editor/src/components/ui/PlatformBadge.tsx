/** Marks a control that does not behave the same on every target.
 *
 * An author edits on a desktop and ships to a headset, so "works here" and "works
 * where this runs" are different claims and only one of them matters. Saying it
 * beside the control beats saying it in a document nobody has opened — this SDK's
 * recurring failure is a field that can be set and does nothing, and the platform
 * axis is one more road to it.
 *
 * Two kinds, deliberately worded differently, because they call for different
 * decisions:
 *
 * - `unsupported` — it will do **nothing** there. Bloom on a headset: the field
 *   saves, round-trips, and never reaches a camera that could use it.
 * - `costly` — it works perfectly and costs more than the frame. Procedural
 *   atmosphere: correct sky, correct sun, 32 fps.
 *
 * The second is the more dangerous, because it looks like success until someone
 * puts the headset on.
 *
 * See `docs/platform-support.md` for the full table and its evidence.
 */
export function PlatformBadge({
  kind,
  where,
  detail,
}: {
  kind: "unsupported" | "costly";
  /** The targets this applies to, in an author's words: "standalone Quest". */
  where: string;
  /** Why, in one clause. Shown on hover. */
  detail: string;
}) {
  const label = kind === "unsupported" ? `no effect on ${where}` : `costly on ${where}`;
  return (
    <span
      title={detail}
      className="ml-1 px-1 rounded text-[9px] font-normal align-middle"
      style={{
        color: kind === "unsupported" ? "var(--red)" : "var(--yellow)",
        border: `1px solid ${kind === "unsupported" ? "var(--red)" : "var(--yellow)"}`,
        opacity: 0.85,
      }}
    >
      {label}
    </span>
  );
}
