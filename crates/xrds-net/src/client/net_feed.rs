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

//! `NetFeed`: the recommended streaming surface for in-app use. It hides the
//! two-stage `connect → stream` handshake behind a single value the app
//! holds and drains — no hand-written state machine. See "Recommended app
//! surface" in `docs/done/xrds-net-devicesdk-integration.md`.
//!
//! It owns the `XrdsNetTask<EventStream>` while the background connect/
//! subscribe is in flight, flips to the live `EventStream` once that resolves,
//! and surfaces a connect failure once via `take_error()`.

use super::error::NetError;
use super::event::{Event, EventStream};
use super::net_task::XrdsNetTask;

enum FeedState {
    /// Background connect/subscribe still in flight.
    Connecting(XrdsNetTask<EventStream>),
    /// Connected — draining the live stream.
    Streaming(EventStream),
    /// Connect failed (error taken via `take_error`) or the feed was retired.
    Ended,
}

pub struct NetFeed {
    state: FeedState,
    error: Option<NetError>,
}

impl NetFeed {
    /// Wrap a still-connecting listen task. Constructed by
    /// `XrdsNet::listen_feed` / `listen_feed_with`.
    pub(crate) fn new(task: XrdsNetTask<EventStream>) -> Self {
        Self {
            state: FeedState::Connecting(task),
            error: None,
        }
    }

    /// Non-blocking. `None` = nothing available yet (still connecting, or no
    /// event arrived this frame, or the feed has ended). Drain in a
    /// `while let` each frame. A connect/subscribe failure moves the feed to
    /// ended and is reported via [`take_error`](Self::take_error).
    pub fn try_recv(&mut self) -> Option<Event> {
        // Advance the handshake if we're still connecting. `mem::replace`
        // moves the task out so we can reassign `self.state` without holding a
        // borrow of it.
        if matches!(self.state, FeedState::Connecting(_)) {
            if let FeedState::Connecting(mut task) =
                std::mem::replace(&mut self.state, FeedState::Ended)
            {
                match task.try_take() {
                    None => {
                        self.state = FeedState::Connecting(task); // still connecting
                        return None;
                    }
                    Some(Ok(stream)) => self.state = FeedState::Streaming(stream),
                    Some(Err(e)) => {
                        self.error = Some(e);
                        return None; // state stays Ended
                    }
                }
            }
        }

        match &self.state {
            FeedState::Streaming(stream) => stream.try_recv(),
            _ => None,
        }
    }

    /// A connect/subscribe failure, surfaced once. Poll it alongside
    /// `try_recv` if you want to react to the feed failing to come up.
    pub fn take_error(&mut self) -> Option<NetError> {
        self.error.take()
    }

    /// Stop the feed — non-blocking whether still connecting (drops the task)
    /// or streaming (immediate `EventStream::close()`).
    pub fn close(self) {
        match self.state {
            FeedState::Streaming(stream) => stream.close(),
            // Connecting: dropping the task is non-blocking (detached worker);
            // Ended: nothing to do.
            _ => {}
        }
    }
}

impl std::fmt::Debug for NetFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = match &self.state {
            FeedState::Connecting(_) => "connecting",
            FeedState::Streaming(_) => "streaming",
            FeedState::Ended => "ended",
        };
        write!(f, "NetFeed {{ state: {state} }}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::categories::StreamHandler;
    use crate::client::context::ClientContext;
    use crate::client::event::ListenOptions;
    use crate::client::handler::ProtocolHandler;
    use crate::common::enums::PROTOCOLS;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};

    /// Emits `total` events (`Ok`) then errors — same shape as the mock in
    /// `event.rs`, repeated here so this module's tests are self-contained.
    struct MockStream {
        emitted: AtomicUsize,
        total: usize,
    }

    impl StreamHandler for MockStream {
        fn connect(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
            Ok(())
        }
        fn send(&mut self, _ctx: &ClientContext, _topic: Option<&str>, _data: Vec<u8>) -> Result<(), NetError> {
            Ok(())
        }
        fn recv(&mut self, _ctx: &ClientContext) -> Result<Event, NetError> {
            let n = self.emitted.fetch_add(1, Ordering::Relaxed);
            if n < self.total {
                Ok(Event::new(None, vec![n as u8]))
            } else {
                Err(NetError::Network("mock stream exhausted".to_string()))
            }
        }
        fn close(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
            Ok(())
        }
    }

    impl ProtocolHandler for MockStream {
        fn as_stream(&mut self) -> Option<&mut dyn StreamHandler> {
            Some(self)
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    /// A resolved-to-`Ok(EventStream)` task backed by a mock emitting `total`
    /// events, built on a background thread (so the feed genuinely starts in
    /// `Connecting`).
    fn streaming_task(total: usize) -> XrdsNetTask<EventStream> {
        XrdsNetTask::spawn(move || {
            let handler = Box::new(MockStream {
                emitted: AtomicUsize::new(0),
                total,
            });
            let ctx = ClientContext::new(PROTOCOLS::WS, "test-id".to_string());
            EventStream::spawn(handler, ctx, ListenOptions::default())
        })
    }

    #[test]
    fn feed_is_send_and_sync() {
        // An `XrdsApp` holding a `NetFeed` must be Send + Sync for
        // `Runtime::run_xrds`.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NetFeed>();
        assert_send_sync::<EventStream>();
    }

    #[test]
    fn none_while_connecting_then_drains_once_streaming() {
        let mut feed = NetFeed::new(streaming_task(2));

        let mut got = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.len() < 2 {
            if let Some(ev) = feed.try_recv() {
                got.push(ev.payload);
            }
            assert!(Instant::now() < deadline, "feed never delivered events");
            std::thread::yield_now();
        }
        assert_eq!(got, vec![vec![0u8], vec![1u8]]);
        assert!(feed.take_error().is_none());
    }

    #[test]
    fn connect_failure_surfaces_once_then_feed_is_inert() {
        let task =
            XrdsNetTask::spawn(|| Err::<EventStream, NetError>(NetError::Network("nope".to_string())));
        let mut feed = NetFeed::new(task);

        let deadline = Instant::now() + Duration::from_secs(5);
        let err = loop {
            let _ = feed.try_recv(); // drives the handshake to Ended on failure
            if let Some(e) = feed.take_error() {
                break e;
            }
            assert!(Instant::now() < deadline, "connect error never surfaced");
            std::thread::yield_now();
        };
        assert!(matches!(err, NetError::Network(_)));

        // Inert afterward: no more events, no second error.
        assert!(feed.try_recv().is_none());
        assert!(feed.take_error().is_none());
    }

    #[test]
    fn close_is_non_blocking_while_connecting() {
        // A task that never resolves until we release it — the feed stays in
        // Connecting, so close() must just drop the task (non-blocking).
        let (gate_tx, gate_rx) = channel::<()>();
        let task = XrdsNetTask::spawn(move || {
            gate_rx.recv().unwrap();
            Err::<EventStream, NetError>(NetError::Network("x".to_string()))
        });
        let feed = NetFeed::new(task);

        let start = Instant::now();
        feed.close();
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "close while connecting must not block on the task"
        );

        let _ = gate_tx.send(()); // let the detached worker exit cleanly
    }

    #[test]
    fn close_is_prompt_while_streaming() {
        let mut feed = NetFeed::new(streaming_task(1));

        // Receiving an event proves we've flipped to Streaming.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if feed.try_recv().is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "never reached streaming");
            std::thread::yield_now();
        }

        let start = Instant::now();
        feed.close();
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "close while streaming should be prompt"
        );
    }
}
