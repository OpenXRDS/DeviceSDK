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

//! `ProtocolHandler`: one implementation per protocol, the mechanism shared by
//! both the `Client` expert/session API and the `XrdsNet` intent-verb layer.
//! See `docs/done/xrds-net-protocol-handler.md`'s "`ProtocolHandler` mechanism".
//!
//! `create_handler` is the one place that matches over all `PROTOCOLS` — it
//! is not called by `Client`/`ClientBuilder` yet (that cutover is Phase 2),
//! but every concrete handler from Phase 1 is wired in so `XrdsNet` (Phase 3)
//! and `Client` (Phase 2) share the exact same construction path.

use crate::common::data_structure::NetResponse;
use crate::common::enums::PROTOCOLS;

use super::categories::{FileTransferHandler, SessionHandler, StreamHandler};
use super::context::ClientContext;
use super::error::NetError;
use super::protocols::coap::CoapHandler;
use super::protocols::ftp::FtpHandler;
use super::protocols::http::HttpHandler;
use super::protocols::http3::Http3Handler;
use super::protocols::mqtt::MqttHandler;
use super::protocols::quic::QuicHandler;
use super::protocols::ws::WsHandler;

pub trait ProtocolHandler: Send + Sync {
    /// Precondition check — see `docs/done/xrds-net-protocol-handler.md`'s
    /// "Guided-error validation". Runs before the handler's actual verb;
    /// default `Ok(())` for handlers with nothing to precheck.
    fn validate(&self, _ctx: &ClientContext) -> Result<(), NetError> {
        Ok(())
    }

    /// Request-shaped protocols (HTTP/HTTPS/FILE/CoAP/HTTP3) implement this
    /// directly. Everyone else's default declares the capability absent.
    fn request(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        Err(NetError::capability(
            ctx.protocol,
            "request",
            "protocol does not support request/response",
        ))
    }

    /// Capability queries: does this handler have the `StreamHandler`/
    /// `SessionHandler`/`FileTransferHandler` shape? `None` (the default)
    /// means no — callers turn that into `NetError::Capability`.
    fn as_stream(&mut self) -> Option<&mut dyn StreamHandler> {
        None
    }
    fn as_session(&mut self) -> Option<&mut dyn SessionHandler> {
        None
    }
    fn as_file_transfer(&mut self) -> Option<&mut dyn FileTransferHandler> {
        None
    }

    /// Escape hatch for concrete-type-only extras (FTP's raw `FtpCommands`
    /// surface, MQTT's raw connection handle) — see
    /// `docs/done/xrds-net-protocol-handler.md`'s "Expert-only extras".
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Placeholder for protocols with no handler at all (`WEBRTC` today; future
/// feature-disabled protocols once Cargo features exist). Implements neither
/// `request` nor either capability — every verb reaches the trait's default
/// bodies and reports `NetError::Capability`.
pub struct UnsupportedHandler {
    protocol: PROTOCOLS,
}

impl UnsupportedHandler {
    pub fn new(protocol: PROTOCOLS) -> Self {
        Self { protocol }
    }

    pub fn protocol(&self) -> PROTOCOLS {
        self.protocol
    }
}

impl ProtocolHandler for UnsupportedHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// One `ProtocolHandler` per `PROTOCOLS` variant — the construction path
/// shared by `Client`/`ClientBuilder` (Phase 2) and `XrdsNet` (Phase 3).
/// `WEBRTC` has no handler here; it stays on its own dedicated API (see
/// "Can WebRTC join this model?" in the plan doc) and falls back to
/// `UnsupportedHandler` for anything routed through this mechanism.
pub fn create_handler(protocol: PROTOCOLS) -> Box<dyn ProtocolHandler> {
    match protocol {
        PROTOCOLS::HTTP | PROTOCOLS::HTTPS | PROTOCOLS::FILE => Box::new(HttpHandler::new()),
        PROTOCOLS::COAP => Box::new(CoapHandler::new()),
        PROTOCOLS::HTTP3 => Box::new(Http3Handler::new()),
        PROTOCOLS::QUIC => Box::new(QuicHandler::new()),
        PROTOCOLS::WS | PROTOCOLS::WSS => Box::new(WsHandler::new()),
        PROTOCOLS::MQTT => Box::new(MqttHandler::new()),
        PROTOCOLS::FTP | PROTOCOLS::SFTP => Box::new(FtpHandler::new()),
        PROTOCOLS::WEBRTC => Box::new(UnsupportedHandler::new(protocol)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(protocol: PROTOCOLS) -> ClientContext {
        ClientContext::new(protocol, "test-id".to_string())
    }

    #[test]
    fn unsupported_handler_reports_capability_error_for_request() {
        let handler = UnsupportedHandler::new(PROTOCOLS::WEBRTC);
        let err = handler
            .request(&ctx(PROTOCOLS::WEBRTC))
            .expect_err("should not support request");
        assert!(matches!(err, NetError::Capability { verb: "request", .. }));
    }

    #[test]
    fn unsupported_handler_has_no_stream_or_file_transfer_shape() {
        let mut handler = UnsupportedHandler::new(PROTOCOLS::WEBRTC);
        assert!(handler.as_stream().is_none());
        assert!(handler.as_file_transfer().is_none());
    }

    #[test]
    fn default_validate_is_ok() {
        let handler = UnsupportedHandler::new(PROTOCOLS::WEBRTC);
        assert!(handler.validate(&ctx(PROTOCOLS::WEBRTC)).is_ok());
    }

    #[test]
    fn create_handler_covers_every_protocol_with_the_right_shape() {
        let stream_protocols = [PROTOCOLS::QUIC, PROTOCOLS::WS, PROTOCOLS::WSS, PROTOCOLS::MQTT];
        let file_transfer_protocols = [PROTOCOLS::FTP, PROTOCOLS::SFTP];
        let request_protocols = [
            PROTOCOLS::HTTP,
            PROTOCOLS::HTTPS,
            PROTOCOLS::FILE,
            PROTOCOLS::COAP,
            PROTOCOLS::HTTP3,
        ];

        for protocol in stream_protocols {
            let mut handler = create_handler(protocol);
            assert!(handler.as_stream().is_some(), "{protocol:?} should be a StreamHandler");
        }
        for protocol in file_transfer_protocols {
            let mut handler = create_handler(protocol);
            assert!(
                handler.as_file_transfer().is_some(),
                "{protocol:?} should be a FileTransferHandler"
            );
        }
        for protocol in request_protocols {
            let handler = create_handler(protocol);
            assert!(handler.as_any().downcast_ref::<UnsupportedHandler>().is_none());
        }

        let webrtc = create_handler(PROTOCOLS::WEBRTC);
        assert!(webrtc.as_any().downcast_ref::<UnsupportedHandler>().is_some());
    }
}
