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

//! `CoapHandler`: CoAP, request-shaped only (mirrors `HttpHandler`'s
//! stateless GET pattern, just over a different transport).
//!
//! Extracted verbatim from `client.rs`'s `request_coap`/`run_coap` (Phase 1
//! of `docs/xrds-net-protocol-handler.md`) — `Client`'s old methods are
//! untouched and still the ones actually called until Phase 2 rewires
//! `Client` onto this handler.

use coap::UdpCoAPClient;
use coap_lite::CoapResponse;

use crate::client::context::ClientContext;
use crate::client::error::NetError;
use crate::client::handler::ProtocolHandler;
use crate::common::coap_code_to_decimal;
use crate::common::data_structure::NetResponse;

#[derive(Default)]
pub struct CoapHandler;

impl CoapHandler {
    pub fn new() -> Self {
        Self
    }

    fn request_coap(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| NetError::Network(e.to_string()))?;
        let response = rt
            .block_on(Self::run_coap(&ctx.raw_url))
            .map_err(NetError::Network)?;

        let coap_res_header = response.message.header.clone();
        let coap_res_payload = response.message.payload.clone();

        let status_code_str = coap_res_header.code.to_string();
        let coap_status_code = coap_code_to_decimal(&status_code_str);

        let headers: Vec<(String, String)> = vec![
            ("Code".to_string(), coap_res_header.code.to_string()),
            (
                "Message ID".to_string(),
                coap_res_header.message_id.to_string(),
            ),
            (
                "Version".to_string(),
                coap_res_header.get_version().to_string(),
            ),
        ];

        let body_result = String::from_utf8(coap_res_payload);
        match body_result {
            Ok(body) => Ok(NetResponse {
                protocol: ctx.protocol,
                status_code: coap_status_code,
                headers,
                body: body.as_bytes().to_vec(),
                error: None,
            }),
            Err(_) => Ok(NetResponse {
                protocol: ctx.protocol,
                status_code: coap_status_code,
                headers,
                body: vec![],
                error: Some("Failed to parse CoAP response body as UTF-8 string.".to_string()),
            }),
        }
    }

    async fn run_coap(raw_url: &str) -> Result<CoapResponse, String> {
        let response = UdpCoAPClient::get(raw_url).await;

        match response {
            Ok(res) => Ok(res),
            Err(e) => Err(format!("Failed to get CoAP response: {e:?}")),
        }
    }
}

impl ProtocolHandler for CoapHandler {
    fn request(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        self.request_coap(ctx)
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

    fn ctx_for(url: &str) -> ClientContext {
        let mut ctx = ClientContext::new(PROTOCOLS::COAP, "test-id".to_string());
        ctx.raw_url = url.to_string();
        ctx
    }

    #[test]
    fn bad_host_is_a_network_error_not_a_panic() {
        let handler = CoapHandler::new();
        let ctx = ctx_for("coap://does-not-exist.invalid/");

        let err = handler.request(&ctx).expect_err("bad host should fail");
        assert!(matches!(err, NetError::Network(_)));
    }
}
