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



use crate::common::enums::PROTOCOLS;

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

pub struct Url {
    pub scheme: String,
    pub host: String,
    pub port: u32,
    pub path: String,

    // Optional fields
    pub query: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}