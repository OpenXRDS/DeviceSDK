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
use crate::common::validate_url;

use url::Url;
use url::ParseError;

use curl::easy::{Easy2, Handler, WriteError, List};

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
    url: String,
    port: u16,
    path: String,
    
    // Optional fields
    req_headers: Option<Vec<(String, String)>>,
    req_body: Option<String>,
    timeout: Option<u64>,

    // authentication
    user: Option<String>,
    password: Option<String>,

}

impl ClientBuilder {
    pub fn new() -> Self {
        ClientBuilder {
            protocol: PROTOCOLS::HTTP,
            url: "".to_string(),
            port: 80,
            path: "".to_string(),

            req_headers: None,
            req_body: None,
            timeout: None,

            user: None,
            password: None,
        }
    }

    pub fn set_protocol(mut self, protocol: PROTOCOLS) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn set_url(mut self, url: String) -> Self {
        self.url = url;
        self
    }

    pub fn set_req_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.req_headers = Some(headers);
        self
    }

    pub fn set_req_body(mut self, body: String) -> Self {
        self.req_body = Some(body);
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

    pub fn build(self) -> Result<Client, String> {
        let url_validation_result = validate_url(self.url.as_str());
        if url_validation_result.is_err() {
            return Err(url_validation_result.err().unwrap());
        }

        let url = url_validation_result.unwrap();
        let val_port = url.port().unwrap_or(80);
        let val_host = url.host_str().unwrap_or("").to_string();
        let val_path = url.path().to_string();

        Ok(Client {
            protocol: self.protocol,
            host: val_host,
            port: val_port,
            path: val_path,

            req_headers: self.req_headers,
            req_body: self.req_body,
            timeout: self.timeout,

            user: self.user,
            password: self.password,

            res_headers: vec![],
            res_body: "".to_string(),
        })
    }
}

 pub struct Client {
    protocol: PROTOCOLS,
    host: String,
    port: u16,
    path: String,

    req_headers: Option<Vec<(String, String)>>,
    req_body: Option<String>,
    timeout: Option<u64>,

    user: Option<String>,
    password: Option<String>,

    res_headers: Vec<(String, String)>,
    res_body: String,
 }

 impl Client {

    pub fn get_protocol(&self) -> PROTOCOLS {
        self.protocol
    }

    pub fn get_host(&self) -> &str {
        &self.host
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn get_path(&self) -> &str {
        &self.path
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
        // 1. validation of the url is done in the builder

        // 2. request to the server using HTTP
        let mut easy = Easy2::new(ResponseCollector(Vec::new(), Vec::new()));
        easy.get(true).unwrap(); // GET method is default
        
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

        if self.req_body.is_some() {
            easy.post(true).unwrap();    // POST method
            easy.post_fields_copy(self.req_body.as_ref().unwrap().as_bytes()).unwrap();
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
        let tokenized_headers = String::from_utf8(response_headers).unwrap();
        let tokenized_headers = tokenized_headers.split("\r\n").collect::<Vec<&str>>();
        let mut response_headers: Vec<(String, String)> = vec![];
        for header in tokenized_headers {
            let header = header.split(":").collect::<Vec<&str>>();
            if header.len() == 2 {
                response_headers.push((header[0].to_string(), header[1].to_string()));
            }
        }

        return NetResponse {
            protocol: self.protocol,
            status_code: response_code,
            headers: response_headers,
            body: response_body,
            error: None,
        };
    }

    fn request_https(&self) -> NetResponse {
        // 1. validate url is done in the builder

        // 2. request to the server using HTTPS

        return NetResponse {
            protocol: self.protocol,
            status_code: 200,
            headers: vec![],
            body: Vec::new(),
            error: None,
        };  // temporal return
    }

    /**
     * request to the server using FILE
     * returns the file byte stream in NetResponse.body
     */
    fn request_file(&self) -> NetResponse {
        // request to the server using FILE

        return NetResponse {
            protocol: self.protocol,
            status_code: 200,
            headers: vec![],
            body: Vec::new(),
            error: None,
        };  // temporal return
    }

    fn request_coap(&self) -> NetResponse {
        // request to the server using COAP

        return NetResponse {
            protocol: self.protocol,
            status_code: 200,
            headers: vec![],
            body: Vec::new(),
            error: None,
        }
    }


 }

 #[cfg(test)]
 mod tests {
    use crate::client::{Client, ClientBuilder};
    use crate::common::data_structure::NetResponse;
    use crate::common::enums::PROTOCOLS;

    #[test]
    fn test_build_client() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .set_url("https://www.rust-lang.org:80/".to_string())
            .build().unwrap();

        assert_eq!(client.protocol, PROTOCOLS::HTTP);
        assert_eq!(client.host, "www.rust-lang.org");
        assert_eq!(client.port, 80);

    }
 }