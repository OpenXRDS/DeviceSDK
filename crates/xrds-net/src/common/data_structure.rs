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

use serde::{Deserialize, Serialize};

use crate::common::enums::{FtpCommands, PROTOCOLS};

use url::Url;

pub const CREATE_SESSION: &str = "create_session"; // publisher to server
pub const LIST_SESSIONS: &str = "list_sessions"; // subscriber to server
pub const JOIN_SESSION: &str = "join_session"; // subscriber to server
pub const LEAVE_SESSION: &str = "leave_session"; // subscriber to server
pub const CLOSE_SESSION: &str = "close_session"; // publisher to server
pub const LIST_PARTICIPANTS: &str = "list_participants"; // server to client (publisher or subscriber)
pub const OFFER: &str = "offer"; // publisher to server, server to subscriber
pub const ANSWER: &str = "answer"; // subscriber to server
pub const WELCOME: &str = "welcome"; // server to client (publisher or subscriber)
pub const ICE_CANDIDATE: &str = "ice_candidate"; // publisher to server, server to subscriber
pub const ICE_CANDIDATE_ACK: &str = "ice_candidate_ack"; // subscriber to server

/**
 * In case of Using CoAP protocol, refer to the following link:
 * https://www.potaroo.net/ietf/all-ids/draft-castellani-core-http-mapping-07.html#rfc.section.4
 * The return code is different from the HTTP protocol.
 */
#[derive(Debug, Clone)]
pub struct NetResponse {
    pub protocol: PROTOCOLS,
    pub status_code: u32,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,

    // Optional fields
    pub error: Option<String>,
}

// implement display for NetResponse
impl std::fmt::Display for NetResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let body_str = String::from_utf8(self.body.clone()).unwrap();

        write!(
            f,
            "Protocol: {:?}\nStatus Code: {}\nHeaders: {:?}\nBody: {:?}\nError: {:?}",
            self.protocol, self.status_code, self.headers, body_str, self.error
        )
    }
}

#[derive(Debug, Clone)]
pub struct FtpPayload {
    pub command: FtpCommands,
    pub payload_name: String,     // file / directory name, etc.
    pub payload: Option<Vec<u8>>, // file content, etc.
}

#[derive(Debug, Clone)]
pub struct FtpResponse {
    pub payload: Option<Vec<u8>>,

    // Optional fields
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct XrUrl {
    pub scheme: String,
    pub host: String,
    pub port: u32,
    pub path: String,
    pub raw_url: String,

    // Optional fields
    pub query: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl XrUrl {
    pub fn socket_addrs(&self) -> Result<std::net::SocketAddr, String> {
        let url = Url::parse(self.raw_url.as_str());
        if url.is_err() {
            return Err("Invalid URL".to_string());
        }

        let url = url.unwrap();
        let sock_addr_result = url.socket_addrs(|| None);

        if sock_addr_result.is_err() {
            Err("Invalid URL".to_string())
        } else if let Ok(addr) = sock_addr_result {
            if addr.is_empty() {
                return Err("Empty URL".to_string());
            }
            Ok(addr[0])
        } else {
            Err("Invalid URL".to_string())
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebRTCMessage {
    pub client_id: String,
    pub session_id: String,
    pub message_type: String,
    pub ice_candidates: Option<String>, // ICE candidates, participants, etc.
    pub sdp: Option<String>,            // Session Description Protocol. base64 encoded
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips a `WebRTCMessage` for each signaling message type through
    /// `serde_json`, asserting every field survives — this is exercised
    /// today only indirectly, via full WebSocket round-trips in the
    /// integration suite.
    fn assert_round_trips(message: WebRTCMessage) {
        let json = serde_json::to_string(&message).expect("serialize");
        let decoded: WebRTCMessage = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.client_id, message.client_id);
        assert_eq!(decoded.session_id, message.session_id);
        assert_eq!(decoded.message_type, message.message_type);
        assert_eq!(decoded.ice_candidates, message.ice_candidates);
        assert_eq!(decoded.sdp, message.sdp);
        assert_eq!(decoded.error, message.error);
    }

    #[test]
    fn create_session_round_trips() {
        assert_round_trips(WebRTCMessage {
            client_id: "client-1".to_string(),
            session_id: "".to_string(),
            message_type: CREATE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        });
    }

    #[test]
    fn join_session_round_trips() {
        assert_round_trips(WebRTCMessage {
            client_id: "client-2".to_string(),
            session_id: "session-abc".to_string(),
            message_type: JOIN_SESSION.to_string(),
            ice_candidates: None,
            sdp: Some("v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\n".to_string()),
            error: None,
        });
    }

    #[test]
    fn offer_round_trips() {
        assert_round_trips(WebRTCMessage {
            client_id: "publisher".to_string(),
            session_id: "session-abc".to_string(),
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: Some("v=0\r\ns=offer\r\n".to_string()),
            error: None,
        });
    }

    #[test]
    fn answer_round_trips() {
        assert_round_trips(WebRTCMessage {
            client_id: "subscriber".to_string(),
            session_id: "session-abc".to_string(),
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp: Some("v=0\r\ns=answer\r\n".to_string()),
            error: None,
        });
    }

    #[test]
    fn ice_candidate_round_trips() {
        assert_round_trips(WebRTCMessage {
            client_id: "client-1".to_string(),
            session_id: "session-abc".to_string(),
            message_type: ICE_CANDIDATE.to_string(),
            ice_candidates: Some("[\"candidate:1 1 UDP 2122252543 10.0.0.1 54321 typ host\"]".to_string()),
            sdp: None,
            error: None,
        });
    }

    #[test]
    fn list_participants_round_trips_with_multiple_ids() {
        assert_round_trips(WebRTCMessage {
            client_id: "".to_string(),
            session_id: "session-abc".to_string(),
            message_type: LIST_PARTICIPANTS.to_string(),
            ice_candidates: Some("client-1,client-2".to_string()),
            sdp: None,
            error: None,
        });
    }

    #[test]
    fn error_field_round_trips() {
        assert_round_trips(WebRTCMessage {
            client_id: "client-1".to_string(),
            session_id: "session-abc".to_string(),
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: None,
            error: Some("session not found".to_string()),
        });
    }

    #[test]
    fn default_message_has_empty_strings_and_no_optionals() {
        let message = WebRTCMessage::default();
        assert_eq!(message.client_id, "");
        assert_eq!(message.session_id, "");
        assert_eq!(message.message_type, "");
        assert!(message.ice_candidates.is_none());
        assert!(message.sdp.is_none());
        assert!(message.error.is_none());
    }

    #[test]
    fn deserializing_missing_optional_fields_defaults_to_none() {
        let json = r#"{"client_id":"c","session_id":"s","message_type":"offer"}"#;
        let decoded: WebRTCMessage = serde_json::from_str(json).expect("deserialize");
        assert_eq!(decoded.client_id, "c");
        assert!(decoded.ice_candidates.is_none());
        assert!(decoded.sdp.is_none());
        assert!(decoded.error.is_none());
    }
}
