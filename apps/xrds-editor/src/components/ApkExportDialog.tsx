import { acquireViewportHoleSuppression, releaseViewportHoleSuppression } from "../lib/viewportHole";
import { useEffect, useRef, useState } from "react";
import type { ApkPrerequisite, EditorCommand, EditorSnapshot } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  onPickFolder: () => Promise<string | null>;
  onClose: () => void;
}

type Phase = "checking" | "ready" | "building" | "done";

export function ApkExportDialog({ snapshot, send, onPickFolder, onClose }: Props) {
  const [prereqs, setPrereqs]   = useState<ApkPrerequisite[] | null>(null);
  const [outputDir, setOutputDir] = useState<string>(() => localStorage.getItem("apk_export_dir") ?? "");
  const [phase, setPhase]       = useState<Phase>("checking");
  const [doneMsg, setDoneMsg]   = useState("");
  const logRef = useRef<HTMLDivElement>(null);

  // Disable viewport hole and kick off prereq check.
  useEffect(() => {
    acquireViewportHoleSuppression();
    send({ type: "CheckApkPrerequisites" });
    return () => {
      releaseViewportHoleSuppression();
    };
  }, []);

  // Capture prereq results (one-shot from snapshot).
  useEffect(() => {
    if (snapshot.apk_prerequisites) {
      setPrereqs(snapshot.apk_prerequisites);
      if (phase === "checking") setPhase("ready");
    }
  }, [snapshot.apk_prerequisites]);

  // Detect build completion.
  useEffect(() => {
    if (phase === "building" && !snapshot.is_exporting_apk) {
      setDoneMsg(snapshot.status_message ?? "Export complete.");
      setPhase("done");
    }
  }, [snapshot.is_exporting_apk, snapshot.status_message]);

  // Auto-scroll log.
  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [snapshot.apk_build_log]);

  // Escape to close (except while building).
  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === "Escape" && phase !== "building") onClose(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [phase, onClose]);

  const allOk     = prereqs !== null && prereqs.every(p => p.ok);
  const canExport = phase === "ready" && allOk && outputDir !== "" && !snapshot.is_dirty;
  const isError   = doneMsg.startsWith("APK export failed");

  async function pickDir() {
    const dir = await onPickFolder();
    if (dir) {
      setOutputDir(dir);
      localStorage.setItem("apk_export_dir", dir);
    }
  }

  function startExport() {
    if (!canExport) return;
    setPhase("building");
    send({ type: "ExportApk", payload: { output_dir: outputDir } });
  }

  return (
    <div className="apk-overlay" onClick={phase !== "building" ? onClose : undefined}>
      <div className="apk-modal" onClick={e => e.stopPropagation()}>

        <div className="apk-header">
          <span>Export for Quest</span>
          {phase !== "building" && <button className="apk-close" onClick={onClose}>✕</button>}
        </div>

        <div className="apk-body">

          {snapshot.is_dirty && (
            <div className="apk-warn">⚠ Save the scene before exporting.</div>
          )}

          <div className="apk-section-title">Prerequisites</div>
          {prereqs === null ? (
            <div className="apk-checking">Checking environment…</div>
          ) : (
            <ul className="apk-prereq-list">
              {prereqs.map(p => (
                <li key={p.name} className={`apk-prereq ${p.ok ? "ok" : "fail"}`}>
                  <span className="apk-prereq-icon">{p.ok ? "✓" : "✗"}</span>
                  <div className="apk-prereq-info">
                    <span className="apk-prereq-name">{p.name}</span>
                    {!p.ok && <span className="apk-prereq-hint">{p.hint}</span>}
                  </div>
                </li>
              ))}
            </ul>
          )}

          <div className="apk-section-title apk-section-gap">Output Directory</div>
          <div className="apk-dir-row">
            <span className="apk-dir-path">{outputDir || "—"}</span>
            <button className="apk-btn" onClick={pickDir} disabled={phase === "building"}>
              Browse…
            </button>
          </div>

          {(phase === "building" || phase === "done") && (
            <>
              <div className="apk-section-title apk-section-gap">Build log</div>
              <div className="apk-log" ref={logRef}>
                {snapshot.apk_build_log.length === 0 ? (
                  <span className="apk-log-empty">Starting…</span>
                ) : (
                  snapshot.apk_build_log.map((line, i) => (
                    <div key={i} className={`apk-log-line${line.startsWith("[err]") ? " err" : ""}`}>
                      {line}
                    </div>
                  ))
                )}
              </div>
            </>
          )}

          {phase === "done" && (
            <div className={`apk-done-msg ${isError ? "fail" : "ok"}`}>{doneMsg}</div>
          )}

        </div>

        <div className="apk-footer">
          {phase === "done" ? (
            <button className="apk-btn primary" onClick={onClose}>Close</button>
          ) : (
            <>
              <button className="apk-btn" onClick={onClose} disabled={phase === "building"}>
                Cancel
              </button>
              <button className="apk-btn primary" onClick={startExport} disabled={!canExport}>
                {phase === "building"
                  ? <><span className="apk-spin" /> Building…</>
                  : "Export APK"}
              </button>
            </>
          )}
        </div>

      </div>
    </div>
  );
}
