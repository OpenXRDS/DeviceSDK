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

use curl::{easy::{Easy2, Handler, List, WriteError}, Error};

use coap_lite::{RequestType as Method, CoapRequest, CoapResponse};
use coap::{UdpCoAPClient};

/**
 * ResponseCollector is a struct to collect response from the server
 * 0: headers
 * 1: body
 */
struct ResponseCollector(Vec<u8>, Vec<u8>);

impl Handler for ResponseCollector {
    fn header(&mut self, data: &[u8]) -> bool {
        self.0.extend_from_slice(data);
        true
    }
    
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.1.extend_from_slice(data);
        Ok(data.len())
    }
}

pub struct ClientBuilder {
    protocol: PROTOCOLS,

    // authentication
    user: Option<String>,
    password: Option<String>,
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

    pub fn set_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }

    pub fn set_password(mut self, password: String) -> Self {
        self.password = Some(password);
        self
    }

    /**
     * build the client with the given parameters
     * This function will parse the url to fill host, port, and path
     */
    pub fn build(self) -> Client {
        // try parse to fill host, port, and path

        Client {
            protocol: self.protocol,
            raw_url: "".to_string(),

            host: None,
            port: None,
            path: None,

            method: None,
            req_headers: None,
            req_body: None,
            timeout: None,
            redirection: false,

            user: self.user,
            password: self.password,

            res_headers: vec![],
            res_body: "".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
 pub struct Client {
    protocol: PROTOCOLS,
    raw_url: String, // url given by the user. This is used for connection and request
    
    // parsed url. these fields are extracted from the url string
    // Not directly used for connection or request. Just for information
    host: Option<String>,
    port: Option<u32>,
    path: Option<String>,

    req_headers: Option<Vec<(String, String)>>,
    req_body: Option<String>,
    timeout: Option<u64>,
    redirection: bool,
    method: Option<String>,

    user: Option<String>,
    password: Option<String>,

    res_headers: Vec<(String, String)>,
    res_body: String,
 }

 impl Client {

    pub fn get_protocol(&self) -> &PROTOCOLS {
        &self.protocol
    }

    pub fn get_url(&self) -> &String {
        &self.raw_url
    }

    pub fn get_host(&self) -> Option<&String> {
        self.host.as_ref()
    }

    pub fn get_port(&self) -> Option<u32> {
        self.port
    }

    pub fn get_path(&self) -> Option<&String> {
        self.path.as_ref()
    }

    pub fn get_req_headers(&self) -> Option<&Vec<(String, String)>> {
        self.req_headers.as_ref()
    }

    pub fn get_req_body(&self) -> Option<&String> {
        self.req_body.as_ref()
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

    pub fn get_response_headers(&self) -> Vec<(String, String)> {
        self.res_headers.clone()
    }

    pub fn get_response_body(&self) -> String {
        self.res_body.clone()
    }

    pub fn get_method(&self) -> Option<&String> {
        self.method.as_ref()
    }

    pub fn set_method(mut self, method: &str) -> Self {
        self.method = Some(method.to_uppercase().to_string());
        self
    }

    pub fn set_url(mut self, url: &str) -> Self {
        self.raw_url = url.to_string();
        self
    }

    pub fn set_follow_redirect(mut self, follow: bool) -> Self {
        self.redirection = follow;
        self
    }

    pub fn set_req_headers(mut self, param_headers: Vec<(&str, &str)>) -> Self {
        // convert (&str, &str) to (String, String)
        let mut headers: Vec<(String, String)> = vec![];
        for (key, value) in param_headers.iter() {
            headers.push((key.to_string(), value.to_string()));
        }
        self.req_headers = Some(headers);
        self
    }

    pub fn set_req_body(mut self, body: &str) -> Self {
        self.req_body = Some(body.to_string());
        self
    }

    pub fn set_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    fn parse_headers(&self, headers: &str) -> Vec<(String, String)> {
        let headers = headers.split("\r\n").collect::<Vec<&str>>();
        let mut parsed_headers: Vec<(String, String)> = vec![];
        for header in headers {
            let header = header.split(":").collect::<Vec<&str>>();
            if header.len() == 2 {
                parsed_headers.push((header[0].to_string(), header[1].to_string()));
            }
        }

        return parsed_headers;
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
    pub fn request(mut self) -> NetResponse {
        let parsed_url = crate::common::parse_url(&self.raw_url);
        if parsed_url.is_err() {
            let err_message = parsed_url.err().unwrap();
            return NetResponse {
                protocol: self.protocol,
                status_code: 0,
                headers: vec![],
                body: Vec::new(),
                error: Some(err_message),
            }
        } else {
            let parsed_url = parsed_url.unwrap();
            self.host = Some(parsed_url.host);
            self.port = Some(parsed_url.port);
            self.path = Some(parsed_url.path);
            if parsed_url.query.is_some() {
                // add query to the path
                self.path = Some(self.path.as_ref().unwrap().to_string() + "?" + parsed_url.query.as_ref().unwrap());
            }
        }

        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::HTTP => self.request_http(),
            PROTOCOLS::HTTPS => self.request_http(),
            PROTOCOLS::FILE => self.request_file(),
            PROTOCOLS::COAP => self.request_coap(),
            _ => NetResponse {
                protocol: self.protocol,
                status_code: 0,
                headers: vec![],
                body: Vec::new(),
                error: Some("The protocol does not support 'Request'. Use 'Connect' instead.".to_string()),
            },
        };

        return result;
    }

    /**
     * Currently GET and POST methods are supported
     */
    fn request_http(&self) -> NetResponse {
        // 1. request to the server using HTTP
        let mut easy = Easy2::new(ResponseCollector(Vec::new(), Vec::new()));
        easy.get(true).unwrap(); // GET method is default
        easy.url(self.raw_url.as_str()).unwrap();
        easy.follow_location(self.redirection).unwrap();

        // Check if the request has headers
        if self.req_headers.is_some() {
            let mut list = List::new();
            let headers = self.req_headers.as_ref().unwrap();
            for (key, value) in headers.iter() {
                let item = format!("{}: {}", key, value);
                list.append(item.as_str()).unwrap();
            }

            // add headers to the request
            easy.http_headers(list).unwrap();
        }

        if self.get_method() == Some(&"POST".to_string()) {
            easy.post(true).unwrap();    // POST method
            
            if self.req_body.is_some() {    // fill body field if there is any in request
                easy.post_fields_copy(self.req_body.as_ref().unwrap().as_bytes()).unwrap();
            }
        }

        let perform_result = easy.perform();
        if perform_result.is_err() {
            return NetResponse {
                protocol: self.protocol,
                status_code: 0,
                headers: vec![],
                body: Vec::new(),
                error: Some(perform_result.err().unwrap().to_string()),
            };
        }

        let response_code = easy.response_code().unwrap();
        let response_headers = easy.get_ref().0.clone();
        let response_body = easy.get_ref().1.clone();

        // tokenized headers from single string to Vec<(String, String)>
        let header_str = String::from_utf8(response_headers).unwrap();
        let tokenized_headers = self.parse_headers(&header_str);

        return NetResponse {
            protocol: self.protocol,
            status_code: response_code,
            headers: tokenized_headers,
            body: response_body,
            error: None,
        };
    }

    /**
     * request to the server using FILE
     * returns the file byte stream in NetResponse.body
     * This request uses 80 port by default
     */
    fn request_file(&self) -> NetResponse {
        let mut easy = Easy2::new(ResponseCollector(Vec::new(), Vec::new()));
        // println!("url: {}", self.raw_url);
        easy.url(self.raw_url.as_str()).unwrap();

        let perform_result = easy.perform();
        if perform_result.is_err() {
            return NetResponse {
                protocol: self.protocol,
                status_code: 0,
                headers: vec![],
                body: Vec::new(),
                error: Some(perform_result.err().unwrap().to_string()),
            };
        }

        let response_code = easy.response_code().unwrap();
        let response_headers = easy.get_ref().0.clone();
        let response_body = easy.get_ref().1.clone();

        // tokenized headers from single string to Vec<(String, String)>
        let header_str = String::from_utf8(response_headers).unwrap();
        let tokenized_headers = self.parse_headers(&header_str);

        return NetResponse {
            protocol: self.protocol,
            status_code: response_code,
            headers: tokenized_headers,
            body: response_body,
            error: None,
        };
    }    

    fn request_coap(&self) -> NetResponse {
        // request to the server using COAP
        let rt = tokio::runtime::Runtime::new().unwrap();
        let response = rt.block_on(self.run_coap());

        if response.is_err() {
            return NetResponse {
                protocol: self.protocol,
                status_code: 0,
                headers: vec![],
                body: Vec::new(),
                error: Some(response.err().unwrap()),
            };
        }
        let response = response.unwrap();
        let coap_res_header = response.message.header.clone();
        let coap_res_payload = response.message.payload.clone();

        let status_code_str = coap_res_header.code.to_string();
        let coap_status_code = crate::common::coap_code_to_decimal(&status_code_str);

        let mut headers: Vec<(String, String)> = vec![];
        headers.push(("Code".to_string(), coap_res_header.code.to_string()));
        headers.push(("Message ID".to_string(), coap_res_header.message_id.to_string()));
        headers.push(("Version".to_string(), coap_res_header.get_version().to_string()));

        let body = String::from_utf8(coap_res_payload).unwrap();

        return NetResponse {
            protocol: self.protocol,
            status_code: coap_status_code,
            headers: headers,
            body: body.as_bytes().to_vec(),
            error: None,
        };
    }

    async fn run_coap(&self) -> Result<CoapResponse, String> {

        let response = UdpCoAPClient::get(&self.raw_url).await;
        
        if response.is_err() {
            return Err(response.err().unwrap().to_string());
        } else {
            return Ok(response.unwrap());
        }
    }
 }