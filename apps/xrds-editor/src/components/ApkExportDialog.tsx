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
  const [refusal, setRefusal]   = useState<string | null>(null);
  const logRef = useRef<HTMLDivElement>(null);
  /** Highest APK task id that existed *before* this dialog started a build.
   *  Anything above it is ours. */
  const priorTaskId = useRef(0);
  /** Frames spent in "building" with no task yet — see the refusal effect. */
  const waitFrames = useRef(0);

  // The build this dialog started, once the backend has actually created it.
  const apkTask = snapshot.tasks?.find(t => t.tag === "export-apk" && t.id > priorTaskId.current) ?? null;

  // Detect a refused export.
  //
  // `ExportApk` has five guards that reject before any task is created — a build
  // already running, an unsaved scene, an uncreatable output or staging dir, an
  // unwritable scene.json. Each sets a status toast and returns, but this dialog
  // is modal and the toast renders behind it, so the author saw a permanent
  // "Starting…" with the reason computed and thrown away.
  //
  // The status message is the signal; the frame count is the backstop for a
  // future refusal path that forgets to set one. Both beat waiting forever.
  useEffect(() => {
    if (phase !== "building" || apkTask) { waitFrames.current = 0; return; }
    if (snapshot.status_message) {
      setRefusal(snapshot.status_message);
      setPhase("ready");
      return;
    }
    waitFrames.current += 1;
    if (waitFrames.current > 180) {
      setRefusal("The editor did not start the build and gave no reason. Check the editor log.");
      setPhase("ready");
    }
    // `snapshot` is a fresh object per frame, which is what makes the frame
    // count advance; individual fields would sit unchanged and never re-fire.
  }, [phase, apkTask, snapshot]);

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
  //
  // Watches the task itself, not the absence of a "building" flag. The flag
  // version closed the dialog the instant Export was clicked: `phase` became
  // "building" synchronously, but the very next snapshot still had
  // `is_exporting_apk === false` because the backend had not yet drained the
  // command — and "not started yet" is indistinguishable from "finished" when all
  // you have is a boolean going low. Task ids are monotonic, so `> priorTaskId`
  // also rules out a finished build from earlier in the session.
  useEffect(() => {
    if (phase !== "building" || !apkTask?.finished) return;
    setDoneMsg(apkTask.state === "Cancelled"
      ? "APK export cancelled."
      : apkTask.state === "Failed"
        ? `APK export failed: ${apkTask.detail ?? "see log"}`
        : apkTask.detail ?? "Export complete.");
    setPhase("done");
  }, [phase, apkTask?.id, apkTask?.state, apkTask?.finished]);

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
  // Mirrors the backend's own guards. `has_save_path` was missing, so a scene
  // that had never been saved left the button enabled and the export refused.
  const canExport = phase === "ready" && allOk && outputDir !== ""
    && !snapshot.is_dirty && snapshot.has_save_path;
  // Read from the task's own state rather than sniffing the message text — a
  // cancelled build is not a failure, and matching on a prefix would call it one.
  const isError   = apkTask?.state === "Failed";

  async function pickDir() {
    const dir = await onPickFolder();
    if (dir) {
      setOutputDir(dir);
      localStorage.setItem("apk_export_dir", dir);
    }
  }

  function startExport() {
    if (!canExport) return;
    // Everything with this tag that exists right now belongs to an earlier build.
    priorTaskId.current = Math.max(
      0, ...(snapshot.tasks ?? []).filter(t => t.tag === "export-apk").map(t => t.id));
    setRefusal(null);
    waitFrames.current = 0;
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

          {(snapshot.is_dirty || !snapshot.has_save_path) && (
            <div className="apk-warn">
              ⚠ {snapshot.has_save_path
                   ? "Save the scene before exporting."
                   : "This scene has never been saved. Save it before exporting."}
            </div>
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

          {refusal && (
            <div className="apk-warn">⚠ {refusal}</div>
          )}

          {(phase === "building" || phase === "done") && (
            <>
              <div className="apk-section-title apk-section-gap">
                Build log
                {/* The task's own state, so "the backend never started it" and
                  * "it started and has not printed yet" stop looking identical —
                  * both used to read as a motionless "Starting…". */}
                {apkTask && <span className="apk-task-state"> · {apkTask.state}</span>}
              </div>
              <div className="apk-log" ref={logRef}>
                {snapshot.apk_build_log.length === 0 ? (
                  <span className="apk-log-empty">
                    {apkTask ? `${apkTask.state}, no output yet…` : "Waiting for the editor to start the build…"}
                  </span>
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
              {/* While building, this stops the build rather than being greyed
                * out. An APK build runs for minutes, and until the queue existed
                * there was no way to abandon one you had already realised was
                * pointed at the wrong folder. */}
              <button className="apk-btn"
                onClick={phase === "building"
                  ? () => { if (apkTask) send({ type: "CancelTask", payload: { id: apkTask.id } }); }
                  : onClose}
                disabled={phase === "building" && (!apkTask || apkTask.state === "Cancelling")}>
                {phase === "building"
                  ? (apkTask?.state === "Cancelling" ? "Stopping…" : "Stop build")
                  : "Cancel"}
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
