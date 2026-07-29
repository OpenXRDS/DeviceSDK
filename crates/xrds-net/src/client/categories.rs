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

//! Category traits: the two capability shapes a [`ProtocolHandler`] can offer
//! beyond plain `request`. See `docs/xrds-net-protocol-handler.md`'s
//! "`ProtocolHandler` mechanism".
//!
//! A handler exposes these via [`ProtocolHandler::as_stream`]/
//! [`ProtocolHandler::as_file_transfer`] (default `None`) rather than
//! implementing them as default-erroring methods directly on
//! `ProtocolHandler` — "does this protocol have this shape at all" is a
//! different kind of question from "does it support this optional verb of a
//! shape it definitely has."
//!
//! [`ProtocolHandler`]: super::handler::ProtocolHandler

use super::context::ClientContext;
use super::error::NetError;
use super::event::Event;

/// WS, QUIC, MQTT today; MoQ later. `topic` is `None` for topic-less
/// transports (WS, raw QUIC) — one trait covers both shapes, since a
/// topic-addressed transport is just "byte messaging plus an address."
pub trait StreamHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
    fn send(&mut self, ctx: &ClientContext, topic: Option<&str>, data: Vec<u8>) -> Result<(), NetError>;
    fn recv(&mut self, ctx: &ClientContext) -> Result<Event, NetError>;
    fn close(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
}

/// A bidirectional session on a **single** connection: send *and*
/// non-blocking receive on the same socket. Backs `XrdsNet::open` /
/// `NetChannel`. QUIC today; WS once its client is reworked for duplex.
///
/// Distinct from [`StreamHandler`]: that's the pub/sub shape (`dispatch`
/// publishes on one connection, `listen` subscribes on another, broker-
/// mediated). A session is point-to-point — the same connection carries both
/// directions — which is what WS and raw QUIC actually are. See
/// `docs/done/xrds-net-devicesdk-integration.md`'s "bidirectional sessions".
pub trait SessionHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
    fn send(&mut self, ctx: &ClientContext, data: Vec<u8>) -> Result<(), NetError>;
    /// Non-blocking. `Ok(None)` = nothing available right now (not an error,
    /// not end-of-stream) — the caller polls this each frame.
    fn poll_recv(&mut self, ctx: &ClientContext) -> Result<Option<Event>, NetError>;
    fn close(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
}

/// FTP/SFTP today.
pub trait FileTransferHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
    fn upload(&mut self, ctx: &ClientContext, path: &str, data: Vec<u8>) -> Result<(), NetError>;
    fn download(&mut self, ctx: &ClientContext, path: &str) -> Result<Vec<u8>, NetError>;
    fn list(&mut self, ctx: &ClientContext, path: &str) -> Result<Vec<String>, NetError>;
    fn delete(&mut self, ctx: &ClientContext, path: &str) -> Result<(), NetError>;
}
