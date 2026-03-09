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

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
// Websocket
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type XrdsClient = Option<Arc<Mutex<WebSocketStream<MaybeTlsStream<TcpStream>>>>>;

#[derive(Clone)]
pub struct XrdsWebsocket {
    raw_url: Option<String>,
    ws_client: XrdsClient,
    rt: Option<Arc<Runtime>>,
}

impl Default for XrdsWebsocket {
    fn default() -> Self {
        Self::new()
    }
}

impl XrdsWebsocket {
    pub fn new() -> Self {
        XrdsWebsocket {
            raw_url: None,
            ws_client: None,
            rt: None,
        }
    }

    pub fn connect(mut self, raw_url: &str) -> Result<Self, String> {
        self.raw_url = Some(raw_url.to_string());
        let rt = Arc::new(Runtime::new().map_err(|e| e.to_string())?);
        let client_result = rt.block_on(connect_async(raw_url));

        if client_result.is_err() {
            Err(client_result.err().unwrap().to_string())
        } else if let Ok((client, _)) = client_result {
            self.ws_client = Some(Arc::new(Mutex::new(client)));
            self.rt = Some(rt);
            Ok(self)
        } else {
            Err("Failed to connect to the WebSocket server.".to_string())
        }
    }

    pub fn send_ws(&self, msg_type: Option<&str>, message: Vec<u8>) -> Result<Self, String> {
        let mut ws_client = self.ws_client.clone();
        let mut client = ws_client.as_mut().unwrap().lock().unwrap();
        let rt = self
            .rt
            .as_ref()
            .ok_or("WebSocket runtime is not initialized.".to_string())?;

        let message_type = msg_type.unwrap_or("binary");
        let binding = message_type.to_lowercase().clone();
        let message_type = binding.as_str();

        let message = match message_type {
            "text" => {
                let text = String::from_utf8(message)
                    .map_err(|e| format!("Invalid UTF-8 payload for text message: {}", e))?;
                Message::Text(text.into())
            }
            "binary" => Message::Binary(message.into()),
            _ => return Err("Invalid message type".to_string()),
        };

        let send_result = rt.block_on(async { client.send(message).await });

        if send_result.is_err() {
            Err(send_result.err().unwrap().to_string())
        } else {
            Ok(self.clone())
        }
    }

    pub fn rcv_ws(&self) -> Result<Vec<u8>, String> {
        let ws_client = self.ws_client.as_ref().unwrap();
        let mut ws_client = ws_client.lock().unwrap();
        let rt = self
            .rt
            .as_ref()
            .ok_or("WebSocket runtime is not initialized.".to_string())?;
        let message = rt.block_on(async { ws_client.next().await });

        if let Some(Ok(message)) = message {
            match message {
                Message::Binary(data) => Ok(data.to_vec()),
                Message::Text(data) => Ok(data.to_string().into_bytes()),
                _ => Err("The received message is not binary.".to_string()),
            }
        } else if let Some(Err(err)) = message {
            Err(err.to_string())
        } else {
            Err("WebSocket stream ended.".to_string())
        }
    }

    pub fn close_ws(&self) -> Result<(), String> {
        let ws_client = self.ws_client.as_ref().unwrap();
        let mut ws_client = ws_client.lock().unwrap();
        let rt = self
            .rt
            .as_ref()
            .ok_or("WebSocket runtime is not initialized.".to_string())?;
        let close_result = rt.block_on(async { ws_client.send(Message::Close(None)).await });
        if close_result.is_err() {
            Err(close_result.err().unwrap().to_string())
        } else {
            Ok(())
        }
    }
}
