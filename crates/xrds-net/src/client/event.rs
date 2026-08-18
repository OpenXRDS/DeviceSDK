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

//! `Event`: one message from a [`StreamHandler`](crate::client::categories::StreamHandler).
//!
//! `topic` carries the URL-path-derived topic/track name for topic-addressed
//! transports (MQTT, future MoQ) and is `None` for topic-less ones (WS, raw
//! QUIC) — one shape covers both, see `docs/done/xrds-net-protocol-handler.md`'s
//! "Topic addressing".
//!
//! `EventStream`: `listen()`'s return type. A background thread repeatedly
//! calls the handler's `recv()` and forwards `Event`s over a **bounded**
//! buffer whose overflow behavior is chosen via [`ListenOptions`]. The
//! shutdown flag + `JoinHandle` pattern is the same one already proven twice
//! in this codebase (`xrds-media`'s `Webcam`/`Microphone`, `xrds-net`'s own
//! `AudioTrackWriter` bridge). The bounded buffer is what keeps a fast
//! producer (e.g. a video feed) from growing memory without bound when the
//! consumer falls behind — see "Backpressure" in
//! `docs/done/xrds-net-devicesdk-integration.md`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::common::enums::PROTOCOLS;

use super::context::ClientContext;
use super::error::NetError;
use super::handler::ProtocolHandler;
use super::net_intent::topic_from_path;
use super::protocols::mqtt::MqttHandler;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub topic: Option<String>,
    pub payload: Vec<u8>,
}

impl Event {
    pub fn new(topic: Option<String>, payload: Vec<u8>) -> Self {
        Self { topic, payload }
    }
}

/// What a listen stream's bounded buffer does when it fills faster than the
/// consumer drains it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    /// The producer waits for space (backpressure). Over a TCP-backed
    /// transport this propagates into TCP flow control, so the sender
    /// throttles — lossless. Right for reliable / VOD-style delivery, and the
    /// default.
    Block,
    /// The producer drops the oldest buffered event to make room, never
    /// waiting. Bounds both memory and latency at the cost of losing stale
    /// data. Right for live/real-time video, where a late frame is worthless.
    DropOldest,
}

/// Buffering policy for a listen stream. `Default` is bounded + lossless
/// (`buffer: 256`, `Overflow::Block`), so a plain `listen(url)` can't grow
/// memory without bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListenOptions {
    /// Max events buffered before `overflow` applies. Counted in events
    /// (chunks/frames), not bytes. Clamped to at least 1.
    pub buffer: usize,
    pub overflow: Overflow,
}

impl Default for ListenOptions {
    fn default() -> Self {
        Self {
            buffer: 256,
            overflow: Overflow::Block,
        }
    }
}

/// Shared bounded buffer between the background worker (producer) and the
/// `EventStream` (consumer). One structure covers both policies — they differ
/// only in what `push` does when full.
struct BufferInner {
    queue: VecDeque<Event>,
    cap: usize,
    overflow: Overflow,
    /// Worker has exited — no more events will ever arrive.
    producer_gone: bool,
    /// `EventStream` was dropped/closed — the producer should stop.
    consumer_gone: bool,
}

struct Buffer {
    inner: Mutex<BufferInner>,
    not_empty: Condvar,
    not_full: Condvar,
}

impl Buffer {
    fn new(opts: ListenOptions) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(BufferInner {
                queue: VecDeque::new(),
                // cap 0 would deadlock `Block` (never any space) — clamp.
                cap: opts.buffer.max(1),
                overflow: opts.overflow,
                producer_gone: false,
                consumer_gone: false,
            }),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
        })
    }

    /// Producer side. `Err(())` means the consumer is gone — stop pushing.
    fn push(&self, event: Event) -> Result<(), ()> {
        let mut inner = self.inner.lock().unwrap();
        loop {
            if inner.consumer_gone {
                return Err(());
            }
            if inner.queue.len() < inner.cap {
                inner.queue.push_back(event);
                self.not_empty.notify_one();
                return Ok(());
            }
            match inner.overflow {
                Overflow::DropOldest => {
                    inner.queue.pop_front();
                    inner.queue.push_back(event);
                    self.not_empty.notify_one();
                    return Ok(());
                }
                Overflow::Block => {
                    // Wait for the consumer to make space (or go away). The
                    // wait releases the lock; re-check both on wake.
                    inner = self.not_full.wait(inner).unwrap();
                }
            }
        }
    }

    /// Consumer side, non-blocking.
    fn try_pop(&self) -> Option<Event> {
        let mut inner = self.inner.lock().unwrap();
        let ev = inner.queue.pop_front();
        if ev.is_some() {
            self.not_full.notify_one();
        }
        ev
    }

    /// Consumer side, blocking until an event is available or the producer is
    /// gone (`None` = stream ended).
    fn pop_blocking(&self) -> Option<Event> {
        let mut inner = self.inner.lock().unwrap();
        loop {
            if let Some(ev) = inner.queue.pop_front() {
                self.not_full.notify_one();
                return Some(ev);
            }
            if inner.producer_gone {
                return None;
            }
            inner = self.not_empty.wait(inner).unwrap();
        }
    }

    /// Consumer side, blocking up to `timeout`.
    fn pop_timeout(&self, timeout: Duration) -> Result<Event, NetError> {
        let deadline = Instant::now() + timeout;
        let mut inner = self.inner.lock().unwrap();
        loop {
            if let Some(ev) = inner.queue.pop_front() {
                self.not_full.notify_one();
                return Ok(ev);
            }
            if inner.producer_gone {
                return Err(NetError::Network("stream ended".to_string()));
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(NetError::Network(
                    "listen() timed out waiting for an event".to_string(),
                ));
            }
            let (guard, _res) = self.not_empty.wait_timeout(inner, deadline - now).unwrap();
            inner = guard;
        }
    }

    fn mark_producer_gone(&self) {
        self.inner.lock().unwrap().producer_gone = true;
        // Wake any consumer blocked in pop_blocking/pop_timeout so it can
        // observe end-of-stream.
        self.not_empty.notify_all();
    }

    fn mark_consumer_gone(&self) {
        self.inner.lock().unwrap().consumer_gone = true;
        // Wake a `Block`-policy producer waiting for space so it can observe
        // the consumer is gone and stop (prevents a teardown deadlock).
        self.not_full.notify_all();
    }
}

/// Blocking, synchronous stream of `Event`s — `for event in stream { .. }`,
/// `.recv_timeout(..)`, or a per-frame `.try_recv()`, no async runtime
/// required.
pub struct EventStream {
    buffer: Arc<Buffer>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventStream {{ .. }}")
    }
}

impl EventStream {
    /// Connects `handler` (already confirmed to be stream-shaped by the
    /// caller — see `XrdsNet::listen`) and spawns the background recv loop,
    /// buffering into a bounded queue per `opts`. Takes ownership of both
    /// `handler` and `ctx` since the loop must outlive the caller's stack
    /// frame.
    pub(crate) fn spawn(
        mut handler: Box<dyn ProtocolHandler>,
        ctx: ClientContext,
        opts: ListenOptions,
    ) -> Result<Self, NetError> {
        {
            let stream = handler.as_stream().ok_or_else(|| {
                NetError::capability(ctx.protocol, "listen", "protocol has no ongoing-stream capability")
            })?;
            stream.connect(&ctx)?;
        }

        // MQTT's "listen" capability is subscribe-shaped: `connect()` alone
        // only opens the broker connection, it doesn't subscribe to
        // anything. Topic-less transports (WS, raw QUIC) don't need this —
        // their `connect()` is already everything "start listening" means.
        if ctx.protocol == PROTOCOLS::MQTT {
            if let Some(topic) = topic_from_path(&ctx.path) {
                if let Some(mqtt) = handler.as_any_mut().downcast_mut::<MqttHandler>() {
                    mqtt.subscribe(&topic)?;
                }
            }
        }

        let buffer = Buffer::new(opts);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();
        let producer = buffer.clone();

        let worker = std::thread::spawn(move || {
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let Some(stream) = handler.as_stream() else {
                    break;
                };
                match stream.recv(&ctx) {
                    Ok(event) => {
                        if producer.push(event).is_err() {
                            break; // EventStream (consumer) was dropped
                        }
                    }
                    Err(_) => break, // connection is gone — end the stream
                }
            }
            producer.mark_producer_gone();
        });

        Ok(Self {
            buffer,
            shutdown,
            worker: Some(worker),
        })
    }

    /// Non-blocking. `None` = nothing waiting right now (not "ended"). Drain
    /// in a `while let` each frame.
    pub fn try_recv(&self) -> Option<Event> {
        self.buffer.try_pop()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Event, NetError> {
        self.buffer.pop_timeout(timeout)
    }

    /// Signal the background loop to stop and join it — no arbitrary sleep.
    /// If the loop is blocked inside the handler's `recv()`, this returns once
    /// that call unblocks (e.g. the next keep-alive/read), the same bound the
    /// underlying transport itself has.
    pub fn close(mut self) {
        self.teardown();
    }

    fn teardown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        // Unblock a `Block`-policy producer parked in `push` waiting for
        // space, so `join` below can't deadlock against it.
        self.buffer.mark_consumer_gone();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Iterator for EventStream {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        self.buffer.pop_blocking()
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        self.teardown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::categories::StreamHandler;
    use crate::common::enums::PROTOCOLS;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    #[test]
    fn constructs_with_and_without_topic() {
        let with_topic = Event::new(Some("sensors/temp".to_string()), vec![1, 2, 3]);
        assert_eq!(with_topic.topic.as_deref(), Some("sensors/temp"));

        let without_topic = Event::new(None, vec![4, 5]);
        assert_eq!(without_topic.topic, None);
        assert_eq!(without_topic.payload, vec![4, 5]);
    }

    // ---- ListenOptions / Buffer (white-box) --------------------------------

    #[test]
    fn default_options_are_bounded_and_lossless() {
        let opts = ListenOptions::default();
        assert_eq!(opts.buffer, 256);
        assert_eq!(opts.overflow, Overflow::Block);
    }

    #[test]
    fn drop_oldest_caps_and_keeps_newest_in_order() {
        let buf = Buffer::new(ListenOptions {
            buffer: 2,
            overflow: Overflow::DropOldest,
        });
        buf.push(Event::new(None, vec![1])).unwrap();
        buf.push(Event::new(None, vec![2])).unwrap();
        buf.push(Event::new(None, vec![3])).unwrap(); // drops the oldest ([1])

        assert_eq!(buf.try_pop().unwrap().payload, vec![2]);
        assert_eq!(buf.try_pop().unwrap().payload, vec![3]);
        assert!(buf.try_pop().is_none());
    }

    #[test]
    fn block_applies_backpressure_producer_waits_for_space() {
        let buf = Buffer::new(ListenOptions {
            buffer: 1,
            overflow: Overflow::Block,
        });
        buf.push(Event::new(None, vec![1])).unwrap(); // buffer now full

        let producer = {
            let buf = buf.clone();
            std::thread::spawn(move || buf.push(Event::new(None, vec![2])))
        };

        // Give the producer a chance to run: it must block on the full buffer,
        // not push. (A small sleep is the standard way to assert "still
        // blocked"; there's no unbounded growth because push never returns.)
        std::thread::sleep(Duration::from_millis(50));
        assert!(
            !producer.is_finished(),
            "producer should be blocked on a full Block buffer"
        );

        // Draining one slot unblocks the producer.
        assert_eq!(buf.try_pop().unwrap().payload, vec![1]);
        producer.join().unwrap().unwrap();
        assert_eq!(buf.try_pop().unwrap().payload, vec![2]);
    }

    #[test]
    fn block_producer_unblocks_when_consumer_goes_away() {
        let buf = Buffer::new(ListenOptions {
            buffer: 1,
            overflow: Overflow::Block,
        });
        buf.push(Event::new(None, vec![1])).unwrap();

        let producer = {
            let buf = buf.clone();
            std::thread::spawn(move || buf.push(Event::new(None, vec![2])))
        };

        std::thread::sleep(Duration::from_millis(50));
        buf.mark_consumer_gone();

        let result = producer.join().unwrap();
        assert!(
            result.is_err(),
            "push must return Err once the consumer is gone (no teardown deadlock)"
        );
    }

    // ---- EventStream end-to-end over the buffer (via a mock handler) -------

    /// Emits `total` events (`Ok`) then errors forever — enough to drive
    /// `EventStream`'s loop through both the happy path and its
    /// end-of-stream-on-error exit, without any real network I/O.
    struct MockStreamHandler {
        emitted: AtomicUsize,
        total: usize,
    }

    impl StreamHandler for MockStreamHandler {
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

    impl ProtocolHandler for MockStreamHandler {
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

    fn mock_ctx() -> ClientContext {
        ClientContext::new(PROTOCOLS::WS, "test-id".to_string())
    }

    #[test]
    fn iterates_events_then_ends_when_the_handler_errors() {
        let handler = Box::new(MockStreamHandler {
            emitted: AtomicUsize::new(0),
            total: 3,
        });
        let stream = EventStream::spawn(handler, mock_ctx(), ListenOptions::default())
            .expect("mock is stream-shaped");

        let received: Vec<Event> = stream.collect();
        assert_eq!(received.len(), 3);
        assert_eq!(received[0].payload, vec![0]);
        assert_eq!(received[2].payload, vec![2]);
    }

    #[test]
    fn close_joins_without_an_arbitrary_sleep() {
        // total: 0 => the mock errors on the very first recv(), so the
        // worker thread should already be exiting by the time close() signals
        // shutdown — proving join() doesn't need a sleep to avoid hanging.
        let handler = Box::new(MockStreamHandler {
            emitted: AtomicUsize::new(0),
            total: 0,
        });
        let stream = EventStream::spawn(handler, mock_ctx(), ListenOptions::default())
            .expect("mock is stream-shaped");
        stream.close(); // must return promptly, no sleep before/after this call
    }

    #[test]
    fn recv_timeout_reports_a_clear_error_when_nothing_arrives() {
        // total: 0 => no events are ever sent, so recv_timeout must time out
        // rather than hang.
        let handler = Box::new(MockStreamHandler {
            emitted: AtomicUsize::new(0),
            total: 0,
        });
        let stream = EventStream::spawn(handler, mock_ctx(), ListenOptions::default())
            .expect("mock is stream-shaped");
        let err = stream
            .recv_timeout(Duration::from_millis(50))
            .expect_err("no event should arrive");
        assert!(matches!(err, NetError::Network(_)));
    }

    #[test]
    fn try_recv_is_non_blocking_and_drains_in_order() {
        let handler = Box::new(MockStreamHandler {
            emitted: AtomicUsize::new(0),
            total: 2,
        });
        let stream = EventStream::spawn(handler, mock_ctx(), ListenOptions::default())
            .expect("mock is stream-shaped");

        // Drain until we've seen both events; try_recv never blocks, so we
        // spin briefly while the worker fills the buffer.
        let mut got = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while got.len() < 2 && Instant::now() < deadline {
            if let Some(ev) = stream.try_recv() {
                got.push(ev.payload);
            }
        }
        assert_eq!(got, vec![vec![0u8], vec![1u8]]);
    }

    #[test]
    fn listen_on_a_non_stream_handler_is_a_capability_error() {
        use crate::client::handler::UnsupportedHandler;

        let handler: Box<dyn ProtocolHandler> = Box::new(UnsupportedHandler::dedicated_api(PROTOCOLS::WEBRTC, "WebRTCClient"));
        let err = EventStream::spawn(
            handler,
            ClientContext::new(PROTOCOLS::WEBRTC, "test-id".to_string()),
            ListenOptions::default(),
        )
        .expect_err("WEBRTC has no stream shape");
        assert!(matches!(err, NetError::Capability { verb: "listen", .. }));
    }
}
