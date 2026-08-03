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

use webrtc::ice_transport::ice_server::RTCIceServer;

/// Env vars TURN credentials are read from — see `build_ice_servers()`.
/// See docs/done/xrds-net-release-readiness.md Phase 1.
pub const TURN_USERNAME_ENV: &str = "XRDS_TURN_USERNAME";
pub const TURN_PASSWORD_ENV: &str = "XRDS_TURN_PASSWORD";

pub(crate) struct TurnCredentials {
    pub username: String,
    pub password: String,
}

/// Reads TURN credentials from `XRDS_TURN_USERNAME`/`XRDS_TURN_PASSWORD`.
/// `None` if either is unset — the caller then omits the TURN entry
/// entirely rather than falling back to a hardcoded default (see
/// docs/done/xrds-net-release-readiness.md Phase 1: these used to be committed
/// literals in this file).
fn turn_credentials_from_env() -> Option<TurnCredentials> {
    let username = std::env::var(TURN_USERNAME_ENV).ok()?;
    let password = std::env::var(TURN_PASSWORD_ENV).ok()?;
    Some(TurnCredentials { username, password })
}

/// The STUN/TURN servers offered to every peer connection. This is the
/// single source of truth — `WebRTCClient::setup_webrtc` (the code path
/// that actually creates every real peer connection, publisher and
/// subscriber alike) and `WebRTCServer::setup_webrtc` (currently unused for
/// live handshakes — reserved for a future SFU mode) both call this rather
/// than each keeping their own copy. Two independent copies is exactly how
/// the `turn:`/`turns:` scheme bug below got fixed once and silently
/// un-fixed itself: the fix landed in one copy and never propagated to the
/// other, which was the one actually used by live handshakes.
///
/// See docs/done/xrds-net-webrtc-ice-config-fix.md.
pub(crate) fn build_ice_servers() -> Vec<RTCIceServer> {
    let credentials = turn_credentials_from_env();
    if credentials.is_none() {
        log::warn!(
            "{TURN_USERNAME_ENV}/{TURN_PASSWORD_ENV} not set — WebRTC \
             connections will use STUN only, with no TURN relay fallback \
             for restrictive NATs. See MANUAL_WEBRTC.md for how to \
             configure a TURN server."
        );
    }
    build_ice_servers_with(credentials)
}

/// The testable core of `build_ice_servers()` — takes credentials directly
/// instead of reading the environment, so tests don't need to mutate
/// process-global env vars (which races under parallel test execution).
fn build_ice_servers_with(credentials: Option<TurnCredentials>) -> Vec<RTCIceServer> {
    let mut servers = vec![
        // STUN servers for NAT discovery
        RTCIceServer {
            urls: vec![
                "stun:stun.l.google.com:19302".to_owned(),
                "stun:stun1.l.google.com:3478".to_owned(),
                "stun:stun2.l.google.com:19302".to_owned(),
                "stun:stun.keti.xrds.kr:13478".to_owned(),
                "stun:stun.keti.xrds.kr:13478?transport=tcp".to_owned(),
                "stun:stun.keti.xrds.kr:13479".to_owned(),
                "stun:stun.keti.xrds.kr:13479?transport=tcp".to_owned(),
            ],
            ..Default::default()
        },
    ];

    // TURN server for relay when direct connection fails.
    // 13478 = default (plain UDP/TCP); 13479 = secure — TURN-over-TLS,
    // which needs the `turns:` scheme, not `turn:` on a different port
    // (that mismatch is rejected by the ICE agent: "Unable to handle URL
    // in gather_candidates_relay turn:...13479?transport=tcp").
    if let Some(TurnCredentials { username, password }) = credentials {
        servers.push(RTCIceServer {
            urls: vec![
                "turn:turn.keti.xrds.kr:13478".to_owned(),
                "turn:turn.keti.xrds.kr:13478?transport=tcp".to_owned(),
                "turns:turn.keti.xrds.kr:13479?transport=tcp".to_owned(),
            ],
            username,
            credential: password,
        });
    }

    servers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_credentials() -> TurnCredentials {
        TurnCredentials {
            username: "test-user".to_owned(),
            password: "test-pass".to_owned(),
        }
    }

    /// This is the exact class of bug fixed in
    /// docs/done/xrds-net-crypto-consolidation.md (TURN-over-TLS on port 13479
    /// using `turn:` instead of `turns:`, rejected by the ICE agent), which
    /// then reappeared in a second, un-synced copy of this list (see
    /// docs/done/xrds-net-webrtc-ice-config-fix.md). Both `WebRTCClient` and
    /// `WebRTCServer` now call this single function, so this test covers
    /// every caller — there is no copy left for a fix to miss.
    #[test]
    fn build_ice_servers_uses_turns_scheme_for_the_tls_secured_port() {
        let servers = build_ice_servers_with(Some(some_credentials()));

        let stun = &servers[0];
        assert!(
            stun.urls.iter().all(|u| u.starts_with("stun:")),
            "STUN entries must use the stun: scheme: {:?}",
            stun.urls
        );

        let turn = &servers[1];
        let plain: Vec<_> = turn.urls.iter().filter(|u| u.contains(":13478")).collect();
        assert!(!plain.is_empty(), "expected a plain (13478) TURN entry");
        assert!(
            plain.iter().all(|u| u.starts_with("turn:")),
            "plain TURN entries (port 13478) must use turn:, got {:?}",
            plain
        );

        let secure: Vec<_> = turn.urls.iter().filter(|u| u.contains(":13479")).collect();
        assert!(!secure.is_empty(), "expected a TLS-secured (13479) TURN entry");
        assert!(
            secure.iter().all(|u| u.starts_with("turns:")),
            "the TLS-secured TURN entry (port 13479) must use turns:, not turn: — got {:?}",
            secure
        );
    }

    #[test]
    fn turn_credentials_are_carried_through_to_the_turn_entry() {
        let servers = build_ice_servers_with(Some(some_credentials()));
        let turn = &servers[1];
        assert_eq!(turn.username, "test-user");
        assert_eq!(turn.credential, "test-pass");
    }

    #[test]
    fn no_credentials_omits_the_turn_entry_but_keeps_stun() {
        let servers = build_ice_servers_with(None);
        assert_eq!(
            servers.len(),
            1,
            "expected STUN-only (no TURN entry) when no credentials are configured: {servers:?}"
        );
        assert!(servers[0].urls.iter().all(|u| u.starts_with("stun:")));
    }
}
