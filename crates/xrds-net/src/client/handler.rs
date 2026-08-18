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
#[cfg(feature = "protocol-coap")]
use super::protocols::coap::CoapHandler;
#[cfg(feature = "protocol-ftp")]
use super::protocols::ftp::FtpHandler;
#[cfg(feature = "protocol-http")]
use super::protocols::http::HttpHandler;
#[cfg(feature = "protocol-quic")]
use super::protocols::http3::Http3Handler;
#[cfg(feature = "protocol-mqtt")]
use super::protocols::mqtt::MqttHandler;
#[cfg(feature = "protocol-quic")]
use super::protocols::quic::QuicHandler;
#[cfg(feature = "protocol-ws")]
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

/// Why a protocol has no working handler.
///
/// These two cases need to read differently to a caller, which is the whole point of
/// carrying a reason. "This protocol does not do request/response" is a permanent fact
/// about the protocol; "this build has it compiled out" is a fact about *your* Cargo
/// features and is fixable by changing one line. Reporting the second as the first would
/// send someone looking for a protocol limitation that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedReason {
    /// The protocol is real and usable, but not through `ProtocolHandler` — it has its own
    /// richer API. `WEBRTC` is the standing case: media is multi-track and its data channel
    /// has an asymmetric publisher/subscriber lifecycle, so it deliberately stays on
    /// `WebRTCClient` (see `docs/done/xrds-net-protocol-handler.md`).
    DedicatedApi { api: &'static str },

    /// Compiled out by a Cargo feature. The scheme still parses and the `PROTOCOLS`
    /// variant still exists — only the handler is absent — so this must say which feature
    /// to enable.
    FeatureDisabled { feature: &'static str },
}

/// Placeholder for protocols with no handler in this build.
///
/// Implements neither `request` nor any capability query, so every verb reaches the
/// trait's default bodies and reports `NetError::Capability` — nothing silently no-ops.
/// See `docs/done/xrds-net-protocol-features-plan.md`.
pub struct UnsupportedHandler {
    protocol: PROTOCOLS,
    reason: UnsupportedReason,
}

impl UnsupportedHandler {
    /// A protocol that lives on its own API rather than this dispatch.
    pub fn dedicated_api(protocol: PROTOCOLS, api: &'static str) -> Self {
        Self {
            protocol,
            reason: UnsupportedReason::DedicatedApi { api },
        }
    }

    /// A protocol compiled out by a Cargo feature.
    pub fn feature_disabled(protocol: PROTOCOLS, feature: &'static str) -> Self {
        Self {
            protocol,
            reason: UnsupportedReason::FeatureDisabled { feature },
        }
    }

    pub fn protocol(&self) -> PROTOCOLS {
        self.protocol
    }

    pub fn reason(&self) -> UnsupportedReason {
        self.reason
    }

    fn detail(&self, verb: &str) -> String {
        match self.reason {
            UnsupportedReason::DedicatedApi { api } => format!(
                "{:?} does not support '{verb}' through this API; use {api} instead",
                self.protocol
            ),
            UnsupportedReason::FeatureDisabled { feature } => format!(
                "{:?} support is not compiled into this build; enable the '{feature}' \
                 feature of xrds-net",
                self.protocol
            ),
        }
    }
}

impl ProtocolHandler for UnsupportedHandler {
    /// Fail here, before the verb runs — but only for a compiled-out protocol.
    ///
    /// `validate` is the earliest point a caller reaches (`Client::connect`,
    /// `NetIntent::listen` and friends all call it first), so a missing feature surfaces at
    /// connect time rather than on first send. That matters because the alternative is an
    /// app that starts, appears healthy, and does nothing — the failure mode this whole
    /// design exists to avoid.
    ///
    /// `DedicatedApi` deliberately still validates `Ok`: WebRTC is genuinely available,
    /// just not through this dispatch, and its own API path does its own checks.
    fn validate(&self, _ctx: &ClientContext) -> Result<(), NetError> {
        match self.reason {
            UnsupportedReason::DedicatedApi { .. } => Ok(()),
            UnsupportedReason::FeatureDisabled { .. } => Err(NetError::capability(
                self.protocol,
                "connect",
                self.detail("connect"),
            )),
        }
    }

    /// Overridden purely to carry the reason; the trait default already errors, but with a
    /// message that would misattribute a missing feature to a protocol limitation.
    fn request(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        Err(NetError::capability(
            ctx.protocol,
            "request",
            self.detail("request"),
        ))
    }

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
        #[cfg(feature = "protocol-http")]
        PROTOCOLS::HTTP | PROTOCOLS::HTTPS | PROTOCOLS::FILE => Box::new(HttpHandler::new()),
        #[cfg(not(feature = "protocol-http"))]
        PROTOCOLS::HTTP | PROTOCOLS::HTTPS | PROTOCOLS::FILE => Box::new(UnsupportedHandler::feature_disabled(protocol, "protocol-http")),
        #[cfg(feature = "protocol-coap")]
        PROTOCOLS::COAP => Box::new(CoapHandler::new()),
        #[cfg(not(feature = "protocol-coap"))]
        PROTOCOLS::COAP => Box::new(UnsupportedHandler::feature_disabled(protocol, "protocol-coap")),
        #[cfg(feature = "protocol-quic")]
        PROTOCOLS::HTTP3 => Box::new(Http3Handler::new()),
        #[cfg(not(feature = "protocol-quic"))]
        PROTOCOLS::HTTP3 => Box::new(UnsupportedHandler::feature_disabled(protocol, "protocol-quic")),
        #[cfg(feature = "protocol-quic")]
        PROTOCOLS::QUIC => Box::new(QuicHandler::new()),
        #[cfg(not(feature = "protocol-quic"))]
        PROTOCOLS::QUIC => Box::new(UnsupportedHandler::feature_disabled(protocol, "protocol-quic")),
        #[cfg(feature = "protocol-ws")]
        PROTOCOLS::WS | PROTOCOLS::WSS => Box::new(WsHandler::new()),
        #[cfg(not(feature = "protocol-ws"))]
        PROTOCOLS::WS | PROTOCOLS::WSS => Box::new(UnsupportedHandler::feature_disabled(protocol, "protocol-ws")),
        #[cfg(feature = "protocol-mqtt")]
        PROTOCOLS::MQTT => Box::new(MqttHandler::new()),
        #[cfg(not(feature = "protocol-mqtt"))]
        PROTOCOLS::MQTT => Box::new(UnsupportedHandler::feature_disabled(protocol, "protocol-mqtt")),
        #[cfg(feature = "protocol-ftp")]
        PROTOCOLS::FTP | PROTOCOLS::SFTP => Box::new(FtpHandler::new()),
        #[cfg(not(feature = "protocol-ftp"))]
        PROTOCOLS::FTP | PROTOCOLS::SFTP => Box::new(UnsupportedHandler::feature_disabled(protocol, "protocol-ftp")),
        // Two different reasons, and the message has to say which: with the feature on,
        // WebRTC is available via its own API; with it off, it is not in this build at all.
        #[cfg(feature = "protocol-webrtc")]
        PROTOCOLS::WEBRTC => Box::new(UnsupportedHandler::dedicated_api(protocol, "WebRTCClient")),
        #[cfg(not(feature = "protocol-webrtc"))]
        PROTOCOLS::WEBRTC => Box::new(UnsupportedHandler::feature_disabled(
            protocol,
            "protocol-webrtc",
        )),
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
        let handler = UnsupportedHandler::dedicated_api(PROTOCOLS::WEBRTC, "WebRTCClient");
        let err = handler
            .request(&ctx(PROTOCOLS::WEBRTC))
            .expect_err("should not support request");
        assert!(matches!(err, NetError::Capability { verb: "request", .. }));
    }

    #[test]
    fn unsupported_handler_has_no_stream_or_file_transfer_shape() {
        let mut handler = UnsupportedHandler::dedicated_api(PROTOCOLS::WEBRTC, "WebRTCClient");
        assert!(handler.as_stream().is_none());
        assert!(handler.as_file_transfer().is_none());
    }

    #[test]
    fn default_validate_is_ok() {
        let handler = UnsupportedHandler::dedicated_api(PROTOCOLS::WEBRTC, "WebRTCClient");
        assert!(handler.validate(&ctx(PROTOCOLS::WEBRTC)).is_ok());
    }

    /// A compiled-out protocol has to fail at `validate`, which is the first thing every
    /// caller runs (`Client::connect`, `NetIntent::listen`, ...). If it only failed later,
    /// on first send, an app would start and look healthy while doing nothing.
    #[test]
    fn a_feature_disabled_protocol_fails_at_validate_not_on_first_use() {
        let handler = UnsupportedHandler::feature_disabled(PROTOCOLS::MQTT, "protocol-mqtt");
        let err = handler
            .validate(&ctx(PROTOCOLS::MQTT))
            .expect_err("a compiled-out protocol must not validate");
        match err {
            NetError::Capability { protocol, detail, .. } => {
                assert_eq!(protocol, PROTOCOLS::MQTT);
                // The message must name the feature, or the reader has no way to act on it.
                assert!(
                    detail.contains("protocol-mqtt"),
                    "detail should name the feature to enable, got: {detail}"
                );
            }
            other => panic!("expected Capability, got {other:?}"),
        }
    }

    /// The distinction is the entire reason `UnsupportedReason` exists: "this protocol
    /// cannot do that" and "you compiled it out" send a reader to different places.
    #[test]
    fn a_missing_feature_is_not_reported_as_a_protocol_limitation() {
        let disabled = UnsupportedHandler::feature_disabled(PROTOCOLS::MQTT, "protocol-mqtt");
        let dedicated = UnsupportedHandler::dedicated_api(PROTOCOLS::WEBRTC, "WebRTCClient");

        let d = match disabled.request(&ctx(PROTOCOLS::MQTT)).unwrap_err() {
            NetError::Capability { detail, .. } => detail,
            other => panic!("expected Capability, got {other:?}"),
        };
        assert!(d.contains("not compiled into this build"));
        assert!(d.contains("protocol-mqtt"));

        let w = match dedicated.request(&ctx(PROTOCOLS::WEBRTC)).unwrap_err() {
            NetError::Capability { detail, .. } => detail,
            other => panic!("expected Capability, got {other:?}"),
        };
        assert!(w.contains("WebRTCClient"), "should point at the real API, got: {w}");
        assert!(
            !w.contains("compiled"),
            "a dedicated-API protocol is present; must not imply a build problem"
        );
    }

    /// WebRTC is genuinely available, just not through this dispatch, and its own path does
    /// its own checks — so it must keep validating Ok. Guards against the previous test's
    /// fix being applied too broadly.
    #[test]
    fn a_dedicated_api_protocol_still_validates_ok() {
        let handler = UnsupportedHandler::dedicated_api(PROTOCOLS::WEBRTC, "WebRTCClient");
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
