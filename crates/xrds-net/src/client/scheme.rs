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

//! URL scheme -> `PROTOCOLS` inference, the mechanism behind
//! `ClientBuilder::from_url` and every `XrdsNet` intent verb. See
//! `docs/done/xrds-net-protocol-handler.md`'s "Scheme -> protocol mapping".
//!
//! `HTTP3` has no scheme of its own on purpose — reached only via the
//! `ClientBuilder::set_protocol` expert override until real ALPN-based
//! negotiation exists. `quic://` is an SDK-specific convention (raw QUIC
//! channel), not a registered standard scheme.

use crate::common::enums::PROTOCOLS;

use super::error::NetError;

pub fn scheme_to_protocol(scheme: &str) -> Result<PROTOCOLS, NetError> {
    match scheme {
        "http" => Ok(PROTOCOLS::HTTP),
        "https" => Ok(PROTOCOLS::HTTPS),
        "file" => Ok(PROTOCOLS::FILE),
        "coap" => Ok(PROTOCOLS::COAP),
        "ws" => Ok(PROTOCOLS::WS),
        "wss" => Ok(PROTOCOLS::WSS),
        "mqtt" => Ok(PROTOCOLS::MQTT),
        "ftp" => Ok(PROTOCOLS::FTP),
        "sftp" => Ok(PROTOCOLS::SFTP),
        "quic" => Ok(PROTOCOLS::QUIC),
        other => Err(NetError::UnrecognizedScheme(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_documented_scheme() {
        let cases = [
            ("http", PROTOCOLS::HTTP),
            ("https", PROTOCOLS::HTTPS),
            ("file", PROTOCOLS::FILE),
            ("coap", PROTOCOLS::COAP),
            ("ws", PROTOCOLS::WS),
            ("wss", PROTOCOLS::WSS),
            ("mqtt", PROTOCOLS::MQTT),
            ("ftp", PROTOCOLS::FTP),
            ("sftp", PROTOCOLS::SFTP),
            ("quic", PROTOCOLS::QUIC),
        ];
        for (scheme, expected) in cases {
            assert_eq!(scheme_to_protocol(scheme).unwrap(), expected, "scheme: {scheme}");
        }
    }

    #[test]
    fn http3_has_no_scheme_of_its_own() {
        // HTTP3 is reached only via the explicit set_protocol override, never
        // by scheme inference (no real ALPN negotiation yet).
        assert!(scheme_to_protocol("http3").is_err());
    }

    #[test]
    fn unrecognized_scheme_is_a_clear_error_not_a_panic() {
        let err = scheme_to_protocol("gopher").expect_err("gopher is not mapped");
        assert!(matches!(err, NetError::UnrecognizedScheme(s) if s == "gopher"));
    }
}
