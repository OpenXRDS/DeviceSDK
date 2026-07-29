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

//! `NetChannel`: the easy bidirectional-session handle returned by
//! `XrdsNet::open` — one connection you both `send` on and `try_recv` from.
//! It's the "session" shape (point-to-point, both directions on one socket),
//! distinct from the pub/sub `dispatch`/`listen` verbs. Backed by a
//! [`SessionHandler`](crate::client::SessionHandler) — QUIC today. See
//! `docs/done/xrds-net-devicesdk-integration.md`'s "bidirectional sessions".
//!
//! For a poll-based transport (QUIC) no background thread is needed:
//! `try_recv` polls the connection directly and never blocks, so it drops
//! straight into an `XrdsApp::update()` loop.

use std::time::{Duration, Instant};

use super::context::ClientContext;
use super::error::NetError;
use super::event::Event;
use super::handler::ProtocolHandler;

pub struct NetChannel {
    ctx: ClientContext,
    handler: Box<dyn ProtocolHandler>,
    error: Option<NetError>,
}

impl NetChannel {
    /// Wrap an already-connected session handler. Constructed by
    /// `XrdsNet::open`.
    pub(crate) fn new(ctx: ClientContext, handler: Box<dyn ProtocolHandler>) -> Self {
        Self {
            ctx,
            handler,
            error: None,
        }
    }

    /// Send application bytes on the connection.
    pub fn send(&mut self, data: Vec<u8>) -> Result<(), NetError> {
        match self.handler.as_session() {
            Some(session) => session.send(&self.ctx, data),
            None => Err(NetError::capability(
                self.ctx.protocol,
                "open",
                "protocol has no bidirectional-session capability",
            )),
        }
    }

    /// Non-blocking receive. `None` = nothing arrived this poll. A poll error
    /// (e.g. the connection dropped) is recorded and surfaced once via
    /// [`take_error`](Self::take_error), so the common drain loop stays
    /// `while let Some(ev) = chan.try_recv()`.
    pub fn try_recv(&mut self) -> Option<Event> {
        let result = match self.handler.as_session() {
            Some(session) => session.poll_recv(&self.ctx),
            None => Err(NetError::capability(
                self.ctx.protocol,
                "open",
                "protocol has no bidirectional-session capability",
            )),
        };
        match result {
            Ok(event) => event,
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }

    /// A poll/connection error recorded by `try_recv`, surfaced once.
    pub fn take_error(&mut self) -> Option<NetError> {
        self.error.take()
    }

    /// Blocking receive with a deadline — a convenience for scripts/tests
    /// (an `XrdsApp` loop should use `try_recv` each frame instead).
    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<Event, NetError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(event) = self.try_recv() {
                return Ok(event);
            }
            if let Some(e) = self.error.take() {
                return Err(e);
            }
            if Instant::now() >= deadline {
                return Err(NetError::Network("open() recv timed out".to_string()));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Close the session.
    pub fn close(mut self) -> Result<(), NetError> {
        match self.handler.as_session() {
            Some(session) => session.close(&self.ctx),
            None => Ok(()),
        }
    }
}

impl std::fmt::Debug for NetChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NetChannel {{ protocol: {:?} }}", self.ctx.protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::categories::SessionHandler;
    use crate::common::enums::PROTOCOLS;
    use std::collections::VecDeque;

    /// A loopback session: `send` enqueues, `poll_recv` dequeues — enough to
    /// validate `NetChannel`'s plumbing without a real transport.
    #[derive(Default)]
    struct LoopbackSession {
        inbox: VecDeque<Vec<u8>>,
    }

    impl SessionHandler for LoopbackSession {
        fn connect(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
            Ok(())
        }
        fn send(&mut self, _ctx: &ClientContext, data: Vec<u8>) -> Result<(), NetError> {
            self.inbox.push_back(data);
            Ok(())
        }
        fn poll_recv(&mut self, _ctx: &ClientContext) -> Result<Option<Event>, NetError> {
            Ok(self.inbox.pop_front().map(|d| Event::new(None, d)))
        }
        fn close(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
            Ok(())
        }
    }

    impl ProtocolHandler for LoopbackSession {
        fn as_session(&mut self) -> Option<&mut dyn SessionHandler> {
            Some(self)
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    fn ctx() -> ClientContext {
        ClientContext::new(PROTOCOLS::QUIC, "test-id".to_string())
    }

    #[test]
    fn channel_is_send_and_sync() {
        // A `NetChannel` held as an `XrdsApp` field must be Send + Sync.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NetChannel>();
    }

    #[test]
    fn send_then_try_recv_round_trips_via_the_session_in_order() {
        let mut chan = NetChannel::new(ctx(), Box::new(LoopbackSession::default()));

        assert!(chan.try_recv().is_none()); // nothing yet, non-blocking

        chan.send(b"a".to_vec()).unwrap();
        chan.send(b"b".to_vec()).unwrap();

        assert_eq!(chan.try_recv().unwrap().payload, b"a".to_vec());
        assert_eq!(chan.try_recv().unwrap().payload, b"b".to_vec());
        assert!(chan.try_recv().is_none());
        assert!(chan.take_error().is_none());
    }

    #[test]
    fn poll_error_is_stashed_for_take_error_not_a_panic() {
        use crate::client::handler::UnsupportedHandler;

        // A handler with no session capability: send/try_recv report a
        // Capability error rather than panicking; try_recv routes it to
        // take_error and returns None.
        let mut chan = NetChannel::new(ctx(), Box::new(UnsupportedHandler::new(PROTOCOLS::WEBRTC)));

        assert!(matches!(
            chan.send(b"x".to_vec()),
            Err(NetError::Capability { verb: "open", .. })
        ));
        assert!(chan.try_recv().is_none());
        assert!(matches!(
            chan.take_error(),
            Some(NetError::Capability { verb: "open", .. })
        ));
        assert!(chan.take_error().is_none()); // surfaced once
    }
}
