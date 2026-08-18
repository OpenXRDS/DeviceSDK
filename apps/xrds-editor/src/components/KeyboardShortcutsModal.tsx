import { acquireViewportHoleSuppression, releaseViewportHoleSuppression } from "../lib/viewportHole";
import { useEffect } from "react";

interface Props {
  onClose: () => void;
}

const SHORTCUTS: { section: string; rows: [string, string][] }[] = [
  {
    section: "Viewport",
    rows: [
      ["Middle / Right drag",      "Orbit"],
      ["Shift + drag",             "Pan"],
      ["Scroll",                   "Zoom"],
      ["W / S",                    "Move forward / back"],
      ["A / D",                    "Move left / right"],
      ["Q / E",                    "Move down / up"],
    ],
  },
  {
    section: "Selection",
    rows: [
      ["Click",                    "Select object"],
      ["Ctrl + Click",             "Multi-select"],
      ["F5",                       "Play / Stop"],
      ["Escape",                   "Deselect all / stop play"],
      ["Delete / Backspace",       "Delete selection"],
    ],
  },
  {
    section: "Gizmo & Camera",
    rows: [
      ["T",                        "Translate gizmo"],
      ["R",                        "Rotate gizmo"],
      ["Y",                        "Scale gizmo"],
      ["F",                        "Frame selected"],
      ["G",                        "Toggle grid"],
    ],
  },
  {
    section: "Edit",
    rows: [
      ["Ctrl + Z",                 "Undo"],
      ["Ctrl + Y",                 "Redo"],
      ["Ctrl + C",                 "Copy"],
      ["Ctrl + X",                 "Cut"],
      ["Ctrl + V",                 "Paste"],
      ["Ctrl + D",                 "Duplicate"],
    ],
  },
  {
    section: "File",
    rows: [
      ["Ctrl + N",                 "New scene"],
      ["Ctrl + O",                 "Open scene"],
      ["Ctrl + S",                 "Save"],
      ["Ctrl + Shift + S",         "Save as…"],
      ["Ctrl + I",                 "Import asset…"],
      ["Ctrl + Shift + E",         "Export GLB…"],
      ["Ctrl + Shift + A",         "Export application…"],
    ],
  },
];

export function KeyboardShortcutsModal({ onClose }: Props) {
  useEffect(() => {
    // Remove the SetWindowRgn hole so the WebView paints over the Bevy viewport,
    // making the modal visible in the centre of the screen.
    acquireViewportHoleSuppression();
    return () => {
      releaseViewportHoleSuppression();
    };
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <div className="kb-overlay" onClick={onClose}>
      <div className="kb-modal" onClick={e => e.stopPropagation()}>
        <div className="kb-header">
          <span>Keyboard Shortcuts</span>
          <button className="kb-close" onClick={onClose}>✕</button>
        </div>
        <div className="kb-body">
          {SHORTCUTS.map(({ section, rows }) => (
            <div key={section} className="kb-section">
              <div className="kb-section-title">{section}</div>
              {rows.map(([key, desc]) => (
                <div key={key} className="kb-row">
                  <span className="kb-key">{key}</span>
                  <span className="kb-desc">{desc}</span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
