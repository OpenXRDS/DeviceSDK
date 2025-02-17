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

use std::fmt::{self, Debug};
use std::clone::Clone;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Internal dependencies
use crate::common::enums::{PROTOCOLS, FtpCommands};
use crate::common::data_structure::{FtpPayload, FtpResponse, NetResponse};
use crate::common::parse_url;

// HTTP
use curl::easy::{Easy2, Handler, List, WriteError};

// CoAP
use coap_lite::CoapResponse;
use coap::UdpCoAPClient;

// Websocket
use websocket::client::sync::Client as WS_Client;
use websocket::stream::sync::NetworkStream;
use websocket::message::OwnedMessage;

// FTP & FTPS
use suppaftp::FtpStream;

// Mqtt
use rumqttc::Client as MqttClient;
use rumqttc::Connection as MqttConnection;
use rumqttc::{MqttOptions, QoS, Event, Incoming};

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
        use short_uuid::ShortUuid;
        let uuid = uuid::Uuid::new_v4();
        let short_str = ShortUuid::from_uuid(&uuid).to_string();

        Client {
            protocol: self.protocol,
            raw_url: "".to_string(),
            id: short_str,

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

            ws_client: None,
            ftp_stream: None,
            mqtt_client: None,
            mqtt_connection: None,
        }
    }
}

#[derive(Clone)]
 pub struct Client {
    protocol: PROTOCOLS,
    raw_url: String, // url given by the user. This is used for connection and request
    id: String, // unique id for the client

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

    ws_client: Option<Arc<Mutex<WS_Client<Box<dyn NetworkStream + Send>>>>>,
    ftp_stream: Option<Arc<Mutex<FtpStream>>>,
    mqtt_client: Option<MqttClient>,
    mqtt_connection: Option<Arc<Mutex<MqttConnection>>>,
 }

 impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client.Deubg: To Be Implemented")
    }
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

    pub fn get_mqtt_connection(&self) -> Option<Arc<Mutex<MqttConnection>>> {
        self.mqtt_connection.clone()
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
     * Since it is not possible to clarify the type of the client in advance,
     * the function returns Result<Self, String> instead of Result<T, String>
     */
    pub fn connect(mut self) -> Result<Self, String> {
        if self.raw_url.is_empty() {
            return Err("URL is required for connection.".to_string());
        }

        let parse_result = parse_url(&self.raw_url);
        if parse_result.is_err() {
            return Err(parse_result.err().unwrap());
        }

        self.host = Some(parse_result.as_ref().unwrap().host.clone());
        self.port = Some(parse_result.as_ref().unwrap().port);
        self.path = Some(parse_result.as_ref().unwrap().path.clone());
        if parse_result.as_ref().unwrap().query.is_some() {
            self.path = Some(self.path.as_ref().unwrap().to_string() + "?" + parse_result.as_ref().unwrap().query.as_ref().unwrap());
        }
        
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::WS | PROTOCOLS::WSS => self.connect_ws(),
            PROTOCOLS::FTP => self.connect_ftp(),
            PROTOCOLS::SFTP => self.connect_sftp(),
            PROTOCOLS::MQTT => self.connect_mqtt(),
            // PROTOCOLS::WEBRTC => self.connect_webrtc(),
            // PROTOCOLS::HTTP3 => self.connect_http3(),
            // PROTOCOLS::QUIC => self.connect_quic(),
            _ => Err("The protocol does not support 'Connect'. Use 'Request' instead.".to_string()),
        };

        return result;
    }

    fn connect_ftp(mut self) -> Result<Self, String> {
        let ftp_stream = FtpStream::connect(self.raw_url.as_str());
        if ftp_stream.is_err() {
            return Err(ftp_stream.err().unwrap().to_string());
        }

        let mut ftp_stream = ftp_stream.unwrap();

        if self.user.is_some() && self.password.is_some() {
            let user = self.user.as_ref().unwrap();
            let password = self.password.as_ref().unwrap();
            let login_result = ftp_stream.login(user, password);
            if login_result.is_err() {
                return Err(login_result.err().unwrap().to_string());
            } else {
                // store the ftp_stream in the client
                self.ftp_stream = Some(Arc::new(Mutex::new(ftp_stream)));
                return Ok(self);
            }
        } else {
            return Err("User and password are required for FTP connection.".to_string());
        }
    }

    /*
        Need to test first
     */
    fn connect_sftp(self) -> Result<Self, String> {
        return Ok(self);  // temporal return
    }



    /*************************** */
    /* MQTT PROTOCOLS */
    /*************************** */
    
    /**
     * Invokes 'publish' method of the mqtt client
     */
    fn send_mqtt(self, topic: Option<&str>, message: Vec<u8>) -> Result<Self, String> {
        if self.mqtt_client.is_none() {
            return Err("MQTT client is not initialized.".to_string());
        }

        if self.mqtt_connection.is_none() {
            return Err("MQTT connection is not initialized.".to_string());
        }

        let publish_result = self.mqtt_client.as_ref().unwrap()
            .publish(topic.unwrap(), QoS::AtLeastOnce, false, message);
        if publish_result.is_err() {
            return Err(publish_result.err().unwrap().to_string());
        } else {
            return Ok(self);
        }
    }

    /**
     * Receives the 'Publish' event only from the connection
     * if the received event is not 'Publish', it will return the empty Vec<u8>
     */
    fn rcv_mqtt(mut self) -> Result<Vec<u8>, String> {
        let mut mqtt_connection = self.mqtt_connection.as_mut().unwrap().lock().unwrap();
        let notification = mqtt_connection.recv();
        match notification {
            Ok(notification) => {
                let mut result_vec: Vec<u8> = Vec::new();
                if let Event::Incoming(Incoming::Publish(message)) = notification.unwrap() {
                    result_vec = Vec::from(message.payload);
                }
                return Ok(result_vec);
            },
            Err(err) => {
                let err_msg = format!("Error occurred while receiving the message: {:?}", err);
                return Err(err_msg);
            }
        }
    }

    pub fn mqtt_subscribe(self, topic: &str) -> Result<Self, String>{
        if self.mqtt_client.is_none() {
            return Err("MQTT client is not initialized.".to_string());
        }

        if self.mqtt_connection.is_none() {
            return Err("MQTT connection is not initialized.".to_string());
        }

        let subscription_result = 
            self.mqtt_client.as_ref().unwrap().subscribe(topic, QoS::AtMostOnce);
        if subscription_result.is_err() {
            return Err(subscription_result.err().unwrap().to_string());
        } else {
            return Ok(self);
        }
    }

    fn connect_mqtt(mut self) -> Result<Self, String> {
        let mut mqtt_options = MqttOptions::new(self.id.as_str(), 
                self.host.as_ref().unwrap(), self.port.unwrap().try_into().unwrap());
        mqtt_options.set_keep_alive(Duration::from_secs(5));

        let (client, connection) = MqttClient::new(mqtt_options, 10);
        self.mqtt_client = Some(client);
        self.mqtt_connection = Some(Arc::new(Mutex::new(connection)));

        return Ok(self);
    }

    /************************** */
    /* WEBSOCKET PROTOCOLS */
    /************************** */
    fn connect_ws(mut self) -> Result<Self, String> {
        let client_result = websocket::ClientBuilder::new(self.raw_url.as_str()).unwrap().connect(None);
        
        if client_result.is_err() {
            return Err(client_result.err().unwrap().to_string());
        } else {
            self.ws_client = Some(Arc::new(Mutex::new(client_result.unwrap())));
            return Ok(self);
        }
    }

    fn send_ws(mut self, message: Vec<u8>) -> Result<Self, String> {
        let send_result = self.ws_client.as_mut().unwrap()
            .lock().unwrap()
            .send_message(&OwnedMessage::Binary(message));

        if send_result.is_err() {
            return Err(send_result.err().unwrap().to_string());
        } else {
            return Ok(self);
        }
    }

    fn rcv_ws(mut self) -> Result<Vec<u8>, String> {
        let message = self.ws_client.as_mut().unwrap()
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

    pub fn send(self, data: Vec<u8>, topic: Option<&str>) -> Result<Self, String> {
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::WS | PROTOCOLS::WSS => self.send_ws(data),
            PROTOCOLS::MQTT => self.send_mqtt(topic, data),
            // PROTOCOLS::WEBRTC => self.send_webrtc(message),
            // PROTOCOLS::HTTP3 => self.send_http3(message),
            // PROTOCOLS::QUIC => self.send_quic(message),
            _ => Err("The protocol does not support 'Send'. Use another method instead.".to_string()),
        };

        return result;
    }

    pub fn rcv(self) -> Result<Vec<u8>, String> {
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::WS | PROTOCOLS::WSS => self.rcv_ws(),
            PROTOCOLS::MQTT => self.rcv_mqtt(),
            // PROTOCOLS::WEBRTC => self.rcv_webrtc(),
            // PROTOCOLS::HTTP3 => self.rcv_http3(),
            // PROTOCOLS::QUIC => self.rcv_quic(),
            _ => Err("The protocol does not support 'Receive'. Use another method instead".to_string()),
        };

        return result;
    }

    

    /**************************** */
    /* REQUEST-RESPONSE PROTOCOLS */
    /**************************** */
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
            headers,
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

    pub fn run_ftp_command(&self, ftp_payload: FtpPayload) -> FtpResponse {
        match ftp_payload.command {
            FtpCommands::CWD => self.clone().run_ftp_cwd(ftp_payload),
            FtpCommands::CDUP => self.clone().run_ftp_cdup(),
            FtpCommands::QUIT => self.clone().run_ftp_quit(),
            FtpCommands::RETR => self.clone().run_ftp_retr(ftp_payload),
            FtpCommands::STOR => self.clone().run_ftp_stor(ftp_payload),    
            FtpCommands::APPE => self.clone().run_ftp_appe(ftp_payload),    
            FtpCommands::DELE => self.clone().run_ftp_dele(ftp_payload),
            FtpCommands::RMD => self.clone().run_ftp_rmd(ftp_payload),
            FtpCommands::MKD => self.clone().run_ftp_mkd(ftp_payload),
            FtpCommands::PWD => self.clone().run_ftp_pwd(),                 
            FtpCommands::LIST => self.clone().run_ftp_list(ftp_payload),
            FtpCommands::NOOP => self.clone().run_ftp_noop(),
        }
        
    }

    /**
     * FTP commands
     */
    fn run_ftp_cwd(self, ftp_payload: FtpPayload) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().cwd(ftp_payload.payload_name.as_str());
        if response.is_err() {
            let ftp_res = FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
            
            return ftp_res;
        } else {
            let ftp_res = FtpResponse {
                payload: None,
                error: None,
            };
            return ftp_res;
        }
    }

    fn run_ftp_cdup(self) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().cdup();
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }

    fn run_ftp_quit(self) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().quit();
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }

    fn run_ftp_retr(self, ftp_payload: FtpPayload) -> FtpResponse {
        let data = self.ftp_stream.unwrap().lock().unwrap().retr_as_buffer(ftp_payload.payload_name.as_str());
        if data.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(data.err().unwrap().to_string()),
            };
        } else {
            let data = data.unwrap().into_inner();
            return FtpResponse {
                payload: Some(data),
                error: None,
            };
        }
    }

    fn run_ftp_stor(self, ftp_payload: FtpPayload ) -> FtpResponse {
        if ftp_payload.payload.is_none() {
            return FtpResponse {
                payload: None,
                error: Some("The payload is required for STOR command.".to_string()),
            };
        }

        let mut reader = Cursor::new(ftp_payload.payload.unwrap());
        let response = self.ftp_stream.unwrap().lock().unwrap().put_file(ftp_payload.payload_name, &mut reader);
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }

    fn run_ftp_appe(self, ftp_payload: FtpPayload) -> FtpResponse {
        let mut reader = Cursor::new(ftp_payload.payload.unwrap());
        let response = self.ftp_stream.unwrap().lock().unwrap().append_file(ftp_payload.payload_name.as_str(),  &mut reader);
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }

    fn run_ftp_dele(self, ftp_payload: FtpPayload) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().rm(ftp_payload.payload_name.as_str());
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }

    /**
     * Remove the directory
     * Only empty directory can be removed
     */
    fn run_ftp_rmd(self, ftp_payload: FtpPayload) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().rmdir(ftp_payload.payload_name.as_str());
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }

    fn run_ftp_mkd(self, ftp_payload: FtpPayload) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().mkdir(ftp_payload.payload_name.as_str());
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }

    fn run_ftp_pwd(self) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().pwd();
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            let payload = response.unwrap().as_bytes().to_vec();
            return FtpResponse {
                payload: Some(payload),
                error: None,
            };
        }
    }

    fn run_ftp_list(self, ftp_payload: FtpPayload) -> FtpResponse {
        if ftp_payload.payload_name.is_empty() {
            let list_result = self.ftp_stream.unwrap().lock().unwrap().list(None);
            if list_result.is_err() {
                return FtpResponse {
                    payload: None,
                    error: Some(list_result.err().unwrap().to_string()),
                };
            } else {
                return FtpResponse {
                    // convert Vec<String> to Vec<u8>
                    payload: Some(list_result.unwrap().join("\n").as_bytes().to_vec()),
                    error: None,
                };
            }
        } else {
            let list_result = self.ftp_stream.unwrap().lock().unwrap().list(Some(ftp_payload.payload_name.as_str()));
            if list_result.is_err() {
                return FtpResponse {
                    payload: None,
                    error: Some(list_result.err().unwrap().to_string()),
                };
            } else {
                return FtpResponse {
                    // convert Vec<String> to Vec<u8>
                    payload: Some(list_result.unwrap().join("\n").as_bytes().to_vec()),
                    error: None,
                };
            }
        }
    }

    fn run_ftp_noop(self) -> FtpResponse {
        let response = self.ftp_stream.unwrap().lock().unwrap().noop();
        if response.is_err() {
            return FtpResponse {
                payload: None,
                error: Some(response.err().unwrap().to_string()),
            };
        } else {
            return FtpResponse {
                payload: None,
                error: None,
            };
        }
    }
    // ***************** End of Ftp Command Functons ***************//
 }