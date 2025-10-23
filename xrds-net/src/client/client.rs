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

use std::{fmt};
use std::clone::Clone;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::{thread, vec};
use std::time::Duration;
use tokio::time::Instant;

use mio::net::UdpSocket;
use mio::{Events, Poll};

// Internal dependencies
use crate::common::enums::{PROTOCOLS, FtpCommands};
use crate::common::data_structure::{FtpPayload, FtpResponse, NetResponse, XrUrl};
use crate::common::{parse_url, fill_mandatory_http_headers, generate_random_string};

// HTTP
use curl::easy::{Easy2, Handler, List, WriteError};

// CoAP
use coap_lite::CoapResponse;
use coap::UdpCoAPClient;

// Websocket
use crate::client::xrds_websocket::XrdsWebsocket;

// FTP & FTPS
use suppaftp::FtpStream;

// Mqtt
use rumqttc::Client as MqttClient;
use rumqttc::Connection as MqttConnection;
use rumqttc::{MqttOptions, QoS, Event, Incoming};

// QUIC / HTTP3
use quiche::{Connection, RecvInfo};
use quiche::h3::NameValue;


const MAX_DATAGRAM_SIZE: usize = 1350;


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
        let short_str = generate_random_string(20);

        Client {
            protocol: self.protocol,
            raw_url: "".to_string(),
            id: short_str,

            url: None,
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

            ws_client: None,
            ftp_stream: None,
            mqtt_client: None,
            mqtt_connection: None,
            quic_connection: None,
            udp_socket: None,
            event_poll: None,
        }
    }
}

#[derive(Clone)]
 pub struct Client {
    pub protocol: PROTOCOLS,
    pub raw_url: String, // url given by the user. This is used for connection and request
    pub id: String, // unique id for the client

    // parsed url. these fields are extracted from the url string
    // Not directly used for connection or request. Just for information
    pub url: Option<XrUrl>,
    pub host: Option<String>,
    pub port: Option<u32>,
    pub path: Option<String>,

    req_headers: Option<Vec<(String, String)>>,
    req_body: Option<String>,
    timeout: Option<u64>,
    redirection: bool,
    method: Option<String>,

    pub user: Option<String>,
    pub password: Option<String>,

    pub ws_client: Option<XrdsWebsocket>,
    pub ftp_stream: Option<Arc<Mutex<FtpStream>>>,
    pub mqtt_client: Option<MqttClient>,
    pub mqtt_connection: Option<Arc<Mutex<MqttConnection>>>,

    /* QUIC */
    pub quic_connection: Option<Arc<Mutex<quiche::Connection>>>,
    pub udp_socket: Option<Arc<Mutex<mio::net::UdpSocket>>>,
    pub event_poll: Option<Arc<Mutex<mio::Poll>>>,


 }

 impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client.Deubg: To Be Implemented")
    }
}

impl Client {
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

    /******************************************** */
    /*************     CONNECTION       ********* */
    /******************************************** */
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

        self.url = Some(parse_result.as_ref().unwrap().clone());
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
            PROTOCOLS::QUIC => self.connect_quic(),
            _ => Err("The protocol does not support 'Connect'. Use 'Request' instead.".to_string()),
        };

        return result;
    }

    /******************************************** */
    /*************         SEND         ********* */
    /******************************************** */
    /**
     * topic is required for MQTT
     * topic is optional for WS, and indicates the message type (binary, text, etc.)
     * if no message type is given for WS, it will be considered as binary
     */
    pub fn send(self, data: Vec<u8>, topic: Option<&str>) -> Result<Self, String> {
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::WS | PROTOCOLS::WSS => self.send_ws(topic, data),
            PROTOCOLS::MQTT => self.send_mqtt(topic, data),
            // PROTOCOLS::WEBRTC => self.send_webrtc(message),
            PROTOCOLS::QUIC => self.send_quic(data),
            _ => Err("The protocol does not support 'Send'. Use another method instead.".to_string()),
        };

        return result;
    }

    /******************************************** */
    /*************       RECEIVE        ********* */
    /******************************************** */
    pub fn rcv(&self) -> Result<Vec<u8>, String> {
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::WS | PROTOCOLS::WSS => &self.rcv_ws(),
            PROTOCOLS::MQTT => &self.rcv_mqtt(),
            // PROTOCOLS::WEBRTC => self.rcv_webrtc(),
            PROTOCOLS::QUIC => &self.rcv_quic(),
            _ => &Err("The protocol does not support 'Receive'. Use another method instead".to_string()),
        };

        result.clone()
    }

    pub fn close(&self) -> Result<(), String> {
        // check the protocol
        let result = match self.protocol {
            PROTOCOLS::WS | PROTOCOLS::WSS => self.close_ws(),
            // PROTOCOLS::MQTT => self.close_mqtt(),
            // PROTOCOLS::WEBRTC => self.close_webrtc(),
            // PROTOCOLS::QUIC => self.close_quic(),
            _ => Err("The protocol does not support 'Close'. Use another method instead".to_string()),
        };

        return result;
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
            .publish(topic.unwrap(), QoS::AtLeastOnce, true, message);
        if publish_result.is_err() {
            return Err(publish_result.err().unwrap().to_string());
        } else {
            // wait for puback confirmation in timeout
            let start_time = Instant::now();
            loop {
                if start_time.elapsed() > Duration::from_secs(5) {
                    return Err("Publish confirmation timed out.".to_string());
                }

                let message = self.rcv_mqtt();
                if message.is_err() {
                    return Err(message.err().unwrap().to_string());
                } else {
                    let message = message.unwrap();
                    if message == b"PUBACK_CONFIRMED".to_vec() {
                        return Ok(self); // Publish confirmed
                    }
                }
            }
        }
    }

    /**
     * Receives the 'Publish' event only from the connection
     * if the received event is not 'Publish', it will return the empty Vec<u8>
     */
    fn rcv_mqtt(&self) -> Result<Vec<u8>, String> {
        let mqtt_connection = self.mqtt_connection.as_ref().unwrap();
        let notification = mqtt_connection.lock().unwrap().recv();
        match notification {
            Err(recv_err) => {
                let err_msg = format!("Error occurred while receiving the message: {:?}", recv_err);
                return Err(err_msg);
            }
            Ok(inner_result) => {
                match inner_result {
                    Ok(event) => {
                        match event {
                            Event::Incoming(incoming) => {
                                match incoming {
                                    Incoming::Publish(message) => {
                                        let result_vec = Vec::from(message.payload);
                                        println!("Received MQTT Publish message: {:?}", result_vec);
                                        return Ok(result_vec);
                                    }
                                    Incoming::Subscribe(message) => {
                                        println!("Subscription success: {:?}", message);
                                        Ok(Vec::new())
                                    }
                                    Incoming::SubAck(message) => {
                                        println!("SubAck received: {:?}", message.return_codes);
                                        // Return a special marker for SUBACK instead of empty Vec
                                        Ok(b"SUBACK_CONFIRMED".to_vec())
                                    }
                                    Incoming::PubAck(message) => {
                                        println!("PubAck received: {:?}", message);
                                        Ok(b"PUBACK_CONFIRMED".to_vec())
                                    }
                                    _ => {
                                        println!("Received other MQTT event: {:?}", incoming);
                                        Ok(Vec::new()) // Still return empty for other events
                                    }
                                }
                            }
                            Event::Outgoing(_outgoing) => {
                                Ok(Vec::new())
                            }
                        }
                    }
                    Err(conn_err) => {
                        let err_msg = format!("Error occurred while receiving the message: {:?}", conn_err);
                        return Err(err_msg);
                    }
                }
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
            // wait til subscription is confirmed with SUBACK in timeout
            let start_time = Instant::now();
            loop {
                if start_time.elapsed() > Duration::from_secs(5) {
                    return Err("Subscription confirmation timed out.".to_string());
                }

                let message = self.rcv_mqtt();
                if message.is_err() {
                    return Err(message.err().unwrap().to_string());
                } else {
                    let message = message.unwrap();
                    if message == b"SUBACK_CONFIRMED".to_vec() {
                        let mst_str = String::from_utf8(message.clone());
                        println!("Subscription message string: {:?}", mst_str);
                        return Ok(self); // Subscription confirmed
                    }
                }
            }
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
        let client_result = XrdsWebsocket::new().connect(self.raw_url.as_str());

        if client_result.is_err() {
            return Err(client_result.err().unwrap().to_string());
        } else {
            self.ws_client = Some(client_result.unwrap());
            return Ok(self);
        }
    }

    fn send_ws(self, msg_type: Option<&str>, message: Vec<u8>) -> Result<Self, String> {
        let ws_client = self.ws_client.as_ref().unwrap();

        let send_result = ws_client.send_ws(msg_type, message);
        if send_result.is_err() {
            return Err(send_result.err().unwrap().to_string());
        } else {
            return Ok(self);
        }
    }

    fn rcv_ws(&self) -> Result<Vec<u8>, String> {
        let ws_client = self.ws_client.as_ref().unwrap();
        let message = ws_client.rcv_ws();

        if message.is_err() {
            return Err(message.err().unwrap().to_string());
        } else {
            return Ok(message.unwrap());
        }
    }

    fn close_ws(&self) -> Result<(), String> {
        let ws_client = self.ws_client.as_ref().unwrap();
        let close_result = ws_client.close_ws();

        if close_result.is_err() {
            return Err(close_result.err().unwrap().to_string());
        } else {
            return Ok(());
        }
    }

    /**************************** */
    /* REQUEST-RESPONSE PROTOCOLS */
    /**************************** */
    /**
     * request to the server
     */
    pub fn request(mut self) -> NetResponse {
        let parsed_url: Result<XrUrl, String> = crate::common::parse_url(&self.raw_url);
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
            self.url = Some(parsed_url.clone());
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
            PROTOCOLS::HTTP3 => self.request_http3(),
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

        if self.method == Some("POST".to_string()) {
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


    /******************************** ****************/
    /* *****************    COAP    ******************/
    /******************************** ****************/
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

        let body_result = String::from_utf8(coap_res_payload);
        let body: String;
        if body_result.is_err() {
            body = "".to_string();
        } else {
            body = body_result.unwrap();
        }

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

    /******************************** ****************/
    /* ***************** FTP & FTPS ******************/
    /******************************** ****************/
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


    /********************************** */
    /********* QUIC PROTOCOL ************/
    /********************************** */
    fn connect_quic(mut self) -> Result<Self, String> {
        let url = self.url.as_ref().ok_or("URL not set")?;
        let peer_addr = url.socket_addrs().map_err(|e| e.to_string())?;
        let bind_addr = "0.0.0.0:0".to_string();

        let mut socket: mio::net::UdpSocket = mio::net::UdpSocket::bind(bind_addr.parse().unwrap()).unwrap();
        let local_addr = socket.local_addr().unwrap();

        let mut quic_config = self.create_quic_config();

        // scid MUST be 20 bytes long
        let scid = generate_random_string(20);
        let scid = quiche::ConnectionId::from_ref(&scid.as_bytes());

        let mut poll = mio::Poll::new().unwrap();
        let poll_result = poll.registry().register(&mut socket, mio::Token(0), mio::Interest::READABLE);
        if poll_result.is_err() {
            return Err(poll_result.err().unwrap().to_string());
        }

        let mut conn = quiche::connect(Some(self.host.clone().unwrap().as_str()), &scid, 
            local_addr, peer_addr, &mut quic_config).map_err(|e| e.to_string())?;

        // Start the QUIC connection
        Self::send_initial_packet(&mut socket, &mut conn)?;

        // Condition of breaking the loop: Connection is closed or established
        Self::event_loop(&mut socket, &mut conn, &mut poll)?;

        if conn.is_closed() {
            return Err("Connection closed.".to_string());
        }

        self.quic_connection = Some(Arc::new(Mutex::new(conn)));
        self.udp_socket = Some(Arc::new(Mutex::new(socket)));
        self.event_poll = Some(Arc::new(Mutex::new(poll)));

        Ok(self)

    }

    // TODO: unit test
    fn send_quic(mut self, mut data: Vec<u8>) -> Result<Self, String> {
        let conn = self.quic_connection.as_mut().ok_or("QUIC connection is not initialized")?;

        Self::handle_write( &mut self.udp_socket.as_mut().unwrap().lock().unwrap(),
             &mut conn.lock().unwrap(), &mut data)?;

        Ok(self)
    }

    fn rcv_quic(&self) -> Result<Vec<u8>, String> {
        let conn = self.quic_connection.as_ref().ok_or("QUIC connection is not initialized")?;
        let mut buf = [0; 65535];
        let mut socket = self.udp_socket.as_ref().unwrap().lock().unwrap();
        let mut conn = conn.lock().unwrap();

        Self::handle_read(&mut socket, &mut conn, &mut buf)?;

        Ok(buf.to_vec())
    }

    fn create_quic_config(&self) -> quiche::Config {
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
        config.verify_peer(false);

        config.set_application_protos(quiche::h3::APPLICATION_PROTOCOL).unwrap();
        config.set_max_idle_timeout(30_000);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_stream_data_uni(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);
        config.verify_peer(true);

        config
    }

    fn send_initial_packet(socket: &mut UdpSocket, conn: &mut Connection) -> Result<(), String> {
        let mut out = [0; MAX_DATAGRAM_SIZE];
        let (write, send_info) = conn.send(&mut out).expect("initial send failed");
        while let Err(e) = socket.send_to(&out[..write], send_info.to) {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                continue;
            }
            return Err(format!("send() failed: {:?}", e));
        }
        Ok(())
    }

    /**
     * For QUIC only
     */
    pub fn start_event_loop(&self, socket: Arc<Mutex<UdpSocket>>, 
            conn: Arc<Mutex<Connection>>, poll: Arc<Mutex<Poll>>) {
                println!("[start_event_loop] Start the event loop");
        thread::spawn(move || {
            let mut events = Events::with_capacity(1024);
            let mut buf = [0; 65535];
            let mut out = [0; MAX_DATAGRAM_SIZE];

            loop {
                println!("Event loop is running...");
                // sleep for 1 sec
                thread::sleep(Duration::from_secs(1));

                {
                    let conn = conn.lock().unwrap();
                    if conn.is_closed() {
                        println!("[start_event_loop]Connection closed.");
                        break;
                    }
                }
                
                {
                    let mut poll = poll.lock().unwrap();
                    poll.poll(&mut events, None).unwrap();
                }

                {
                    let mut conn = conn.lock().unwrap();
                    let mut socket = socket.lock().unwrap();
                    let _ = Self::handle_read(&mut socket, &mut conn, &mut buf);
                    let _ = Self::handle_write(&mut socket, &mut conn, &mut out);
                }
            }   // end of loop
            println!("[start_event_loop] Event loop is finished.");
        });

        
    }

    /* Used for initial handshake */
    fn event_loop(socket: &mut UdpSocket, conn: &mut Connection, poll: &mut Poll) -> Result<(), String> {
        let mut events = Events::with_capacity(1024);
        let mut buf = [0; 65535];
        let mut out = [0; MAX_DATAGRAM_SIZE];

        loop {
            poll.poll(&mut events, conn.timeout()).map_err(|e| e.to_string())?;
            if conn.is_closed() {
                println!("Connection closed.");
                break;
            }

            if conn.is_established() {
                println!("Connection established.");
                break;
            }

            Self::handle_read(socket, conn, &mut buf)?;
            Self::handle_write(socket, conn, &mut out)?;
        }
        
        Ok(())
    }

    fn handle_read(socket: &mut UdpSocket, conn: &mut Connection, buf: &mut [u8]) -> Result<(), String> {
        let (len, from) = match socket.recv_from(buf) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) => return Err(format!("recv() failed: {:?}", e)),
        };

        let recv_info = RecvInfo { to: socket.local_addr().unwrap(), from };

        conn.recv(&mut buf[..len], recv_info).map_err(|e| format!("recv failed: {:?}", e))?;
        Ok(())
    }

    fn handle_write(socket: &mut UdpSocket, conn: &mut Connection, out: &mut [u8]) -> Result<(), String> {
        loop {
            let (write, send_info) = match conn.send(out) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => {
                    conn.close(false, 0x1, b"fail").ok();
                    return Err(format!("send failed: {:?}", e));
                }
            };

            if let Err(e) = socket.send_to(&out[..write], send_info.to) {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    break;
                }
                return Err(format!("send() failed: {:?}", e));
            }

            println!("Sent {} bytes", write);
        }

        Ok(())
    }

    /********************************** */
    /*************** HTTP/3 *************/
    /********************************** */
    /**
     * Mandatory headers for HTTP3 MUJST be included in the request
     *  - RFC 9114 Section 4.3.1
     *  - https://datatracker.ietf.org/doc/html/rfc9114#section-4.3.1
     */
    fn request_http3(&self) -> NetResponse {
        let start_time = Instant::now();
        let max_duration = Duration::from_secs(30);
        
        let mut response = NetResponse {
            protocol: PROTOCOLS::HTTP3,
            status_code: 0,
            headers: vec![],
            body: Vec::new(),
            error: None,
        };

        // meta data preparation
        let url = self.clone().url.unwrap();
        let peer_addr = url.socket_addrs().unwrap();
        let bind_addr = match peer_addr {
            std::net::SocketAddr::V4(_) => "0.0.0.0:0",
            std::net::SocketAddr::V6(_) => "[::]:0",
        };

        // Need improvement on header setting. in case of using .set_method() method or not 
        let req_headers = match self.req_headers.clone() {
            Some(headers) => fill_mandatory_http_headers(url.clone(), Some(headers), self.method.clone()),
            None => fill_mandatory_http_headers(url.clone(), None, self.method.clone()),
        };

        let mut socket = mio::net::UdpSocket::bind(bind_addr.parse().unwrap()).unwrap();
        let mut poll = mio::Poll::new().unwrap();
        poll.registry()
            .register(&mut socket, mio::Token(0), mio::Interest::READABLE)
            .unwrap();

        let scid = generate_random_string(20);
        let scid = quiche::ConnectionId::from_ref(&scid.as_bytes());

        let mut quic_config = self.create_quic_config();

        let local_addr = socket.local_addr().unwrap();
        let mut conn = quiche::connect(Some(url.host.as_str())
            , &scid, local_addr, peer_addr, &mut quic_config).unwrap();

        // QUIC Initialization
        let mut out = [0; MAX_DATAGRAM_SIZE];
        Self::send_packet(&mut socket, &mut conn, &mut out).expect("Initial send failed");

        let h3_config = quiche::h3::Config::new().unwrap();
        let mut http3_conn: Option<quiche::h3::Connection> = None;
        let mut req_sent = false;
        let mut events = mio::Events::with_capacity(1024);
        let mut is_exit = false;

        let handshake_timeout = Duration::from_secs(20);    // New: QUIC handshake timeout
        let connection_timeout = Duration::from_secs(25);   // HTTP/3 connection timeout
        let response_timeout = Duration::from_secs(30);     // Response timeout (increased)
        
        let handshake_start = Instant::now();
        let connection_start = Instant::now();
        let mut handshake_completed = false;
        let mut connection_established = false;
        let mut request_sent_time: Option<Instant> = None;
        let mut buf = [0; 65535];

        loop {
            // Overall timeout check
            if start_time.elapsed() > max_duration {
                response.error = Some(format!("Request timed out after {} seconds", max_duration.as_secs()));
                break;
            }

            if is_exit {
                break;
            }

            // Handshake timeout check
            if !handshake_completed && handshake_start.elapsed() > handshake_timeout {
                response.error = Some("QUIC handshake timeout - check server availability".to_string());
                break;
            }

            // Connection timeout check
            if handshake_completed && !connection_established && connection_start.elapsed() > connection_timeout {
                response.error = Some("HTTP/3 connection establishment timeout".to_string());
                break;
            }

            // Adaptive polling based on connection state
            let poll_timeout = if !handshake_completed {
                Some(Duration::from_millis(100)) // More frequent during handshake
            } else if !connection_established {
                Some(Duration::from_millis(50))  // Frequent during HTTP/3 setup
            } else if request_sent_time.is_none() {
                Some(Duration::from_millis(50))  // Frequent until request sent
            } else {
                // After request sent, use longer timeouts to avoid missing data
                Some(Duration::from_millis(500)) // Less frequent during response wait
            };

            if poll.poll(&mut events, poll_timeout).is_err() {
                response.error = Some("Polling error".to_string());
                break;
            }

            // Check QUIC handshake completion
            if !handshake_completed && conn.is_established() {
                handshake_completed = true;
                println!("QUIC handshake completed in {:?}", handshake_start.elapsed());
            }

            // Handle packet reception with better error handling
            if let Err(e) = Self::receive_packets(&mut socket, &mut conn, &mut buf, local_addr, &mut http3_conn, &h3_config) {
                // Don't fail on recoverable errors
                if !e.contains("would block") && !e.contains("Done") {
                    response.error = Some(e);
                    break;
                }
            }

            if let Some(h3) = http3_conn.as_mut() {
                if !connection_established {
                    connection_established = true;
                    println!("HTTP/3 connection established in {:?}", connection_start.elapsed());
                }

                if !req_sent {
                    match Self::send_http3_request(h3, &mut conn, req_headers.as_slice()) {
                        Ok(_) => {
                            req_sent = true;
                            request_sent_time = Some(Instant::now());
                            println!("HTTP/3 request sent successfully");
                        }
                        Err(e) => {
                            // Retry on certain errors
                            if e.contains("stream limit") || e.contains("would block") {
                                continue; // Retry on next iteration
                            } else {
                                response.error = Some(e);
                                break;
                            }
                        }
                    }
                }

                // Response timeout check (only after request sent)
                if let Some(sent_time) = request_sent_time {
                    if sent_time.elapsed() > response_timeout {
                        response.error = Some(format!("Response timeout after {} seconds", response_timeout.as_secs()));
                        break;
                    }
                }

                is_exit = match Self::handle_http3_events(h3, &mut conn, &mut buf, &mut response) {
                    Ok(exit) => exit,
                    Err(err) => {
                        response.error = Some(err);
                        true
                    }
                };
            }

            // Send packets with better error handling
            if let Err(e) = Self::send_packet(&mut socket, &mut conn, &mut out) {
                if !e.contains("Done") && !e.contains("would block") {
                    response.error = Some(e);
                    break;
                }
            }

            if conn.is_closed() {
                if response.status_code == 0 && response.body.is_empty() {
                    response.error = Some("Connection closed without receiving response".to_string());
                } else {
                    println!("Connection closed after receiving partial response");
                }
                break;
            }

            if conn.is_draining() {
                println!("Connection is draining");
                if response.status_code > 0 {
                    println!("Received response before connection drain");
                    break;
                }
            }
        }

        response
    }

    fn send_packet(
        socket: &mut mio::net::UdpSocket,
        conn: &mut quiche::Connection,
        out: &mut [u8],
    ) -> Result<(), String> {
        while let Ok((write, send_info)) = conn.send(out) {
            socket.send_to(&out[..write], send_info.to).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn receive_packets(
        socket: &mut mio::net::UdpSocket,
        conn: &mut quiche::Connection,
        buf: &mut [u8],
        local_addr: std::net::SocketAddr,
        http3_conn: &mut Option<quiche::h3::Connection>,
        h3_config: &quiche::h3::Config,
    ) -> Result<(), String> {
        while let Ok((len, from)) = socket.recv_from(buf) {
            let recv_info = quiche::RecvInfo { to: local_addr, from };
            if let Err(e) = conn.recv(&mut buf[..len], recv_info) {
                return Err(format!("QUIC recv failed: {:?}", e));
            }
            if conn.is_established() && http3_conn.is_none() {
                *http3_conn = Some(quiche::h3::Connection::with_transport(conn, h3_config).map_err(|e| format!("HTTP3 connection failed: {:?}", e))?);
            }
        }
        Ok(())
    }
    
    fn send_http3_request(
        h3: &mut quiche::h3::Connection,
        conn: &mut quiche::Connection,
        req: &[quiche::h3::Header],
    ) -> Result<(), String> {
        let result = h3.send_request(conn, req, true).map_err(|e| e.to_string());
        if result.is_err() {
            return Err(result.err().unwrap());
        } else {
            return Ok(());
        }
        
    }

    fn handle_http3_events(
        h3: &mut quiche::h3::Connection,
        conn: &mut quiche::Connection,
        buf: &mut [u8],
        response: &mut NetResponse
    ) -> Result<bool, String> {
        let mut events_processed = 0;
        let max_events_per_iteration = 100;
        
        while events_processed < max_events_per_iteration {
            match h3.poll(conn) {
                Ok((stream_id, event)) => {
                    events_processed += 1;
                    println!("HTTP/3 event on stream {}: {:?}", stream_id, event);
                    
                    match event {
                        quiche::h3::Event::Headers { list, more_frames } => {
                            println!("Received headers (count: {}, more_frames: {})", list.len(), more_frames);
                            for header in list {
                                let name = String::from_utf8_lossy(header.name());
                                let value = String::from_utf8_lossy(header.value());
                                println!("  {}: {}", name, value);
                                
                                if name == ":status" {
                                    if let Ok(status_code) = value.parse::<u16>() {
                                        response.status_code = status_code as u32;
                                        println!("Status code set to: {}", status_code);
                                    }
                                }
                                
                                response.headers.push((name.to_string(), value.to_string()));
                            }
                            
                            // If there's no body and no more frames, we might be done
                            if !more_frames && response.status_code > 0 {
                                println!("Response complete (headers only)");
                                return Ok(true);
                            }
                        }
                        quiche::h3::Event::Data => {
                            let mut total_read = 0;
                            loop {
                                match h3.recv_body(conn, stream_id, buf) {
                                    Ok(read) => {
                                        if read > 0 {
                                            response.body.extend_from_slice(&buf[..read]);
                                            total_read += read;
                                            println!("Received {} bytes of data (total this event: {})", read, total_read);
                                        } else {
                                            break;
                                        }
                                    }
                                    Err(quiche::h3::Error::Done) => break,
                                    Err(e) => {
                                        println!("Error reading body: {:?}", e);
                                        break;
                                    }
                                }
                            }
                            
                            if total_read > 0 {
                                println!("Total response body size: {} bytes", response.body.len());
                            }
                        }
                        quiche::h3::Event::Finished => {
                            println!("Stream {} finished. Final response - Status: {}, Body size: {}", 
                                stream_id, response.status_code, response.body.len());
                            return Ok(true);
                        }
                        quiche::h3::Event::Reset(error_code) => {
                            println!("Stream {} reset with error: {}", stream_id, error_code);
                            return Err(format!("Stream reset with error: {}", error_code));
                        }
                        quiche::h3::Event::GoAway => {
                            println!("Received GoAway");
                            if response.status_code > 0 {
                                return Ok(true);
                            }
                            return Err("Server sent GoAway".to_string());
                        }
                        _ => {
                            println!("Other HTTP/3 event: {:?}", event);
                        }
                    }
                }
                Err(quiche::h3::Error::Done) => {
                    break; // No more events
                }
                Err(e) => {
                    println!("HTTP/3 poll error: {:?}", e);
                    return Err(format!("HTTP/3 poll failed: {:?}", e));
                }
            }
        }
        
        Ok(false)
    }

 }