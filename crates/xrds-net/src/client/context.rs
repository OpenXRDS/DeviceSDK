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

//! `ClientContext`: the request/config state a [`ProtocolHandler`] reads.
//!
//! This is everything `Client` holds today *except* the protocol-specific
//! connection state (the `ws_client`/`ftp_stream`/`mqtt_client`/
//! `quic_connection`/... fields) — those move into each handler's own struct
//! in Phase 1. Handlers stay stateless w.r.t. config; they only own their
//! *connection* state. See `docs/xrds-net-protocol-handler.md`'s
//! "`ProtocolHandler` mechanism".

use crate::common::data_structure::XrUrl;
use crate::common::enums::PROTOCOLS;
use crate::common::parse_url;

use super::error::NetError;

// req_headers/req_body/timeout/redirection/method aren't read by anything yet
// in Phase 0 — handlers start reading them in Phase 1, and Client::set_*
// starts populating them in Phase 2.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ClientContext {
    pub protocol: PROTOCOLS,
    pub raw_url: String,
    pub id: String,

    // Parsed from raw_url by parse_url_into_self(); not directly used for
    // connection or request, informational until parsed.
    pub url: Option<XrUrl>,
    pub host: Option<String>,
    pub port: Option<u32>,
    pub path: Option<String>,

    pub(crate) req_headers: Option<Vec<(String, String)>>,
    pub(crate) req_body: Option<String>,
    pub(crate) timeout: Option<u64>,
    pub(crate) redirection: bool,
    pub(crate) method: Option<String>,

    pub user: Option<String>,
    pub password: Option<String>,

    /// Skip TLS peer verification (QUIC today). Off by default; used for
    /// self-signed/dev servers and the crate's own QUIC round-trip test.
    /// Analogous to an HTTP client's `danger_accept_invalid_certs`.
    pub(crate) insecure: bool,
}

impl ClientContext {
    pub fn new(protocol: PROTOCOLS, id: String) -> Self {
        Self {
            protocol,
            raw_url: String::new(),
            id,
            url: None,
            host: None,
            port: None,
            path: None,
            req_headers: None,
            req_body: None,
            timeout: None,
            redirection: false,
            method: None,
            user: None,
            password: None,
            insecure: false,
        }
    }

    /// Parse `raw_url` and fill in `url`/`host`/`port`/`path` (folding the
    /// query string into `path`, matching the current behavior duplicated
    /// today in both `Client::connect()` and `Client::request()`).
    pub fn parse_url_into_self(&mut self) -> Result<(), NetError> {
        let parsed = parse_url(&self.raw_url).map_err(NetError::Network)?;

        self.host = Some(parsed.host.clone());
        self.port = Some(parsed.port);
        self.path = Some(match &parsed.query {
            Some(query) => format!("{}?{}", parsed.path, query),
            None => parsed.path.clone(),
        });
        // Only fill from the URL's embedded userinfo (`user:pass@host`) if
        // nothing was already set explicitly via `set_user`/`set_password` —
        // those calls stay authoritative for the expert `Client` API.
        if self.user.is_none() {
            self.user = parsed.username.clone();
        }
        if self.password.is_none() {
            self.password = parsed.password.clone();
        }
        self.url = Some(parsed);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_url_and_folds_query_into_path() {
        let mut ctx = ClientContext::new(PROTOCOLS::HTTP, "id".to_string());
        ctx.raw_url = "http://example.com/search?q=xr".to_string();

        ctx.parse_url_into_self().expect("should parse");

        assert_eq!(ctx.host.as_deref(), Some("example.com"));
        assert_eq!(ctx.port, Some(80));
        assert_eq!(ctx.path.as_deref(), Some("/search?q=xr"));
        assert!(ctx.url.is_some());
    }

    #[test]
    fn parses_url_without_query() {
        let mut ctx = ClientContext::new(PROTOCOLS::FTP, "id".to_string());
        ctx.raw_url = "ftp://files.example.com/reports".to_string();

        ctx.parse_url_into_self().expect("should parse");

        assert_eq!(ctx.path.as_deref(), Some("/reports"));
    }

    #[test]
    fn invalid_url_returns_network_error() {
        let mut ctx = ClientContext::new(PROTOCOLS::HTTP, "id".to_string());
        ctx.raw_url = "http://host:notaport/".to_string();

        let err = ctx.parse_url_into_self().expect_err("should fail to parse");
        assert!(matches!(err, NetError::Network(_)));
    }
}
