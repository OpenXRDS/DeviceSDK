/*
Copyright 2025 KETI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! `XrdsNetTask<T>`: a poll-able handle to a still-running one-shot XrdsNet
//! call, so a blocking network operation can be kicked off and then checked
//! non-blockingly once per frame from `XrdsApp::update()` — see
//! `docs/done/xrds-net-devicesdk-integration.md`.
//!
//! The worker thread runs exactly one blocking call, sends the result once,
//! and exits. It is **detached** — never joined — so dropping the task (to
//! cancel a request, or discard a fire-and-forget `dispatch_async`) never
//! blocks the caller, even if the underlying call would take seconds. That's
//! the whole point: blocking-on-drop would defeat the "never stall the frame
//! loop" premise. (Trade-off: at process exit a worker mid-call isn't waited
//! on — acceptable, and never a hang.)

use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Mutex;

use super::error::NetError;

/// A background-thread-backed handle to a still-running XrdsNet call. Drain it
/// each frame with [`try_take`](XrdsNetTask::try_take).
///
/// The `Receiver` is wrapped in a `Mutex` purely so the whole task is `Sync`:
/// an `XrdsApp` (which holds tasks as fields) must be `Send + Sync + 'static`
/// for `Runtime::run_xrds`, and `mpsc::Receiver` is `Send` but not `Sync`. The
/// lock is never contended (the task lives in one place); it's a type-system
/// requirement, not real cross-thread sharing.
pub struct XrdsNetTask<T> {
    rx: Mutex<Receiver<Result<T, NetError>>>,
    done: bool,
}

impl<T: Send + 'static> XrdsNetTask<T> {
    pub(crate) fn spawn(f: impl FnOnce() -> Result<T, NetError> + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        // Detached on purpose — never joined. If the task is dropped first the
        // receiver is gone and `tx.send` just no-ops; the thread still winds
        // down on its own.
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        Self {
            rx: Mutex::new(rx),
            done: false,
        }
    }

    /// Non-blocking. `None` = still running. `Some(_)` = finished, and the
    /// **owned** result is handed to you (no clone needed for large bodies /
    /// frames); the task is then spent and every later call returns `None`.
    /// Poll this once per frame until you get `Some`.
    pub fn try_take(&mut self) -> Option<Result<T, NetError>> {
        if self.done {
            return None;
        }
        let result = self.rx.lock().unwrap().try_recv();
        match result {
            Ok(result) => {
                self.done = true;
                Some(result)
            }
            Err(TryRecvError::Empty) => None, // still running
            Err(TryRecvError::Disconnected) => {
                // Worker vanished without sending (e.g. it panicked mid-call).
                // Surface it once as an error rather than looking pending
                // forever, then mark spent.
                self.done = true;
                Some(Err(NetError::Network(
                    "network task ended without producing a result".to_string(),
                )))
            }
        }
    }
}

impl<T> std::fmt::Debug for XrdsNetTask<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XrdsNetTask {{ done: {} }}", self.done)
    }
}

/// Ergonomic drain for a task held in an `Option` field — the common one-shot
/// pattern. Lets `if let Some(result) = self.field.take_ready()` replace the
/// poll-then-null-the-slot dance.
pub trait NetTaskSlot<T> {
    /// Non-blocking. `Some(_)` exactly once, when the task finishes — and the
    /// slot is reset to `None` for you (can't forget it, and a later frame
    /// won't re-handle a spent task). `None` while pending, already taken, or
    /// the slot is empty.
    fn take_ready(&mut self) -> Option<Result<T, NetError>>;
}

impl<T: Send + 'static> NetTaskSlot<T> for Option<XrdsNetTask<T>> {
    fn take_ready(&mut self) -> Option<Result<T, NetError>> {
        let result = self.as_mut()?.try_take()?;
        *self = None; // spent — clear the slot
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};

    /// Spin (non-blocking) until the task yields a result, with a generous
    /// timeout so a genuinely stuck task fails the test instead of hanging it.
    fn wait_take<T: Send + 'static>(task: &mut XrdsNetTask<T>) -> Result<T, NetError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = task.try_take() {
                return result;
            }
            assert!(Instant::now() < deadline, "task never completed");
            std::thread::yield_now();
        }
    }

    #[test]
    fn task_is_send_and_sync() {
        // `XrdsApp` (which holds tasks as fields) must be Send + Sync for
        // `Runtime::run_xrds`; guard that the task keeps satisfying it.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<XrdsNetTask<i32>>();
    }

    #[test]
    fn try_take_is_pending_then_owns_the_result_once_then_spent() {
        let (gate_tx, gate_rx) = channel::<()>();
        let mut task = XrdsNetTask::spawn(move || {
            gate_rx.recv().unwrap(); // block until the test releases it
            Ok::<i32, NetError>(42)
        });

        // Non-blocking while pending.
        assert!(task.try_take().is_none());

        gate_tx.send(()).unwrap(); // let the worker finish
        assert_eq!(wait_take(&mut task).unwrap(), 42);

        // Spent — every later call is None.
        assert!(task.try_take().is_none());
        assert!(task.try_take().is_none());
    }

    #[test]
    fn a_worker_that_dies_surfaces_one_error_then_none() {
        let mut task: XrdsNetTask<i32> = XrdsNetTask::spawn(|| panic!("boom"));

        // The panic drops the sender without sending → Disconnected → one Err.
        let result = wait_take(&mut task);
        assert!(result.is_err());
        assert!(task.try_take().is_none());
    }

    #[test]
    fn dropping_a_pending_task_does_not_block() {
        let start = Instant::now();
        {
            let _task = XrdsNetTask::spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                Ok::<i32, NetError>(1)
            });
            // dropped here while the worker is still sleeping
        }
        // No `Drop` join, so this is instant. A regression that re-added a
        // joining `Drop` would push this past the sleep, failing (not hanging).
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "dropping a pending task must not join/block the worker"
        );
    }

    #[test]
    fn take_ready_hands_back_the_result_once_and_clears_the_slot() {
        let mut slot: Option<XrdsNetTask<i32>> =
            Some(XrdsNetTask::spawn(|| Ok::<i32, NetError>(7)));

        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(r) = slot.take_ready() {
                break r;
            }
            assert!(Instant::now() < deadline, "task never completed");
            std::thread::yield_now();
        };
        assert_eq!(result.unwrap(), 7);

        // take_ready cleared the slot, and a None slot yields None.
        assert!(slot.is_none());
        assert!(slot.take_ready().is_none());
    }
}
