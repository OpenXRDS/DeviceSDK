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

//! `Client`/`ClientBuilder`: the expert/session API, now built on the
//! `ProtocolHandler` mechanism (Phase 2 of
//! `docs/done/xrds-net-protocol-handler.md`). Public shape is unchanged except
//! `Client` is no longer `Clone` (see "Drop `Client: Clone`" in the plan
//! doc) — every method that used to match over `self.protocol` and call a
//! private per-protocol method now delegates through a capability query on
//! `self.handler`.

use std::fmt;
// Only the MQTT connection accessor needs these.
#[cfg(feature = "protocol-mqtt")]
use std::sync::{Arc, Mutex};
use std::vec;

#[cfg(feature = "protocol-mqtt")]
use rumqttc::Connection as MqttConnection;

use crate::common::data_structure::NetResponse;
// Only `run_ftp_command` uses these.
#[cfg(feature = "protocol-ftp")]
use crate::common::data_structure::{FtpPayload, FtpResponse};
use crate::common::enums::PROTOCOLS;
use crate::common::{generate_random_string, parse_url};

use super::context::ClientContext;
use super::error::NetError;
use super::handler::{create_handler, ProtocolHandler};
#[cfg(feature = "protocol-ftp")]
use super::protocols::ftp::FtpHandler;
#[cfg(feature = "protocol-mqtt")]
use super::protocols::mqtt::MqttHandler;
use super::scheme::scheme_to_protocol;

pub struct ClientBuilder {
    protocol: PROTOCOLS,

    // authentication
    user: Option<String>,
    password: Option<String>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    pub fn new() -> Self {
        ClientBuilder {
            protocol: PROTOCOLS::HTTP,
            user: None,
            password: None,
        }
    }

    pub fn set_protocol(mut self, protocol: PROTOCOLS) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn set_user(mut self, user: &str) -> Self {
        self.user = Some(user.to_string());
        self
    }

    pub fn set_password(mut self, password: &str) -> Self {
        self.password = Some(password.to_string());
        self
    }

    /**
     * build the client with the given parameters
     * This function will parse the url to fill host, port, and path
     */
    pub fn build(self) -> Client {
        let mut ctx = ClientContext::new(self.protocol, generate_random_string(20));
        ctx.user = self.user;
        ctx.password = self.password;

        Client {
            handler: create_handler(self.protocol),
            ctx,
        }
    }

    /// Infers the protocol from the URL scheme and builds a `Client` with
    /// `raw_url` already parsed. The mechanism `XrdsNet`'s intent verbs are
    /// built on (see "`ClientBuilder::from_url` and scheme inference" in the
    /// plan doc); also usable directly by expert-API callers who'd rather
    /// not call `set_protocol` themselves.
    pub fn from_url(url: &str) -> Result<Client, NetError> {
        let parsed = parse_url(url).map_err(NetError::Network)?;
        let protocol = scheme_to_protocol(&parsed.scheme)?;

        let mut client = Self::new().set_protocol(protocol).build();
        client.ctx.raw_url = url.to_string();
        client.ctx.parse_url_into_self()?;
        Ok(client)
    }
}

pub struct Client {
    ctx: ClientContext,
    handler: Box<dyn ProtocolHandler>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Client {{ protocol: {:?}, id: {} }}",
            self.ctx.protocol, self.ctx.id
        )
    }
}

impl Client {
    pub fn set_method(mut self, method: &str) -> Self {
        self.ctx.method = Some(method.to_uppercase());
        self
    }

    pub fn set_url(mut self, url: &str) -> Self {
        self.ctx.raw_url = url.to_string();
        self
    }

    pub fn set_follow_redirect(mut self, follow: bool) -> Self {
        self.ctx.redirection = follow;
        self
    }

    pub fn set_req_headers(mut self, param_headers: Vec<(&str, &str)>) -> Self {
        // convert (&str, &str) to (String, String)
        let mut headers: Vec<(String, String)> = vec![];
        for (key, value) in param_headers.iter() {
            headers.push((key.to_string(), value.to_string()));
        }
        self.ctx.req_headers = Some(headers);
        self
    }

    pub fn set_req_body(mut self, body: &str) -> Self {
        self.ctx.req_body = Some(body.to_string());
        self
    }

    pub fn set_timeout(mut self, timeout: u64) -> Self {
        self.ctx.timeout = Some(timeout);
        self
    }

    pub fn set_user(mut self, user: &str) -> Self {
        self.ctx.user = Some(user.to_string());
        self
    }

    pub fn set_password(mut self, password: &str) -> Self {
        self.ctx.password = Some(password.to_string());
        self
    }

    /// Expert-only extra, unchanged in shape — see "Expert-only extras" in
    /// the plan doc. `None` if this `Client` isn't MQTT-protocol.
#[cfg(feature = "protocol-mqtt")]
    pub fn get_mqtt_connection(&self) -> Option<Arc<Mutex<MqttConnection>>> {
        self.handler
            .as_any()
            .downcast_ref::<MqttHandler>()
            .and_then(|mqtt| mqtt.get_connection())
    }

    pub fn get_protocol(&self) -> PROTOCOLS {
        self.ctx.protocol
    }

    pub fn get_id(&self) -> String {
        self.ctx.id.clone()
    }

    /// Consumes the `Client`, handing over its `ClientContext` and handler.
    /// Used internally by `XrdsNet` (`net_intent.rs`), which needs to move
    /// both into a background thread for `listen()` or otherwise operate on
    /// them directly rather than through `Client`'s builder-chain shape.
    pub(crate) fn into_parts(self) -> (ClientContext, Box<dyn ProtocolHandler>) {
        (self.ctx, self.handler)
    }

    /******************************************** */
    /*************     CONNECTION       ********* */
    /******************************************** */
    /**
     * connect to the server
     * Since it is not possible to clarify the type of the client in advance,
     * the function returns Result<Self, NetError> instead of Result<T, NetError>
     */
    pub fn connect(mut self) -> Result<Self, NetError> {
        if self.ctx.raw_url.is_empty() {
            return Err(NetError::Network("URL is required for connection.".to_string()));
        }

        self.ctx.parse_url_into_self()?;
        self.handler.validate(&self.ctx)?;

        if let Some(stream) = self.handler.as_stream() {
            stream.connect(&self.ctx)?;
            return Ok(self);
        }
        if let Some(ft) = self.handler.as_file_transfer() {
            ft.connect(&self.ctx)?;
            return Ok(self);
        }

        Err(NetError::capability(
            self.ctx.protocol,
            "connect",
            "The protocol does not support 'Connect'. Use 'Request' instead.",
        ))
    }

    /******************************************** */
    /*************         SEND         ********* */
    /******************************************** */
    /**
     * topic is required for MQTT
     * topic is optional for WS, and indicates the message type (binary, text, etc.)
     * if no message type is given for WS, it will be considered as binary
     */
    pub fn send(mut self, data: Vec<u8>, topic: Option<&str>) -> Result<Self, NetError> {
        match self.handler.as_stream() {
            Some(stream) => {
                stream.send(&self.ctx, topic, data)?;
                Ok(self)
            }
            None => Err(NetError::capability(
                self.ctx.protocol,
                "send",
                "The protocol does not support 'Send'. Use another method instead.",
            )),
        }
    }

    /******************************************** */
    /*************       RECEIVE        ********* */
    /******************************************** */
    pub fn rcv(&mut self) -> Result<Vec<u8>, NetError> {
        match self.handler.as_stream() {
            Some(stream) => stream.recv(&self.ctx).map(|event| event.payload),
            None => Err(NetError::capability(
                self.ctx.protocol,
                "rcv",
                "The protocol does not support 'Rcv'. Use another method instead.",
            )),
        }
    }

    pub fn close(&mut self) -> Result<(), NetError> {
        match self.handler.as_stream() {
            Some(stream) => stream.close(&self.ctx),
            None => Err(NetError::capability(
                self.ctx.protocol,
                "close",
                "The protocol does not support 'Close'. Use another method instead",
            )),
        }
    }

    /*************************** */
    /* MQTT PROTOCOLS */
    /*************************** */
    /// Expert-only extra, unchanged in shape — see "Expert-only extras" in
    /// the plan doc. Errs if this `Client` isn't MQTT-protocol.
    #[cfg(feature = "protocol-mqtt")]
    pub fn mqtt_subscribe(mut self, topic: &str) -> Result<Self, NetError> {
        match self.handler.as_any_mut().downcast_mut::<MqttHandler>() {
            Some(mqtt) => {
                mqtt.subscribe(topic)?;
                Ok(self)
            }
            None => Err(NetError::capability(
                self.ctx.protocol,
                "mqtt_subscribe",
                "Client is not using the MQTT protocol.",
            )),
        }
    }

    /**************************** */
    /* REQUEST-RESPONSE PROTOCOLS */
    /**************************** */
    /**
     * request to the server
     */
    pub fn request(self) -> Result<NetResponse, NetError> {
        let Client { mut ctx, handler } = self;
        ctx.parse_url_into_self()?;
        handler.validate(&ctx)?;
        handler.request(&ctx)
    }

    /*************************** */
    /* FTP & FTPS PROTOCOLS */
    /*************************** */
    /// Expert-only extra, unchanged in shape — see "Expert-only extras" in
    /// the plan doc. Reports an `FtpResponse` error if this `Client` isn't
    /// FTP-protocol (matching the original's all-in-`FtpResponse` error
    /// convention rather than a `Result`).
    #[cfg(feature = "protocol-ftp")]
    pub fn run_ftp_command(&self, ftp_payload: FtpPayload) -> FtpResponse {
        match self.handler.as_any().downcast_ref::<FtpHandler>() {
            Some(ftp) => ftp.run_command(ftp_payload),
            None => FtpResponse {
                payload: None,
                error: Some("Client is not using the FTP protocol.".to_string()),
            },
        }
    }
}
