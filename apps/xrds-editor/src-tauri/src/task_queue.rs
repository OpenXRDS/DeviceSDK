//! A queue for slow, author-initiated background work.
//!
//! Phase A of `docs/editor-task-queue-and-hdr-conversion.md`. This replaces two
//! bespoke job implementations (`ExportJob`, `ApkExportJob`) that each carried
//! their own `Arc<Mutex<Option<Result<..>>>>`, their own snapshot boolean, and
//! their own UI, and neither of which could be cancelled or report progress.
//!
//! # Scope: author-initiated work only
//!
//! **This queue is for visible work the author is waiting on** — exporting,
//! building, converting, baking. Slow, few, and started by a deliberate act.
//!
//! It is the wrong tool for anything high-frequency or system-initiated, and the
//! boundary matters because the two look alike. Terrain is the case that makes it
//! concrete: *importing* a heightmap belongs here, *streaming chunks as the player
//! moves* does not. Three properties that are features here are ruinous there —
//! finished tasks linger until dismissed (which would bury one real failure under
//! thousands of successes), the order is FIFO (wrong when the chunk ahead of you
//! matters more than the one behind), and every task is announced in the UI.
//!
//! A shared thread pool underneath would be fine. A shared queue *with a UI* is
//! not. If streaming ever needs scheduling it needs its own: priority-ordered,
//! cancellable, and silent.
//!
//! # Lanes
//!
//! Tasks are serialised per lane rather than globally, because the reason two jobs
//! must not overlap is specific to what they do. `Build` is capped at 1 because
//! cargo takes a lock on the target directory: a desktop export and an APK build
//! launched together do not run in parallel today, they run *one at a time while
//! both claim to be running*, which reads as a hang. Making the second one
//! `Queued` states what is actually happening.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Beyond this a log is not diagnostic, it is a symptom. The head is kept rather
/// than the tail: build failures are usually explained by the configuration at the
/// top, and a truncation that silently discarded it would be worse than none.
const MAX_LOG_LINES: usize = 20_000;

/// How many lines of a task's log the snapshot carries. The full log still goes to
/// disk for builds; this is only what the UI can usefully show.
pub const LOG_TAIL_LINES: usize = 200;

// ---------------------------------------------------------------------------
// Lanes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskLane {
    /// Compiles and packages. Capped at 1 — cargo serialises on the target-dir
    /// lock anyway, and doing it visibly is better than appearing to hang.
    Build,
    /// Environment-map conversion and similar. Capped at 1 because these are
    /// memory-heavy; six at once because someone dropped in a folder would be
    /// worse than six in a row.
    Convert,
    /// Everything else — cheap, parallel-safe work.
    General,
}

impl TaskLane {
    pub fn max_concurrent(self) -> usize {
        match self {
            TaskLane::Build | TaskLane::Convert => 1,
            TaskLane::General => 4,
        }
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Running,
    /// Cancellation requested, worker not yet stopped.
    ///
    /// A distinct state rather than jumping straight to `Cancelled`, because a
    /// cargo build does not stop the instant it is asked. Reporting "Cancelled"
    /// while the compiler is still holding the target-dir lock would make the
    /// next queued build look stuck for no visible reason.
    Cancelling,
    Done,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn is_finished(self) -> bool {
        matches!(self, TaskState::Done | TaskState::Failed | TaskState::Cancelled)
    }
    pub fn is_active(self) -> bool {
        matches!(self, TaskState::Queued | TaskState::Running | TaskState::Cancelling)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            TaskState::Queued => "Queued",
            TaskState::Running => "Running",
            TaskState::Cancelling => "Cancelling",
            TaskState::Done => "Done",
            TaskState::Failed => "Failed",
            TaskState::Cancelled => "Cancelled",
        }
    }
}

// ---------------------------------------------------------------------------
// Worker-facing handle
// ---------------------------------------------------------------------------

/// The half of a task that the worker thread writes and the main thread reads.
#[derive(Default)]
pub struct TaskShared {
    progress: Mutex<Option<f32>>,
    detail: Mutex<Option<String>>,
    log: Mutex<Vec<String>>,
    log_truncated: AtomicBool,
    result: Mutex<Option<Result<String, String>>>,
    cancel: AtomicBool,
}

impl TaskShared {
    pub fn progress(&self) -> Option<f32> {
        *self.progress.lock().unwrap()
    }
    pub fn detail(&self) -> Option<String> {
        self.detail.lock().unwrap().clone()
    }
    pub fn log_tail(&self, n: usize) -> Vec<String> {
        let log = self.log.lock().unwrap();
        log[log.len().saturating_sub(n)..].to_vec()
    }
    pub fn log_snapshot(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
    pub fn is_cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Handed to the worker closure. Everything a background job needs to report on
/// itself, and the one thing it must cooperate with: cancellation.
#[derive(Clone)]
pub struct TaskContext {
    shared: Arc<TaskShared>,
}

impl TaskContext {
    /// `0.0..=1.0`. Leave unset for work that cannot honestly report a fraction —
    /// an indeterminate spinner beats a bar that jumps to 90% and sits there.
    pub fn set_progress(&self, fraction: f32) {
        *self.shared.progress.lock().unwrap() = Some(fraction.clamp(0.0, 1.0));
    }
    pub fn set_detail(&self, detail: impl Into<String>) {
        *self.shared.detail.lock().unwrap() = Some(detail.into());
    }
    pub fn log(&self, line: impl Into<String>) {
        let mut log = self.shared.log.lock().unwrap();
        if log.len() < MAX_LOG_LINES {
            log.push(line.into());
        } else if !self.shared.log_truncated.swap(true, Ordering::Relaxed) {
            log.push(format!("[log truncated at {MAX_LOG_LINES} lines]"));
        }
    }
    /// Workers must poll this. Nothing can stop a thread from outside.
    pub fn is_cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::Relaxed)
    }

    /// Everything logged so far, for a worker that also wants to write its log to
    /// disk — a build log is read after the editor has moved on, so the on-disk
    /// copy is the one that matters.
    pub fn log_snapshot(&self) -> Vec<String> {
        self.shared.log_snapshot()
    }

    /// Run a child process to completion, streaming both its streams into the
    /// task log, and killing it if the task is cancelled.
    ///
    /// This lives here rather than in each job because it is where cancellation
    /// actually has to bite: every slow task in this editor is slow because it is
    /// waiting on a compiler, and a cancel that cannot stop the compiler is
    /// decoration. Returns `Ok(true)` on a zero exit code.
    pub fn run_child(&self, mut cmd: std::process::Command) -> Result<bool, String> {
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("failed to start: {e}"))?;

        let stdout = child.stdout.take().expect("stdout piped above");
        let stderr = child.stderr.take().expect("stderr piped above");

        // Readers signal completion through a channel rather than being joined
        // directly, so the wait below can be bounded. See the drain step.
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let out_done = done_tx.clone();

        let out_ctx = self.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                out_ctx.log(line);
            }
            let _ = out_done.send(());
        });
        let err_ctx = self.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                // cargo and gradle write *all* diagnostics to stderr — progress,
                // warnings and errors alike. Tag only real errors, so a build log
                // does not present routine output as failure.
                let trimmed = line.trim_start();
                let is_error = trimmed.starts_with("error")
                    || trimmed.contains("error:")
                    || trimmed.contains("error[");
                err_ctx.log(if is_error { format!("[err] {line}") } else { line });
            }
            let _ = done_tx.send(());
        });

        // Poll rather than `wait()`, so a cancel is noticed while the child runs
        // rather than after it finishes — which would defeat the point.
        let mut killed = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if !killed && self.is_cancelled() {
                        self.log("[task] cancelled — stopping build");
                        let _ = child.kill();
                        killed = true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => break Err(format!("wait failed: {e}")),
            }
        };

        // Drain the readers, but never unconditionally.
        //
        // `kill()` stops the process we spawned and not its descendants: killing
        // cargo leaves its rustc children alive, and they inherited these pipe
        // handles. A plain `join()` therefore waits on a pipe that a grandchild
        // still holds open — which wedges this task in `Cancelling` forever and,
        // because a wedged task never leaves the lane, blocks every later build
        // behind it. That is not a hypothetical: it is why an APK export sat at
        // "Starting…" after a cancelled application export.
        //
        // So: wait a bounded moment for a clean drain, then move on. The readers
        // are detached and hold an `Arc`, so any straggling lines still land in
        // the log; they are merely late rather than lost.
        const DRAIN_GRACE: std::time::Duration = std::time::Duration::from_secs(2);
        let mut drained = 0;
        for _ in 0..2 {
            if done_rx.recv_timeout(DRAIN_GRACE).is_ok() {
                drained += 1;
            }
        }
        if drained < 2 {
            self.log("[task] output pipe still held by a surviving child process — \
                      continuing without it (later lines may arrive out of order)");
        }

        status.map(|s| s.success())
    }
}

// ---------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------

type TaskWork = Box<dyn FnOnce(TaskContext) -> Result<String, String> + Send + 'static>;

/// Identifies a task to code that needs to find *that specific* job again —
/// the APK dialog wants its own build log, not whatever build is running.
///
/// A stable tag rather than a label prefix: the label is user-facing text that
/// includes a path, so matching on it would make a UI lookup break the moment
/// someone reworded a string.
pub mod tag {
    pub const EXPORT_APP: &str = "export-app";
    pub const EXPORT_APK: &str = "export-apk";
    pub const ENV_CONVERT: &str = "env-convert";
}

pub struct EditorTask {
    pub id: u64,
    pub label: String,
    pub lane: TaskLane,
    pub state: TaskState,
    /// Stable identifier for lookups, independent of the display label.
    pub tag: Option<&'static str>,
    pub shared: Arc<TaskShared>,
    /// Present only while `Queued`; taken when the task starts.
    ///
    /// The `Mutex` is not for contention — only one thread ever touches this. It
    /// is what makes the closure `Sync`, which a Bevy `Resource` requires and a
    /// bare `Box<dyn FnOnce + Send>` is not. Demanding `+ Sync` on the closure
    /// instead would push that bound onto every job's captures for no reason.
    work: Option<Mutex<TaskWork>>,
}

impl EditorTask {
    pub fn progress(&self) -> Option<f32> {
        self.shared.progress()
    }
    /// The error text for a failed task, otherwise whatever step it last reported.
    pub fn detail(&self) -> Option<String> {
        self.shared.detail()
    }
}

/// What a task turned into, reported once by [`TaskQueue::pump`].
#[derive(Clone, Debug, PartialEq)]
pub struct TaskOutcome {
    pub id: u64,
    pub label: String,
    pub lane: TaskLane,
    pub state: TaskState,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Queue
// ---------------------------------------------------------------------------

#[derive(Default, bevy::prelude::Resource)]
pub struct TaskQueue {
    tasks: Vec<EditorTask>,
    next_id: u64,
}

impl TaskQueue {
    /// Enqueue work. It starts on the next [`pump`](Self::pump) if its lane has
    /// room, and otherwise waits — visibly.
    pub fn spawn(
        &mut self,
        label: impl Into<String>,
        lane: TaskLane,
        work: impl FnOnce(TaskContext) -> Result<String, String> + Send + 'static,
    ) -> u64 {
        self.spawn_tagged(label, lane, None, work)
    }

    /// As [`spawn`](Self::spawn), with a stable [`tag`] so specialised UI can find
    /// this task again.
    pub fn spawn_tagged(
        &mut self,
        label: impl Into<String>,
        lane: TaskLane,
        tag: Option<&'static str>,
        work: impl FnOnce(TaskContext) -> Result<String, String> + Send + 'static,
    ) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.tasks.push(EditorTask {
            id,
            label: label.into(),
            lane,
            state: TaskState::Queued,
            tag,
            shared: Arc::new(TaskShared::default()),
            work: Some(Mutex::new(Box::new(work))),
        });
        id
    }

    /// The most recent task with this tag, finished or not. The APK dialog uses
    /// this to keep showing a build log after the build has ended.
    pub fn latest_tagged(&self, tag: &str) -> Option<&EditorTask> {
        self.tasks.iter().rev().find(|t| t.tag == Some(tag))
    }

    pub fn tasks(&self) -> &[EditorTask] {
        &self.tasks
    }
    pub fn get(&self, id: u64) -> Option<&EditorTask> {
        self.tasks.iter().find(|t| t.id == id)
    }
    /// The active task in a lane, if any. Used by callers that must not enqueue a
    /// duplicate — a second APK export is a mistake, not a queue entry.
    pub fn active_in_lane(&self, lane: TaskLane) -> Option<&EditorTask> {
        self.tasks.iter().find(|t| t.lane == lane && t.state.is_active())
    }
    pub fn active_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.state.is_active()).count()
    }

    /// Ask a task to stop.
    ///
    /// A `Queued` task never started, so it is cancelled outright. A running one
    /// only gets the flag: the worker decides when to notice, and until it does
    /// the task reports `Cancelling` rather than pretending to be stopped.
    pub fn cancel(&mut self, id: u64) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        match task.state {
            TaskState::Queued => {
                task.work = None;
                task.state = TaskState::Cancelled;
                *task.shared.detail.lock().unwrap() = Some("Cancelled before it started".into());
                true
            }
            TaskState::Running => {
                task.shared.cancel.store(true, Ordering::Relaxed);
                task.state = TaskState::Cancelling;
                true
            }
            _ => false,
        }
    }

    /// Remove one finished task. Running tasks are never dismissed — that would
    /// leave a thread writing into something nothing is watching.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.tasks.len();
        self.tasks.retain(|t| !(t.id == id && t.state.is_finished()));
        self.tasks.len() != before
    }

    pub fn dismiss_finished(&mut self) {
        self.tasks.retain(|t| !t.state.is_finished());
    }

    /// Start what can start, and collect what has finished. Call once per frame.
    ///
    /// Finished tasks stay in the list — they are not returned and dropped. A
    /// failure that lands while the author is looking at the viewport must still
    /// be there when they look back; that is the standard failure of
    /// status-bar-only reporting.
    pub fn pump(&mut self) -> Vec<TaskOutcome> {
        let mut outcomes = Vec::new();

        // ── Collect finished workers ──────────────────────────────────────
        for task in self.tasks.iter_mut() {
            if !matches!(task.state, TaskState::Running | TaskState::Cancelling) {
                continue;
            }
            // `try_lock` so a worker mid-write never stalls the frame.
            let Ok(mut guard) = task.shared.result.try_lock() else {
                continue;
            };
            let Some(result) = guard.take() else { continue };
            drop(guard);

            let cancelled = task.shared.is_cancel_requested();
            let (state, message) = match result {
                // A cancelled job that exits non-zero because we killed it is
                // cancelled, not failed. Reporting "Build failed" for a stop the
                // author asked for would be a lie about their own action.
                _ if cancelled => (TaskState::Cancelled, format!("{} cancelled", task.label)),
                Ok(msg) => (TaskState::Done, msg),
                Err(msg) => (TaskState::Failed, msg),
            };
            task.state = state;
            *task.shared.detail.lock().unwrap() = Some(message.clone());
            *task.shared.progress.lock().unwrap() = None;
            outcomes.push(TaskOutcome {
                id: task.id,
                label: task.label.clone(),
                lane: task.lane,
                state,
                message,
            });
        }

        // ── Start queued work where its lane has room ─────────────────────
        for lane in [TaskLane::Build, TaskLane::Convert, TaskLane::General] {
            let mut running = self
                .tasks
                .iter()
                .filter(|t| {
                    t.lane == lane && matches!(t.state, TaskState::Running | TaskState::Cancelling)
                })
                .count();

            for task in self.tasks.iter_mut() {
                if running >= lane.max_concurrent() {
                    break;
                }
                if task.lane != lane || task.state != TaskState::Queued {
                    continue;
                }
                let Some(work) = task.work.take() else { continue };
                let work = work.into_inner().expect("task work mutex is never poisoned");
                let shared = Arc::clone(&task.shared);
                let ctx = TaskContext {
                    shared: Arc::clone(&shared),
                };
                std::thread::spawn(move || {
                    let result = work(ctx);
                    *shared.result.lock().unwrap() = Some(result);
                });
                task.state = TaskState::Running;
                running += 1;
            }
        }

        outcomes
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive `pump` until the predicate holds, so tests never depend on how long
    /// a thread takes. Fails loudly rather than hanging CI.
    fn pump_until(q: &mut TaskQueue, mut done: impl FnMut(&TaskQueue) -> bool) -> Vec<TaskOutcome> {
        let mut all = Vec::new();
        for _ in 0..500 {
            all.extend(q.pump());
            if done(q) {
                return all;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("task queue did not reach the expected state within 5s");
    }

    #[test]
    fn a_task_runs_and_reports_success() {
        let mut q = TaskQueue::default();
        let id = q.spawn("Export", TaskLane::Build, |ctx| {
            ctx.set_detail("compiling");
            Ok("Exported to /tmp/out".into())
        });
        let outcomes = pump_until(&mut q, |q| q.get(id).unwrap().state.is_finished());

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].state, TaskState::Done);
        assert_eq!(outcomes[0].message, "Exported to /tmp/out");
        assert_eq!(q.get(id).unwrap().state, TaskState::Done);
    }

    #[test]
    fn a_failing_task_keeps_its_error_text() {
        let mut q = TaskQueue::default();
        let id = q.spawn("Convert", TaskLane::Convert, |_| Err("bad header".into()));
        pump_until(&mut q, |q| q.get(id).unwrap().state.is_finished());

        let task = q.get(id).unwrap();
        assert_eq!(task.state, TaskState::Failed);
        // The error must survive into the detail, not just the one-shot outcome —
        // this is what keeps a failure readable after the toast has gone.
        assert_eq!(task.detail().as_deref(), Some("bad header"));
    }

    /// The reason lanes exist. Two builds launched together must not both claim to
    /// be running when cargo will only let one proceed.
    #[test]
    fn a_lane_runs_one_task_at_a_time() {
        let mut q = TaskQueue::default();
        let gate = Arc::new(AtomicBool::new(false));
        let g = Arc::clone(&gate);

        let first = q.spawn("Build A", TaskLane::Build, move |_| {
            while !g.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok("A".into())
        });
        let second = q.spawn("Build B", TaskLane::Build, |_| Ok("B".into()));

        q.pump();
        assert_eq!(q.get(first).unwrap().state, TaskState::Running);
        assert_eq!(
            q.get(second).unwrap().state,
            TaskState::Queued,
            "the second build must wait visibly, not run into cargo's target-dir lock"
        );

        gate.store(true, Ordering::Relaxed);
        pump_until(&mut q, |q| q.get(second).unwrap().state.is_finished());
        assert_eq!(q.get(first).unwrap().state, TaskState::Done);
        assert_eq!(q.get(second).unwrap().state, TaskState::Done);
    }

    /// Different lanes are independent — a long build must not hold up a
    /// conversion that shares nothing with it.
    #[test]
    fn separate_lanes_run_concurrently() {
        let mut q = TaskQueue::default();
        let gate = Arc::new(AtomicBool::new(false));
        let g = Arc::clone(&gate);

        let build = q.spawn("Build", TaskLane::Build, move |_| {
            while !g.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok("built".into())
        });
        let convert = q.spawn("Convert", TaskLane::Convert, |_| Ok("converted".into()));

        pump_until(&mut q, |q| q.get(convert).unwrap().state.is_finished());
        assert_eq!(q.get(build).unwrap().state, TaskState::Running);
        gate.store(true, Ordering::Relaxed);
        pump_until(&mut q, |q| q.get(build).unwrap().state.is_finished());
    }

    #[test]
    fn cancelling_a_queued_task_stops_it_before_it_starts() {
        let mut q = TaskQueue::default();
        let ran = Arc::new(AtomicBool::new(false));
        let gate = Arc::new(AtomicBool::new(false));
        let g = Arc::clone(&gate);
        let r = Arc::clone(&ran);

        let first = q.spawn("Build A", TaskLane::Build, move |_| {
            while !g.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok("A".into())
        });
        let second = q.spawn("Build B", TaskLane::Build, move |_| {
            r.store(true, Ordering::Relaxed);
            Ok("B".into())
        });

        q.pump();
        assert!(q.cancel(second));
        gate.store(true, Ordering::Relaxed);
        pump_until(&mut q, |q| q.get(first).unwrap().state.is_finished());
        q.pump();

        assert_eq!(q.get(second).unwrap().state, TaskState::Cancelled);
        assert!(
            !ran.load(Ordering::Relaxed),
            "a task cancelled while queued must never run"
        );
    }

    /// The distinction that keeps the UI honest: a running task that has been
    /// asked to stop is `Cancelling`, not `Cancelled`, until its thread ends.
    #[test]
    fn a_running_task_reports_cancelling_until_it_actually_stops() {
        let mut q = TaskQueue::default();
        let id = q.spawn("Build", TaskLane::Build, |ctx| {
            while !ctx.is_cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            // Exactly what a killed child process produces, and the reason the
            // outcome must not be derived from the result alone.
            Err("exited with code 1".into())
        });

        q.pump();
        assert!(q.cancel(id));
        assert_eq!(q.get(id).unwrap().state, TaskState::Cancelling);

        let outcomes = pump_until(&mut q, |q| q.get(id).unwrap().state.is_finished());
        assert_eq!(q.get(id).unwrap().state, TaskState::Cancelled);
        assert_eq!(
            outcomes.last().unwrap().state,
            TaskState::Cancelled,
            "a build killed on request is cancelled, not failed"
        );
    }

    /// The lane must always come back.
    ///
    /// A task that never finishes holds its lane forever, and every later build is
    /// refused with "Busy" — which is what an APK export stuck at "Starting…"
    /// actually was. Whatever `run_child` does about surviving grandchildren, the
    /// invariant it exists to protect is this one.
    #[test]
    fn a_cancelled_build_releases_its_lane_for_the_next_one() {
        let mut q = TaskQueue::default();
        let first = q.spawn_tagged("Export application", TaskLane::Build, Some(tag::EXPORT_APP), |ctx| {
            while !ctx.is_cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err("exited with code 1".into())
        });
        q.pump();
        q.cancel(first);
        pump_until(&mut q, |q| q.get(first).unwrap().state.is_finished());

        assert!(
            q.active_in_lane(TaskLane::Build).is_none(),
            "a cancelled build must not keep holding the Build lane"
        );

        let second = q.spawn_tagged("Export APK", TaskLane::Build, Some(tag::EXPORT_APK), |_| {
            Ok("apk".into())
        });
        pump_until(&mut q, |q| q.get(second).unwrap().state.is_finished());
        assert_eq!(q.get(second).unwrap().state, TaskState::Done);
    }

    #[test]
    fn finished_tasks_linger_until_dismissed() {
        let mut q = TaskQueue::default();
        let id = q.spawn("Convert", TaskLane::Convert, |_| Err("nope".into()));
        pump_until(&mut q, |q| q.get(id).unwrap().state.is_finished());

        // Several frames pass with the author looking elsewhere.
        for _ in 0..5 {
            q.pump();
        }
        assert!(q.get(id).is_some(), "a failure must survive until dismissed");

        assert!(q.dismiss(id));
        assert!(q.get(id).is_none());
    }

    #[test]
    fn a_running_task_cannot_be_dismissed() {
        let mut q = TaskQueue::default();
        let gate = Arc::new(AtomicBool::new(false));
        let g = Arc::clone(&gate);
        let id = q.spawn("Build", TaskLane::Build, move |_| {
            while !g.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok("done".into())
        });
        q.pump();

        assert!(!q.dismiss(id));
        assert!(q.get(id).is_some());
        gate.store(true, Ordering::Relaxed);
        pump_until(&mut q, |q| q.get(id).unwrap().state.is_finished());
    }

    #[test]
    fn progress_and_log_reach_the_reader() {
        let mut q = TaskQueue::default();
        let id = q.spawn("Convert", TaskLane::Convert, |ctx| {
            for face in 0..6 {
                ctx.set_progress(face as f32 / 6.0);
                ctx.log(format!("face {face}"));
            }
            ctx.set_detail("writing ktx2");
            Ok("converted".into())
        });
        pump_until(&mut q, |q| q.get(id).unwrap().state.is_finished());

        let task = q.get(id).unwrap();
        assert_eq!(task.shared.log_snapshot().len(), 6);
        assert_eq!(task.shared.log_tail(2), vec!["face 4", "face 5"]);
        // Cleared on completion: a bar frozen at 83% next to "Done" is noise.
        assert_eq!(task.progress(), None);
    }

    #[test]
    fn the_log_truncates_at_the_cap_and_says_so() {
        let shared = Arc::new(TaskShared::default());
        let ctx = TaskContext {
            shared: Arc::clone(&shared),
        };
        for i in 0..(MAX_LOG_LINES + 50) {
            ctx.log(format!("line {i}"));
        }
        let log = shared.log_snapshot();
        assert_eq!(log.len(), MAX_LOG_LINES + 1);
        // The head survives, because that is where a build's configuration — and
        // usually its actual mistake — is recorded.
        assert_eq!(log[0], "line 0");
        assert!(log.last().unwrap().contains("truncated"));
    }

    /// What the APK dialog depends on. It must find *its* build, and a finished
    /// one from earlier in the session must not be mistaken for it — the bug that
    /// closed the dialog the instant Export was clicked was exactly this kind of
    /// "is something running?" question standing in for "is my thing done?".
    #[test]
    fn latest_tagged_returns_the_newest_run_and_ids_increase() {
        let mut q = TaskQueue::default();
        let first = q.spawn_tagged("Export APK → a", TaskLane::Build, Some(tag::EXPORT_APK), |_| {
            Ok("a".into())
        });
        pump_until(&mut q, |q| q.get(first).unwrap().state.is_finished());

        let second = q.spawn_tagged("Export APK → b", TaskLane::Build, Some(tag::EXPORT_APK), |_| {
            Ok("b".into())
        });
        assert!(second > first, "ids must increase so a UI can tell runs apart");
        assert_eq!(q.latest_tagged(tag::EXPORT_APK).unwrap().id, second);
        // The finished one is still listed — it just is not the latest.
        assert!(q.get(first).is_some());
        assert!(q.latest_tagged(tag::EXPORT_APP).is_none());

        pump_until(&mut q, |q| q.get(second).unwrap().state.is_finished());
    }

    #[test]
    fn active_in_lane_finds_the_running_job_and_forgets_the_finished_one() {
        let mut q = TaskQueue::default();
        let id = q.spawn("Build", TaskLane::Build, |_| Ok("done".into()));
        q.pump();
        assert!(q.active_in_lane(TaskLane::Build).is_some());
        assert!(q.active_in_lane(TaskLane::Convert).is_none());

        pump_until(&mut q, |q| q.get(id).unwrap().state.is_finished());
        assert!(
            q.active_in_lane(TaskLane::Build).is_none(),
            "a finished task lingers for display but must not block the next one"
        );
    }
}
