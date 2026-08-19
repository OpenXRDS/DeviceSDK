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

//! `HttpHandler`: HTTP and HTTPS, request-shaped and stateless (a fresh
//! `reqwest::blocking::Client` per call — there's no persistent connection to
//! hold, unlike the `StreamHandler`/`FileTransferHandler` protocols).
//!
//! `PROTOCOLS::FILE` is handled separately, by direct `std::fs::read` — not
//! through this HTTP client at all. `reqwest` can't fetch `file://` URLs (it's
//! HTTP-only); the previous `curl`-based implementation could, because curl's
//! URL API is scheme-agnostic. See docs/done/xrds-net-crypto-consolidation.md.
//!
//! Rustls-backed (`rustls-tls-webpki-roots`), replacing `curl`/OpenSSL as
//! part of the crypto-library consolidation — see
//! docs/done/xrds-net-crypto-consolidation.md. Feature parity with the old `curl`
//! implementation: GET/POST, custom request headers, a redirect on/off
//! toggle. `ctx.insecure` and `ctx.timeout` were never read by the old
//! implementation either (confirmed: `insecure` is QUIC-only) — not adding
//! them here would be scope creep, not a regression.

use reqwest::blocking::Client;
use reqwest::redirect::Policy;

use crate::client::context::ClientContext;
use crate::client::error::NetError;
use crate::client::handler::ProtocolHandler;
use crate::common::data_structure::NetResponse;
use crate::common::enums::PROTOCOLS;

#[derive(Default)]
pub struct HttpHandler;

impl HttpHandler {
    pub fn new() -> Self {
        Self
    }

    /// Currently GET and POST methods are supported.
    fn request_http(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        crate::common::ensure_rustls_crypto_provider();

        let redirect_policy = if ctx.redirection {
            Policy::limited(10)
        } else {
            Policy::none()
        };
        let client = Client::builder()
            .redirect(redirect_policy)
            .build()
            .map_err(net_err)?;

        let is_post = ctx.method.as_deref() == Some("POST");
        let mut builder = if is_post {
            client.post(ctx.raw_url.as_str())
        } else {
            client.get(ctx.raw_url.as_str())
        };

        if let Some(headers) = &ctx.req_headers {
            for (key, value) in headers.iter() {
                builder = builder.header(key, value);
            }
        }
        if is_post {
            if let Some(body) = &ctx.req_body {
                builder = builder.body(body.clone());
            }
        }

        let response = builder.send().map_err(net_err)?;
        Self::response_from(ctx, response)
    }

    /// Request to the server using FILE. Returns the file byte stream in
    /// `NetResponse.body`. Direct filesystem read, not an HTTP request — see
    /// the module doc for why this doesn't go through `reqwest`.
    fn request_file(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        let path = ctx
            .raw_url
            .strip_prefix("file://")
            .unwrap_or(ctx.raw_url.as_str());
        let body = std::fs::read(path).map_err(|e| NetError::Network(e.to_string()))?;

        Ok(NetResponse {
            protocol: ctx.protocol,
            status_code: 200,
            headers: Vec::new(),
            body,
            error: None,
        })
    }

    fn response_from(
        ctx: &ClientContext,
        response: reqwest::blocking::Response,
    ) -> Result<NetResponse, NetError> {
        let status_code = response.status().as_u16() as u32;
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = response.bytes().map_err(net_err)?.to_vec();

        Ok(NetResponse {
            protocol: ctx.protocol,
            status_code,
            headers,
            body,
            error: None,
        })
    }
}

fn net_err(e: reqwest::Error) -> NetError {
    NetError::Network(e.to_string())
}

impl ProtocolHandler for HttpHandler {
    fn request(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        match ctx.protocol {
            PROTOCOLS::FILE => self.request_file(ctx),
            _ => self.request_http(ctx), // HTTP, HTTPS
        }
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

    fn ctx_for(protocol: PROTOCOLS, url: &str) -> ClientContext {
        let mut ctx = ClientContext::new(protocol, "test-id".to_string());
        ctx.raw_url = url.to_string();
        ctx
    }

    #[test]
    #[ignore = "live network: hits www.rust-lang.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn get_request_with_redirect_returns_200() {
        let handler = HttpHandler::new();
        let mut ctx = ctx_for(PROTOCOLS::HTTP, "http://www.rust-lang.org:80/");
        ctx.redirection = true; // rust-lang.org redirects http -> https

        let response = handler.request(&ctx).expect("request should succeed");
        assert_eq!(response.protocol, PROTOCOLS::HTTP);
        assert_eq!(response.status_code, 200);
        assert!(!response.body.is_empty());
        assert!(!response.headers.is_empty());
    }

    #[test]
    #[ignore = "live network: hits www.rust-lang.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn get_request_without_redirect_returns_a_redirect_status() {
        let handler = HttpHandler::new();
        let ctx = ctx_for(PROTOCOLS::HTTP, "http://www.rust-lang.org:80/"); // redirection: false

        let response = handler.request(&ctx).expect("request should succeed");
        assert_ne!(response.status_code, 200);
    }

    #[test]
    fn wrong_host_name_is_a_network_error_not_a_panic() {
        let handler = HttpHandler::new();
        let ctx = ctx_for(PROTOCOLS::HTTP, "ww.w.clear.com");

        let err = handler.request(&ctx).expect_err("bad host should fail");
        assert!(matches!(err, NetError::Network(_)));
    }

    // FILE reads the local filesystem (`file://` or a bare path), so this
    // writes a temp file rather than hitting the network. The previous
    // curl-based version passed an `http://` URL here, which only worked
    // because curl's URL API is scheme-agnostic — that was never what the
    // `file` scheme is documented to mean.
    #[test]
    fn file_protocol_returns_bytes_in_body() {
        let path = std::env::temp_dir().join("xrds_net_file_protocol_test.bin");
        std::fs::write(&path, b"xrds-net file protocol test").expect("write temp file");

        let handler = HttpHandler::new();
        let ctx = ctx_for(PROTOCOLS::FILE, &format!("file://{}", path.display()));

        let response = handler.request(&ctx).expect("file request should succeed");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, b"xrds-net file protocol test");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_protocol_missing_file_is_a_network_error_not_a_panic() {
        let handler = HttpHandler::new();
        let ctx = ctx_for(PROTOCOLS::FILE, "file:///definitely/not/a/real/path.bin");

        let err = handler.request(&ctx).expect_err("missing file should fail");
        assert!(matches!(err, NetError::Network(_)));
    }
}
