import { useState } from "react";
import type { EditorCommand, EditorSnapshot, TaskDto, TaskState } from "../types/bridge";

/** One place for every slow background job, replacing the export-only bar.
 *
 *  The bar it replaces could only say "exporting", could not be cancelled, and
 *  vanished the instant the job ended — so a failure that landed while the author
 *  was looking at the viewport left nothing behind but a toast that had already
 *  faded. Finished tasks stay here until dismissed for exactly that reason. */

const STATE_STYLE: Record<TaskState, { dot: string; label: string }> = {
  Queued:     { dot: "var(--overlay0)", label: "var(--subtext0)" },
  Running:    { dot: "var(--blue)",     label: "var(--blue)" },
  Cancelling: { dot: "var(--peach)",    label: "var(--peach)" },
  Done:       { dot: "var(--green)",    label: "var(--green)" },
  Failed:     { dot: "var(--red)",      label: "var(--red)" },
  Cancelled:  { dot: "var(--overlay0)", label: "var(--subtext0)" },
};

function TaskRow({ task, send }: { task: TaskDto; send: (c: EditorCommand) => void }) {
  const style = STATE_STYLE[task.state] ?? STATE_STYLE.Queued;
  const showBar = task.state === "Running" || task.state === "Cancelling";

  return (
    <div className="task-row">
      <span className="task-dot" style={{ background: style.dot }} />
      <div className="task-body">
        <div className="task-line">
          <span className="task-label" title={task.label}>{task.label}</span>
          <span className="task-state" style={{ color: style.label }}>{task.state}</span>
        </div>
        {showBar && (
          /* An indeterminate bar when the job cannot honestly report a fraction.
           * A cargo build has no meaningful percentage, and a bar that jumps to
           * 90% and sits there is worse than one that admits it does not know. */
          <div className={`task-bar${task.progress === null ? " task-bar--indeterminate" : ""}`}>
            <div className="task-bar-fill"
                 style={task.progress !== null ? { width: `${Math.round(task.progress * 100)}%` } : undefined} />
          </div>
        )}
        {task.detail && (
          <div className="task-detail" style={task.state === "Failed" ? { color: "var(--red)" } : undefined}>
            {task.detail}
          </div>
        )}
      </div>
      {task.active ? (
        <button className="task-action"
          disabled={task.state === "Cancelling"}
          title={task.state === "Cancelling" ? "Stopping…" : "Cancel this task"}
          onClick={() => send({ type: "CancelTask", payload: { id: task.id } })}>
          Cancel
        </button>
      ) : (
        <button className="task-action" title="Dismiss"
          onClick={() => send({ type: "DismissTask", payload: { id: task.id } })}>
          ✕
        </button>
      )}
    </div>
  );
}

export function TaskStrip({ snapshot, send }: {
  snapshot: EditorSnapshot;
  send: (c: EditorCommand) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const tasks = snapshot.tasks ?? [];
  if (tasks.length === 0) return null;

  const active = tasks.filter(t => t.active);
  const failed = tasks.filter(t => t.state === "Failed");
  const anyFinished = tasks.some(t => t.finished);

  // A failure is the one thing worth opening the list unprompted for; anything
  // else the author asked for and is already expecting.
  const summary = active.length > 0
    ? (active.length === 1 ? active[0].label : `${active.length} tasks running`)
    : failed.length > 0
      ? `${failed.length} task${failed.length > 1 ? "s" : ""} failed`
      : `${tasks.length} finished`;

  const accent = active.length > 0 ? "var(--blue)" : failed.length > 0 ? "var(--red)" : "var(--overlay0)";

  return (
    <div className="task-strip" style={{ borderBottomColor: accent }}>
      <button className="task-strip-head" onClick={() => setExpanded(e => !e)}
        title={expanded ? "Collapse task list" : "Show task list"}>
        {active.length > 0
          ? <span className="export-spinner" />
          : <span className="task-dot" style={{ background: accent }} />}
        <span className="task-summary" style={{ color: accent }}>{summary}</span>
        <span className="task-chevron">{expanded ? "▾" : "▸"}</span>
      </button>
      {anyFinished && (
        <button className="task-clear" title="Dismiss every finished task"
          onClick={() => send({ type: "DismissFinishedTasks" })}>
          Clear finished
        </button>
      )}
      {expanded && (
        <div className="task-list">
          {tasks.map(t => <TaskRow key={t.id} task={t} send={send} />)}
        </div>
      )}
    </div>
  );
}
