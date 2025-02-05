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

        // 2. request to the server using HTTPS

        return NetResponse {
            protocol: self.protocol,
            status_code: 0,
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

 /**
  * Unit tests

  * To run the tests, execute the following command: cargo test
  * In case of printing the output of the tests, execute the following command: cargo test -- --nocapture
 */
 #[cfg(test)]
 mod tests {
    use crate::client::{Client, ClientBuilder};
    use crate::common::data_structure::NetResponse;
    use crate::common::enums::PROTOCOLS;

    static HTTP_ECHO_SERVER_URL: &str = "https://echo.free.beeceptor.com";

    #[test]
    fn test_build_client() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        /* Assertions */
        assert_eq!(client.get_protocol(), &PROTOCOLS::HTTP);
    }

    /* start of HTTP 1.1 tests */
    #[test]
    fn test_http_request_wrong_host_name1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("ww.w.clear.com").request();
        
        /* Assertions */
        assert!(response.error.is_some());  // wrong host name
    }

    #[test]
    fn test_http_request_wrong_host_name2() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("3.112.22.222.11").request();
        
        /* Assertions */
        assert!(response.error.is_some());  // wrong host name
    }

    /*
        Disabled due to hardness of testing the IP address
     */
    // #[test]
    // fn test_http_request_host_ip() {
    //     let client_builder = ClientBuilder::new();
    //     let client = client_builder.set_protocol(PROTOCOLS::HTTP)
    //         .build();

    //     let response = client.set_url("192.168.10.240")
    //                             .set_follow_redirect(true).request();
    //     println!("error: {}", response.error.as_ref().unwrap());
    //     assert!(response.error.is_none());  // successful request

    //     assert!(response.headers.len() > 0);
    //     assert!(response.body.len() > 0);
    //     assert_eq!(response.status_code, 200);
    // }

    #[test]
    fn test_http_request_get_with_redirection() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_follow_redirect(true)
            .set_url("http://www.rust-lang.org:80/")
            .request();
        
        /* Assertions */
        assert!(response.error.is_none());  // successful request
        assert!(response.headers.len() > 0);
        assert!(response.body.len() > 0);
        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn test_http_request_get_without_redirection() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("http://www.rust-lang.org:80/").request();
        
        /* Assertions */
        assert!(response.error.is_none());  // successful request
        assert!(response.headers.len() > 0);
        assert!(response.body.len() > 0);
        assert_ne!(response.status_code, 200);  // redirection status code
    }

    #[test]
    fn test_http_request_post_no_post_allowed() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("http://www.rust-lang.org:80/")
                        .set_method("POST")
                        .set_follow_redirect(true)
                        .request();
        
        /* Assertions */
        assert!(response.error.is_none());  // successful request
        assert!(response.headers.len() > 0);
        assert!(response.body.len() > 0);

        // println!("return code: {}", response.status_code);
        assert_ne!(response.status_code, 200);  // no post allowed for the server
    }

    #[test]
    fn test_http_request_post_headers_only() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url(HTTP_ECHO_SERVER_URL)
                .set_req_headers(vec![("Content-Type", "application/json")])
                .set_method("POST")
                .request();
        
        /* Assertions */
        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn test_http_request_post_1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url(HTTP_ECHO_SERVER_URL)
                .set_req_headers(vec![("Content-Type", "application/json"), ("Authorization", "Bearer 123456")])
                .set_method("POST")
                .set_req_body("{}")
                .request();
        
        /* Assertions */
        assert_eq!(response.status_code, 200);
    }
    /* end of HTTP 1.1 tests */

    /* start of HTTPS tests */
    #[test]
    fn test_https_request_get() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTPS)
            .build();

        let response = client.set_url("https://www.rust-lang.org:443/")
            .request();

        /* Assertions */
        assert!(response.error.is_none());  // successful request
        assert_ne!(response.status_code, 200);  // returns 403
    }

 }