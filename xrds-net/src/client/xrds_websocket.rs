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

use std::sync::{Arc, Mutex};

// Websocket
use websocket::client::sync::Client as WS_Client;
use websocket::stream::sync::NetworkStream;
use websocket::message::OwnedMessage;

#[derive(Clone)]
pub struct XrdsWebsocket {
    client_id: Option<String>,
    raw_url: Option<String>,
    ws_client: Option<Arc<Mutex<WS_Client<Box<dyn NetworkStream + Send>>>>>,
}

impl XrdsWebsocket {
    pub fn new() -> Self {
        XrdsWebsocket {
            client_id: None,
            raw_url: None,
            ws_client: None,        
        }
    }

    pub fn connect(mut self, raw_url: &str) -> Result<Self, String> {
        self.raw_url = Some(raw_url.to_string());
        let client_result = websocket::ClientBuilder::new(raw_url).unwrap().connect(None);
        
        if client_result.is_err() {
            return Err(client_result.err().unwrap().to_string());
        } else {
            self.ws_client = Some(Arc::new(Mutex::new(client_result.unwrap())));
            return Ok(self);
        }
    }

    pub fn send_ws(&self, msg_type: Option<&str>, message: Vec<u8>) -> Result<Self, String> {
        let mut ws_client = self.ws_client.clone();
        let mut client = ws_client.as_mut().unwrap().lock().unwrap();

        let message_type = match msg_type {
            Some(t) => t,
            None => "binary",
        };

        let binding = message_type.to_lowercase().clone();
        let message_type = binding.as_str();

        let message = match message_type {
            "text" => OwnedMessage::Text(String::from_utf8(message).unwrap()),
            "binary" => OwnedMessage::Binary(message),
            _ => return Err("Invalid message type".to_string()),
        };

        let send_result = client.send_message(&message);

        if send_result.is_err() {
            return Err(send_result.err().unwrap().to_string());
        } else {
            return Ok(self.clone());
        }
    }

    pub fn rcv_ws(&self) -> Result<Vec<u8>, String> {
        let ws_client = self.ws_client.as_ref().unwrap();
        let message = ws_client
            .lock().unwrap()
            .recv_message();

        if message.is_err() {
            return Err(message.err().unwrap().to_string());
        } else {
            let message = message.unwrap();
            match message {
                OwnedMessage::Binary(data) => {
                    return Ok(data);
                },
                OwnedMessage::Text(data) => {
                    return Ok(data.into_bytes());
                },
                _ => {
                    return Err("The received message is not binary.".to_string());
                }
            }
        }
    }

    pub fn close_ws(&self) -> Result<(), String> {
        let ws_client = self.ws_client.as_ref().unwrap();
        let close_msg = OwnedMessage::Close(None);
        let close_result = ws_client.lock().unwrap().send_message(&close_msg);
        if close_result.is_err() {
            return Err(close_result.err().unwrap().to_string());
        } else {
            return Ok(());
        }
    }
}