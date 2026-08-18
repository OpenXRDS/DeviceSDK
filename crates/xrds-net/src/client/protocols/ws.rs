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

//! `WsHandler`: WebSocket (WS/WSS) as both a [`StreamHandler`] (dispatch/
//! listen) and a [`SessionHandler`] (`open`), sharing **one** backend —
//! `WsSession`, built on `tokio-tungstenite`.
//!
//! `topic` is ignored: plain WebSocket has no topic concept (unlike MQTT),
//! it's just a message type hint, so it's threaded through as the `topic`
//! parameter per `StreamHandler`'s shared signature but not addressed by URL
//! path.
//!
//! Previously dispatch/listen used a separate sync backend (the `websocket`
//! crate) while only `open` used `tokio-tungstenite` — two independent WS
//! implementations, each pulling their own TLS dependency. Consolidated onto
//! this one backend as part of the crypto-library consolidation (see
//! `docs/done/xrds-net-crypto-consolidation.md`): one implementation, one TLS
//! stack (rustls), for all three verbs. `StreamHandler::recv` blocks by
//! polling `WsSession` (matching the old sync backend's blocking-until-message
//! contract) rather than the non-blocking `poll_recv` `SessionHandler` uses.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

use crate::client::categories::{SessionHandler, StreamHandler};
use crate::client::context::ClientContext;
use crate::client::error::NetError;
use crate::client::event::Event;
use crate::client::handler::ProtocolHandler;

#[derive(Default)]
pub struct WsHandler {
    /// Shared backend for both `StreamHandler` (dispatch/listen) and
    /// `SessionHandler` (`open`) — a handler is used as one or the other,
    /// never both at once.
    session: Option<WsSession>,
}

impl WsHandler {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StreamHandler for WsHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError> {
        self.session = Some(WsSession::connect(ctx.raw_url.as_str())?);
        Ok(())
    }

    fn send(&mut self, _ctx: &ClientContext, _topic: Option<&str>, data: Vec<u8>) -> Result<(), NetError> {
        self.session
            .as_ref()
            .ok_or_else(|| NetError::Network("WebSocket is not connected".to_string()))?
            .send(data)
    }

    fn recv(&mut self, _ctx: &ClientContext) -> Result<Event, NetError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| NetError::Network("WebSocket is not connected".to_string()))?;

        // Blocks until a message arrives or the connection closes — matches
        // the previous sync backend's `recv_message()` contract. Polls
        // `poll_recv` (non-blocking) rather than a native blocking recv,
        // since the connection is driven by WsSession's background task.
        loop {
            if let Some(event) = session.poll_recv()? {
                return Ok(event);
            }
            if session.is_disconnected() {
                return Err(NetError::Network("WebSocket connection closed".to_string()));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn close(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
        if let Some(session) = self.session.as_mut() {
            session.close();
        }
        Ok(())
    }
}

/// The WS backend shared by both `StreamHandler` (dispatch/listen) and
/// `SessionHandler` (`open`). A dedicated OS thread hosts a current-thread
/// tokio runtime running one `tokio-tungstenite` task that reads incoming
/// messages into `in_rx` and writes queued messages from `out_tx`. Callers
/// bridge to it with channels (`send` = non-blocking `try_send`, `poll_recv`
/// = non-blocking `try_recv`) — the same background-runtime-plus-channels
/// pattern as the WebRTC path's `AudioTrackWriter`. `StreamHandler::recv`
/// (blocking) polls this same non-blocking `poll_recv` in a loop instead of
/// a native blocking recv.
///
/// `Send + Sync`: `mpsc::Sender` is both; the `mpsc::Receiver` (not `Sync`) is
/// wrapped in a `Mutex`; `Arc<Notify>`, `Arc<AtomicBool>` and `JoinHandle<()>`
/// are all `Send + Sync`.
struct WsSession {
    out_tx: mpsc::Sender<Vec<u8>>,
    in_rx: Mutex<mpsc::Receiver<Event>>,
    shutdown: Arc<Notify>,
    /// Set by the worker task on any exit path (clean close, error, or
    /// explicit shutdown) — lets a blocking `poll_recv` loop
    /// (`StreamHandler::recv`) distinguish "nothing yet" from "nothing ever
    /// again" instead of spinning forever after disconnect.
    disconnected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl WsSession {
    /// Connect and start the read/write task; returns only once the WS
    /// handshake has succeeded (or errored).
    fn connect(url: &str) -> Result<Self, NetError> {
        crate::common::ensure_rustls_crypto_provider();
        let url = url.to_string();
        let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(64);
        let (in_tx, in_rx) = mpsc::channel::<Event>(256);
        let shutdown = Arc::new(Notify::new());
        let task_shutdown = shutdown.clone();
        let disconnected = Arc::new(AtomicBool::new(false));
        let task_disconnected = disconnected.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        let worker = std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };
            rt.block_on(async move {
                let ws = match tokio_tungstenite::connect_async(url.as_str()).await {
                    Ok((ws, _resp)) => {
                        let _ = ready_tx.send(Ok(()));
                        ws
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let (mut write, mut read) = ws.split();
                loop {
                    tokio::select! {
                        _ = task_shutdown.notified() => break,
                        msg = read.next() => match msg {
                            Some(Ok(m)) => {
                                if m.is_close() {
                                    break;
                                }
                                if m.is_binary() || m.is_text() {
                                    let bytes = m.into_data().to_vec();
                                    // Backpressure: awaiting a full bounded
                                    // channel pauses reading (cooperatively),
                                    // never blocking the OS thread.
                                    if in_tx.send(Event::new(None, bytes)).await.is_err() {
                                        break; // consumer gone
                                    }
                                }
                            }
                            Some(Err(_)) | None => break,
                        },
                        out = out_rx.recv() => match out {
                            Some(data) => {
                                if write.send(Message::Binary(data.into())).await.is_err() {
                                    break;
                                }
                            }
                            None => break, // send side dropped
                        },
                    }
                }
                // Every `break` above falls through to here — one place to
                // mark the session as done, regardless of exit reason.
                task_disconnected.store(true, Ordering::Relaxed);
            });
        });

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                out_tx,
                in_rx: Mutex::new(in_rx),
                shutdown,
                disconnected,
                worker: Some(worker),
            }),
            Ok(Err(e)) => {
                let _ = worker.join();
                Err(NetError::Network(e))
            }
            Err(_) => {
                let _ = worker.join();
                Err(NetError::Network(
                    "ws session thread exited before the handshake".to_string(),
                ))
            }
        }
    }

    fn send(&self, data: Vec<u8>) -> Result<(), NetError> {
        self.out_tx.try_send(data).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => {
                NetError::Network("ws session send buffer full".to_string())
            }
            mpsc::error::TrySendError::Closed(_) => {
                NetError::Network("ws session closed".to_string())
            }
        })
    }

    fn poll_recv(&self) -> Result<Option<Event>, NetError> {
        match self.in_rx.lock().unwrap().try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            // Task ended — nothing more will arrive. Treated as "nothing right
            // now" rather than an error here; callers that need to tell "not
            // yet" from "never again" apart use `is_disconnected()`
            // (`StreamHandler::recv`'s blocking loop does exactly this).
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    /// `true` once the background task has exited for any reason (clean
    /// close, error, or explicit `close()`) — lets a caller distinguish
    /// "nothing available yet" from "nothing ever again".
    fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::Relaxed)
    }

    fn close(&mut self) {
        self.shutdown.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for WsSession {
    fn drop(&mut self) {
        // Non-blocking: signal the task to stop (it also stops when the
        // channels close) and let the thread wind down on its own. `close`
        // does the explicit join.
        self.shutdown.notify_one();
    }
}

impl SessionHandler for WsHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError> {
        self.session = Some(WsSession::connect(ctx.raw_url.as_str())?);
        Ok(())
    }

    fn send(&mut self, _ctx: &ClientContext, data: Vec<u8>) -> Result<(), NetError> {
        self.session
            .as_ref()
            .ok_or_else(|| NetError::Network("WebSocket session is not connected".to_string()))?
            .send(data)
    }

    fn poll_recv(&mut self, _ctx: &ClientContext) -> Result<Option<Event>, NetError> {
        self.session
            .as_ref()
            .ok_or_else(|| NetError::Network("WebSocket session is not connected".to_string()))?
            .poll_recv()
    }

    fn close(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
        if let Some(session) = self.session.as_mut() {
            session.close();
        }
        Ok(())
    }
}

impl ProtocolHandler for WsHandler {
    fn as_stream(&mut self) -> Option<&mut dyn StreamHandler> {
        Some(self)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::PROTOCOLS;

    #[test]
    fn connect_without_a_server_is_a_network_error_not_a_panic() {
        let mut handler = WsHandler::new();
        let mut ctx = ClientContext::new(PROTOCOLS::WS, "test-id".to_string());
        ctx.raw_url = "ws://127.0.0.1:1/".to_string();

        // `connect` is now on two traits (Stream + Session); this test targets
        // the stream (dispatch/listen) path.
        let err = StreamHandler::connect(&mut handler, &ctx).expect_err("bad server should fail");
        assert!(matches!(err, NetError::Network(_)));
    }

    #[test]
    fn exposes_itself_as_a_stream_handler() {
        let mut handler = WsHandler::new();
        assert!(handler.as_stream().is_some());
    }
}
