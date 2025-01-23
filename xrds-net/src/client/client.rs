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
use crate::common::data_structure::NetResponse;

use url::Url;
use url::ParseError;

pub struct ClientBuilder {
    protocol: PROTOCOLS,
    url: String,
    port: u16,
    path: String,
    
    // Optional fields
    headers: Option<Vec<(String, String)>>,
    body: Option<String>,
    timeout: Option<u64>,

    // authentication
    user: Option<String>,
    password: Option<String>,

}

impl ClientBuilder {
    pub fn new(p_proto: PROTOCOLS, url: String) -> Result<Self, ParseError> {
        // parse the url
        let parsed_url = Url::parse(&url)?;
        let host = parsed_url.host_str().unwrap();
        let port = parsed_url.port().unwrap_or(80);
        let path = parsed_url.path();

        Ok(ClientBuilder::init(p_proto, host, port, path))
    }

    fn init(protocol: PROTOCOLS, host: &str, port: u16, path: &str) -> Self {
        ClientBuilder {
            protocol,
            url: host.to_string(),
            port,
            path: path.to_string(),

            headers: None,
            body: None,
            timeout: None,

            user: None,
            password: None,
        }
    }

    pub fn set_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn set_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    pub fn set_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn set_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }

    pub fn set_password(mut self, password: String) -> Self {
        self.password = Some(password);
        self
    }

    pub fn build(self) -> Client {
        Client {
            protocol: self.protocol,
            url: self.url,
            port: self.port,
            path: self.path,

            headers: self.headers,
            body: self.body,
            timeout: self.timeout,

            user: self.user,
            password: self.password,

            response_body: "".to_string(),
            
        }
    }
}

 pub struct Client {
    protocol: PROTOCOLS,
    url: String,
    port: u16,
    path: String,

    headers: Option<Vec<(String, String)>>,
    body: Option<String>,
    timeout: Option<u64>,

    user: Option<String>,
    password: Option<String>,

    response_body: String,
 }

 impl Client {
    pub fn new (protocol: PROTOCOLS, url: String, port: u16, path: String) -> ClientBuilder {
        ClientBuilder {
            protocol,
            url,
            port,
            path,

            headers: None,
            body: None,
            timeout: None,

            user: None,
            password: None,
        }
    }



    pub fn get_protocol(&self) -> PROTOCOLS {
        self.protocol
    }

    pub fn get_url(&self) -> &str {
        &self.url
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn get_headers(&self) -> Option<&Vec<(String, String)>> {
        self.headers.as_ref()
    }

    pub fn get_body(&self) -> Option<&String> {
        self.body.as_ref()
    }

    pub fn get_timeout(&self) -> Option<u64> {
        self.timeout
    }

    pub fn get_user(&self) -> Option<&String> {
        self.user.as_ref()
    }

    pub fn get_password(&self) -> Option<&String> {
        self.password.as_ref()
    }

    pub fn get_response_body(&self) -> String {
        self.response_body.clone()
    }

    /**
     * connect to the server
     */
    pub fn connect(&self) -> Result<(), String> {
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::MQTT => self.connect_mqtt(),
            PROTOCOLS::FTP => self.connect_ftp(),
            PROTOCOLS::SFTP => self.connect_sftp(),
            PROTOCOLS::WS => self.connect_ws(),
            PROTOCOLS::WSS => self.connect_wss(),
            PROTOCOLS::WEBRTC => self.connect_webrtc(),
            PROTOCOLS::HTTP3 => self.connect_http3(),
            PROTOCOLS::QUIC => self.connect_quic(),
            _ => Err("The protocol does not support 'Connect'. Use 'Request' instead.".to_string()),
        };

        return result;
    }

    fn connect_mqtt(&self) -> Result<(), String> {
        // connect to the server using MQTT

        return Ok(());  // temporal return
    }

    fn connect_ftp(&self) -> Result<(), String> {
        // connect to the server using FTP

        return Ok(());  // temporal return
    }

    fn connect_sftp(&self) -> Result<(), String> {
        // connect to the server using SFTP

        return Ok(());  // temporal return
    }

    fn connect_ws(&self) -> Result<(), String> {
        // connect to the server using WS

        return Ok(());  // temporal return
    }

    fn connect_wss(&self) -> Result<(), String> {
        // connect to the server using WSS

        return Ok(());  // temporal return
    }

    fn connect_webrtc(&self) -> Result<(), String> {
        return Err("The protocol is not supported yet.".to_string());
    }

    fn connect_http3(&self) -> Result<(), String> {
        return Err("The protocol is not supported yet.".to_string());
    }

    fn connect_quic(&self) -> Result<(), String> {
        // connect to the server using QUIC
        return Err("The protocol is not supported yet.".to_string());
    }

    /**
     * request to the server
     */
    pub fn request(&self) -> NetResponse {
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::HTTP => self.request_http(),
            PROTOCOLS::HTTPS => self.request_https(),
            PROTOCOLS::FILE => self.request_file(),
            PROTOCOLS::COAP => self.request_coap(),
            // PROTOCOLS::COAPS => self.request_coaps(),
            _ => Err("The protocol does not support 'Request'. Use 'Connect' instead.".to_string()),
        };

        return result;
    }

    fn request_http(&self) -> NetResponse {
        // 1. validate url
        let result = self.validate_url();
        if result.is_err() {
            return NetResponse {
                protocol: self.protocol,
                status_code: 0,
                headers: vec![],
                body: "".to_string(),
                error: Some(result.err().unwrap()),
            };
        }

        // 2. request to the server using HTTP


        return NetResponse {
            protocol: self.protocol,
            status_code: 200,
            headers: vec![],
            body: "".to_string(),
            error: None,
        };  // temporal return
    }

    fn request_https(&self) -> NetResponse {
        // 1. validate url
        let result = self.validate_url();
        if result.is_err() {
            return NetResponse {
                protocol: self.protocol,
                status_code: 0,
                headers: vec![],
                body: "".to_string(),
                error: Some(result.err().unwrap()),
            };
        }

        // 2. request to the server using HTTPS

        return NetResponse {
            protocol: self.protocol,
            status_code: 200,
            headers: vec![],
            body: "".to_string(),
            error: None,
        };  // temporal return
    }

    fn request_file(&self) -> Result<(), String> {
        // request to the server using FILE

        return Ok(());  // temporal return
    }

    fn request_coap(&self) -> Result<(), String> {
        // request to the server using COAP

        return Ok(());  // temporal return
    }

    fn validate_url(&self) -> Result<(), String> {
        // do scheme test based on url string
        let tokenized_url = self.url.split("://").collect::<Vec<&str>>();
        if tokenized_url.len() != 2 {
            return Err("Missing scheme".to_string());
        }
        
        let parsed_url = Url::parse(&self.url);
        if parsed_url.is_err() {
            return Err(parsed_url.err().unwrap().to_string());  // return the error message to force scheme in the url
        } else {
            return Ok(());
        }
    }
 }