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

use serde::{Serialize, Deserialize};

use crate::common::enums::{PROTOCOLS, FtpCommands};

use url::Url;

pub const CREATE_SESSION: &str = "create_session"; // publisher to server
pub const LIST_SESSIONS: &str = "list_sessions";   // subscriber to server
pub const JOIN_SESSION: &str = "join_session";     // subscriber to server
pub const LEAVE_SESSION: &str = "leave_session";   // subscriber to server
pub const CLOSE_SESSION: &str = "close_session";   // publisher to server
pub const LIST_PARTICIPANTS: &str = "list_participants"; // server to client (publisher or subscriber)
pub const OFFER: &str = "offer";                   // publisher to server, server to subscriber
pub const ANSWER: &str = "answer";                 // subscriber to server
pub const WELCOME: &str = "welcome";               // server to client (publisher or subscriber)
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

        write!(f, "Protocol: {:?}\nStatus Code: {}\nHeaders: {:?}\nBody: {:?}\nError: {:?}",
            self.protocol, self.status_code, self.headers, body_str, self.error)
    }
}

#[derive(Debug, Clone)]
pub struct FtpPayload {
    pub command: FtpCommands,
    pub payload_name: String,    // file / directory name, etc.
    pub payload: Option<Vec<u8>>,   // file content, etc.
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
            return Err("Invalid URL".to_string());
        } else {
            let sock_addr = sock_addr_result.unwrap();
            if sock_addr.len() == 0 {
                return Err("Invalid URL".to_string());
            } else {
                return Ok(sock_addr[0]);
            }
        }        
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebRTCMessage {
    pub client_id: String,
    pub session_id: String,
    pub message_type: String,
    pub ice_candidates: Option<String>, // ICE candidates, participants, etc.
    pub sdp: Option<String>,    // Session Description Protocol. base64 encoded
    pub error: Option<String>,
}