import { BRIDGE_VERSION, type EditorSnapshot } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
}

/** Hard banner shown when the Rust backend and this UI disagree about the
 * bridge schema version.
 *
 * ## Why this exists
 *
 * `src/types/bridge.ts` is a hand-written mirror of `src-tauri/src/bridge.rs`
 * with nothing linking them. Drift produces **no compile error on either
 * side** — `cargo check` passes, `tsc` passes, `vite build` passes, and every
 * test passes, because no test crosses the boundary. It then fails at runtime,
 * in two different and equally unhelpful ways:
 *
 * - **Outbound:** an unknown command is dropped Rust-side. `useSendCommand` is
 *   fire-and-forget, so nothing here ever learns. You click; nothing happens.
 * - **Inbound:** a removed snapshot field arrives as `undefined` and throws on
 *   the first `.map()`, giving a stack trace that names neither the field nor
 *   the bridge.
 *
 * This banner converts both into one sentence naming the actual cause.
 *
 * ## Why it renders *before* anything else
 *
 * A version mismatch means the snapshot shape is untrustworthy, so any panel
 * reading it may throw. This is rendered ahead of the editor and returns early,
 * so the message survives even when the rest of the UI cannot mount — which is
 * exactly the case where it is most needed.
 */
export function BridgeMismatchBanner({ snapshot }: Props) {
  // `0` is the default for a backend predating this field, which is itself a
  // mismatch worth reporting. Waiting for the first real snapshot avoids
  // flashing the banner against `defaultSnapshot` at startup — but
  // `defaultSnapshot` carries the correct version, so that case never trips.
  if (snapshot.bridge_version === BRIDGE_VERSION) return null;

  return (
    <div className="bridge-mismatch">
      <strong>Editor and UI are out of sync.</strong>
      <span>
        The backend speaks bridge version <code>{snapshot.bridge_version || "&lt;none&gt;"}</code>,
        this UI speaks <code>{BRIDGE_VERSION}</code>. Commands may be silently
        dropped and panels may fail to render.
      </span>
      <span className="bridge-mismatch-fix">
        Rebuild the frontend (<code>npm run build</code> in <code>apps/xrds-editor</code>)
        and restart the editor. If it persists, <code>BRIDGE_VERSION</code> was bumped in
        one of <code>src-tauri/src/bridge.rs</code> / <code>src/types/bridge.ts</code> but
        not the other.
      </span>
    </div>
  );
}
