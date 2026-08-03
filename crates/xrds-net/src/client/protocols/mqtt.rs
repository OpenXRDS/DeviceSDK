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

//! `MqttHandler`: MQTT as a [`StreamHandler`], plus `get_connection()` as an
//! expert-only extra (raw `rumqttc` handle access via `as_any`).
//!
//! Extracted verbatim from `client.rs`'s `connect_mqtt`/`send_mqtt`/
//! `rcv_mqtt` (Phase 1 of `docs/done/xrds-net-protocol-handler.md`) — `Client`'s
//! old methods are untouched and still the ones actually called until Phase
//! 2 rewires `Client` onto this handler.
//!
//! `rumqttc::Event` is aliased to `MqttEvent` here since it collides by name
//! with `crate::client::event::Event` (the `StreamHandler`-wide type this
//! handler's `recv()` returns instead).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rumqttc::Client as MqttClient;
use rumqttc::Connection as MqttConnection;
use rumqttc::Event as MqttEvent;
use rumqttc::{Incoming, MqttOptions, QoS};

use crate::client::categories::StreamHandler;
use crate::client::context::ClientContext;
use crate::client::error::NetError;
use crate::client::event::Event;
use crate::client::handler::ProtocolHandler;

#[derive(Default)]
pub struct MqttHandler {
    client: Option<MqttClient>,
    connection: Option<Arc<Mutex<MqttConnection>>>,
}

impl MqttHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Expert-only extra: raw `rumqttc` connection handle, reached via
    /// `ProtocolHandler::as_any()` downcast.
    pub fn get_connection(&self) -> Option<Arc<Mutex<MqttConnection>>> {
        self.connection.clone()
    }

    /// Expert-only extra: subscribe + wait (up to 5s) for the SUBACK
    /// confirmation, mirroring the original `Client::mqtt_subscribe`.
    /// Reached via `ProtocolHandler::as_any_mut()` downcast.
    pub fn subscribe(&self, topic: &str) -> Result<(), NetError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| NetError::Network("MQTT client is not initialized.".to_string()))?;
        if self.connection.is_none() {
            return Err(NetError::Network("MQTT connection is not initialized.".to_string()));
        }

        client
            .subscribe(topic, QoS::AtMostOnce)
            .map_err(|e| NetError::Network(e.to_string()))?;

        // wait til subscription is confirmed with SUBACK in timeout
        let start_time = Instant::now();
        loop {
            if start_time.elapsed() > Duration::from_secs(5) {
                return Err(NetError::Network("Subscription confirmation timed out.".to_string()));
            }

            let msg = self.recv_raw()?;
            if msg.is_empty() {
                continue; // not a SUBACK message, keep waiting
            }
            if msg == b"SUBACK_CONFIRMED".to_vec() {
                return Ok(());
            }
        }
    }

    /// Receives the 'Publish' event only from the connection; other event
    /// kinds are surfaced as sentinel byte strings so `send`'s
    /// wait-for-PUBACK loop (below) can recognize them without a second
    /// event type leaking into the public `Event`.
    fn recv_raw(&self) -> Result<Vec<u8>, NetError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| NetError::Network("MQTT connection is not initialized.".to_string()))?;
        let notification = connection.lock().unwrap().recv();

        match notification {
            Err(recv_err) => Err(NetError::Network(format!(
                "Error occurred while receiving the message: {:?}",
                recv_err
            ))),
            Ok(inner_result) => match inner_result {
                Ok(event) => match event {
                    MqttEvent::Incoming(incoming) => match incoming {
                        Incoming::Publish(message) => Ok(Vec::from(message.payload)),
                        Incoming::Subscribe(message) => {
                            println!("Subscription success: {:?}", message);
                            Ok(Vec::new())
                        }
                        Incoming::SubAck(message) => {
                            println!("SubAck received: {:?}", message.return_codes);
                            Ok(b"SUBACK_CONFIRMED".to_vec())
                        }
                        Incoming::PubAck(message) => {
                            println!("PubAck received: {:?}", message);
                            Ok(b"PUBACK_CONFIRMED".to_vec())
                        }
                        _ => Ok(Vec::new()),
                    },
                    MqttEvent::Outgoing(_outgoing) => Ok(Vec::new()),
                },
                Err(conn_err) => Err(NetError::Network(format!(
                    "Error occurred while receiving the message: {:?}",
                    conn_err
                ))),
            },
        }
    }
}

impl StreamHandler for MqttHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError> {
        let host = ctx
            .host
            .as_ref()
            .ok_or_else(|| NetError::Network("host not set".to_string()))?;
        let port = ctx
            .port
            .ok_or_else(|| NetError::Network("port not set".to_string()))?;

        let mut mqtt_options = MqttOptions::new(ctx.id.as_str(), host, port.try_into().unwrap());
        mqtt_options.set_keep_alive(Duration::from_secs(5));

        let (client, connection) = MqttClient::new(mqtt_options, 10);
        self.client = Some(client);
        self.connection = Some(Arc::new(Mutex::new(connection)));

        Ok(())
    }

    /// Invokes 'publish' method of the mqtt client, then waits (up to 5s)
    /// for the PUBACK confirmation, matching the original blocking behavior.
    fn send(&mut self, _ctx: &ClientContext, topic: Option<&str>, message: Vec<u8>) -> Result<(), NetError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| NetError::Network("MQTT client is not initialized.".to_string()))?;
        if self.connection.is_none() {
            return Err(NetError::Network("MQTT connection is not initialized.".to_string()));
        }
        let topic = topic.ok_or_else(|| NetError::Network("MQTT publish requires a topic.".to_string()))?;

        client
            .publish(topic, QoS::AtLeastOnce, true, message)
            .map_err(|e| NetError::Network(e.to_string()))?;

        // wait for puback confirmation in timeout
        let start_time = Instant::now();
        loop {
            if start_time.elapsed() > Duration::from_secs(5) {
                return Err(NetError::Network("Publish confirmation timed out.".to_string()));
            }

            let msg = self.recv_raw()?;
            if msg == b"PUBACK_CONFIRMED".to_vec() {
                return Ok(());
            }
        }
    }

    fn recv(&mut self, _ctx: &ClientContext) -> Result<Event, NetError> {
        let payload = self.recv_raw()?;
        Ok(Event::new(None, payload))
    }

    fn close(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
        Err(NetError::capability(
            crate::common::enums::PROTOCOLS::MQTT,
            "close",
            "MQTT has no explicit close in this SDK yet; drop the handler instead",
        ))
    }
}

impl ProtocolHandler for MqttHandler {
    fn as_stream(&mut self) -> Option<&mut dyn StreamHandler> {
        Some(self)
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

    #[test]
    fn connect_populates_client_and_connection() {
        let mut handler = MqttHandler::new();
        let mut ctx = ClientContext::new(PROTOCOLS::MQTT, "test-id".to_string());
        ctx.raw_url = "mqtt://127.0.0.1:1883/".to_string();
        ctx.parse_url_into_self().expect("should parse");

        handler.connect(&ctx).expect("connect should succeed (no network I/O yet)");
        assert!(handler.get_connection().is_some());
    }

    #[test]
    fn send_without_a_topic_is_a_network_error_not_a_panic() {
        let mut handler = MqttHandler::new();
        let mut ctx = ClientContext::new(PROTOCOLS::MQTT, "test-id".to_string());
        ctx.raw_url = "mqtt://127.0.0.1:1883/".to_string();
        ctx.parse_url_into_self().expect("should parse");
        handler.connect(&ctx).expect("connect should succeed");

        let err = handler
            .send(&ctx, None, b"hello".to_vec())
            .expect_err("missing topic should fail");
        assert!(matches!(err, NetError::Network(_)));
    }

    #[test]
    fn exposes_itself_as_a_stream_handler() {
        let mut handler = MqttHandler::new();
        assert!(handler.as_stream().is_some());
    }
}
