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

//! Structured error type for the protocol-agnostic API.
//!
//! Replaces `Result<_, String>` across the `ProtocolHandler` mechanism and the
//! `XrdsNet` intent verbs, so "this protocol doesn't support that verb" and
//! "you forgot to fill in a required field" are programmatically distinguishable
//! from a generic network failure — not just human-readable strings. See
//! `docs/xrds-net-protocol-handler.md`'s "`NetError`: structured, not
//! stringly-typed".

use crate::common::enums::PROTOCOLS;

#[derive(Debug, Clone)]
pub enum NetError {
    /// A URL scheme with no known protocol mapping (see `scheme_to_protocol`).
    UnrecognizedScheme(String),

    /// The protocol fundamentally does not support the requested verb (e.g.
    /// `request()` on FTP), or supports it only as a declared opt-in that
    /// isn't met (e.g. `request()` on an MQTT broker with no request/response
    /// support declared). `verb` is one of the four intent verbs
    /// (`"request"`/`"dispatch"`/`"listen"`/`"transfer"`) or a `Client`
    /// session-API verb (`"connect"`/`"send"`/`"rcv"`/`"close"`).
    Capability {
        protocol: PROTOCOLS,
        verb: &'static str,
        detail: String,
    },

    /// Required input is missing before the operation can even be attempted
    /// (e.g. FTP credentials). `hint` must say what to fill in — this is the
    /// "guided error" case, not just "something's missing."
    MissingInput {
        protocol: PROTOCOLS,
        field: &'static str,
        hint: String,
    },

    /// Underlying I/O/transport failure (socket errors, DNS, TLS, ...).
    Network(String),

    /// Server/protocol-level rejection (auth failed, broker refused the
    /// connection, FTP login denied, ...).
    Protocol(String),
}

impl NetError {
    pub fn capability(protocol: PROTOCOLS, verb: &'static str, detail: impl Into<String>) -> Self {
        NetError::Capability {
            protocol,
            verb,
            detail: detail.into(),
        }
    }

    pub fn missing_input(
        protocol: PROTOCOLS,
        field: &'static str,
        hint: impl Into<String>,
    ) -> Self {
        NetError::MissingInput {
            protocol,
            field,
            hint: hint.into(),
        }
    }
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetError::UnrecognizedScheme(scheme) => {
                write!(f, "unrecognized URL scheme: '{scheme}'")
            }
            NetError::Capability {
                protocol,
                verb,
                detail,
            } => write!(f, "{protocol:?} does not support '{verb}': {detail}"),
            NetError::MissingInput {
                protocol,
                field,
                hint,
            } => write!(f, "{protocol:?} is missing required '{field}' — {hint}"),
            NetError::Network(msg) => write!(f, "network error: {msg}"),
            NetError::Protocol(msg) => write!(f, "protocol error: {msg}"),
        }
    }
}

impl std::error::Error for NetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_message_names_protocol_and_verb() {
        let err = NetError::capability(PROTOCOLS::FTP, "request", "FTP is transfer-only");
        let msg = err.to_string();
        assert!(msg.contains("FTP"));
        assert!(msg.contains("request"));
        assert!(msg.contains("transfer-only"));
    }

    #[test]
    fn missing_input_message_includes_hint() {
        let err = NetError::missing_input(
            PROTOCOLS::FTP,
            "user/password",
            "call .set_user(...) and .set_password(...) before .connect()",
        );
        let msg = err.to_string();
        assert!(msg.contains("user/password"));
        assert!(msg.contains(".set_user("));
    }
}
