import { useEffect } from "react";
import {
  acquireViewportHoleSuppression,
  releaseViewportHoleSuppression,
} from "../../lib/viewportHole";

/** A destructive-action confirmation, in the app rather than from the OS.
 *
 * `window.confirm` cannot be used here, and not merely for looks: the viewport is
 * a hole punched through the window so the 3-D scene shows through, and a native
 * dialog paints *behind* that hole — the author gets a modal they cannot see,
 * blocking an app that appears frozen. Every in-app modal therefore suppresses the
 * hole while it is open, which is what `KeyboardShortcutsModal` does too.
 *
 * It is also synchronous: `confirm()` blocks the webview, so anything mid-flight
 * stalls behind a dialog nobody can find.
 */
export function ConfirmDialog({
  message,
  detail,
  confirmLabel = "Delete",
  onConfirm,
  onCancel,
}: {
  message: string;
  /** The consequence, when there is one worth stating before the click. */
  detail?: string;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  useEffect(() => {
    acquireViewportHoleSuppression();
    return () => releaseViewportHoleSuppression();
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Stopped from propagating: the editor binds Delete, Escape and friends
      // globally, and a confirmation is exactly when those must not also reach
      // the scene behind it.
      e.stopPropagation();
      if (e.key === "Escape") onCancel();
      if (e.key === "Enter") onConfirm();
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [onCancel, onConfirm]);

  return (
    <div className="kb-overlay" onClick={onCancel}>
      <div className="kb-modal" style={{ maxWidth: 380 }} onClick={e => e.stopPropagation()}>
        <div className="kb-header">
          <span>{message}</span>
        </div>
        <div className="kb-body">
          {detail && <div className="text-[11px] text-overlay0 mb-2">{detail}</div>}
          <div className="flex gap-2 justify-end">
            <button className="tb-btn text-[11px] px-3 py-1" onClick={onCancel}>
              Cancel
            </button>
            <button className="tb-btn text-red text-[11px] px-3 py-1" onClick={onConfirm}>
              {confirmLabel}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
