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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::error::Error;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream as WsStream;

use crate::common::data_structure::WebRTCMessage;
use crate::common::generate_uuid;

static CREATE_SESSION: &str = "create_session"; // publisher to server
static LIST_SESSIONS: &str = "list_sessions";   // subscriber to server
static JOIN_SESSION: &str = "join_session";     // subscriber to server
static LEAVE_SESSION: &str = "leave_session";   // subscriber to server
static CLOSE_SESSION: &str = "close_session";   // publisher to server
static OFFER: &str = "offer";                   // publisher to server, server to subscriber
static ANSWER: &str = "answer";                 // subscriber to server
static WELCOME: &str = "welcome";               // server to client (publisher or subscriber)


/**
 * This server is a signaling server for WebRTC.
 * It is based on WebSocket.
 * Purpose
 * - Keep track of connected clients
 * - Provide the way to identify target client
 * - Handle signaling messages (offer, answer, ice candidate)
 * - Deliver signaling messages to the correct client
 * 
 * For now, it supports 1:N uni-directional for media streaming.
 * But, data channel and bi-directional streaming will be supported in the future.
 */
pub struct WebRTCServer {
    clients: Arc<Mutex<HashMap<String, WebRTCClient>>>, // simple client_id, WebRTCClient
    sessions: Arc<Mutex<HashMap<String, Session>>>,  // <session_id, Session>
}

/**
 * This represents a session between two or more clients.
 * like a chat room
 */
#[derive(Clone)]
pub struct Session {
    session_id: String,
    creator_id: String,  // client_id. Only creator can close the session
    participants: Vec<String>,  // a vector of client_ids
    offer: Option<String>,  // SDP offer from the creator. Participants will receive this. base64 encoded
}

impl WebRTCServer {
    pub fn new() -> Self {
        WebRTCServer {
            clients: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn add_client(&self, client_id: &String, peer_addr: &String) {
        let client = WebRTCClient {
            client_id: client_id.clone(),
            peer_addr: peer_addr.clone(),
        };
        self.clients.lock().unwrap().insert(client_id.to_string(), client);
    }

    fn remove_client(&mut self, client_id: &str) {
        self.clients.lock().unwrap().remove(client_id);
    }

    fn get_client(&self, client_id: &str) -> Option<WebRTCClient> {
        self.clients.lock().unwrap().get(client_id).cloned()
    }

    pub async fn run(self: Arc<Self>, port: u32) -> Result<(), Box<dyn std::error::Error>> {
        let host_addr = "0.0.0.0".to_owned() + ":" + &port.to_string();
        let try_socket = TcpListener::bind(host_addr.clone()).await;
        let listener = match try_socket { 
            Ok(l) => {
                println!("WebRTC Signaling server started on {}", host_addr);  // temporal log
                l
            }
            Err(e) => {
                println!("Error binding to {}: {}", host_addr, e);
                return Err(Box::new(e));
            }
        };
        
        while let Ok((stream, addr)) = listener.accept().await {
            println!("Accepted connection from {}", addr);  // temporal log
            
            let self_clone = Arc::clone(&self);
            tokio::spawn({
                async move {
                    match accept_async(stream).await {
                        Ok(ws_stream) => {  // connection established
                            let client_id = generate_uuid();    // generate client's unique id
                            let peer_addr = addr.to_string();

                            self_clone.add_client(&client_id, &peer_addr);

                            if let Err(e) = self_clone.handle_connection(client_id, ws_stream).await {
                                println!("Error handling connection from {}: {}", addr, e);
                            }
                        }
                        Err(e) => println!("Error accepting WebSocket connection from {}: {}", addr, e),
                    }
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(&self, client_id: String, ws_stream: WsStream<TcpStream>) -> Result<(), Box<dyn std::error::Error>> {
        let (mut sender, mut receiver) = ws_stream.split();

        // Send welcome message with the issued client id
        let welcome_msg = WebRTCMessage {
            client_id: client_id.clone(),
            message_type: "welcome".to_string(),
            payload: "".as_bytes().to_vec(),
            sdp: None,
            error: None,
        };
        
        let client_id_msg_json = serde_json::to_string(&welcome_msg).unwrap();
        let client_id_msg = Message::text(client_id_msg_json.to_string());

        if let Err(e) = sender.send(client_id_msg).await {
            println!("Error sending welcome message: {}", e);
            return Err(Box::new(e));
        }

        // handle incoming messages
        while let Some(msg) = receiver.next().await {
            println!("WebRTC Server Received message: {:?}", msg);
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    Self::log_error_connection(e);

                    if let Err(close_err) = sender.send(Message::Close(None)).await {
                        println!("Failed to send close frame: {}", close_err);
                    }
                    break;
                }
            };

            if msg.is_close() {
                println!("Connection closed by client");
                break;
            }

            println!("preparing message back to client");
            let result = self.signaling_handler(msg.into_data().to_vec());
            // prepare message back to client
            if result.is_some() {
                // send message in text since it's json only
                let msg = Message::text(String::from_utf8_lossy(&result.unwrap()).to_string());
                if let Err(e) = sender.send(msg).await {
                    println!("Error sending message: {}", e);
                    continue;
                }
            }
        }
        Ok(())
        
    }

    /**
     * This is a signaling message handler.
     * It is called when a signaling message is received.
     * It returns a response message.
     */
    fn signaling_handler(&self, input: Vec<u8>) -> Option<Vec<u8>> {
        // parse input
        let msg = String::from_utf8_lossy(&input);
        let msg: WebRTCMessage = serde_json::from_str(msg.as_ref()).unwrap();
        let message_type = msg.clone().message_type;
        // handle message by matching message_types

        if message_type == CREATE_SESSION {
            let response = self.handle_create_session(msg);
            let response = serde_json::to_string(&response).unwrap();
            return Some(response.into_bytes());
        } else if message_type == OFFER {
            return None;
        } else if message_type == LIST_SESSIONS {
            let response = self.handle_list_session(msg);
            let response = serde_json::to_string(&response).unwrap();
            return Some(response.into_bytes());
        } else if message_type == JOIN_SESSION {    // Start of WebRTC Connection Creation
            return None;

        } else if message_type == LEAVE_SESSION {
           return None; 
        } else {
            //lefties: offer, answer, ice candidate
            // unknown message type
            return None;
        }
    }

    fn log_error_connection(error: Error) {
        match &error {
            Error::ConnectionClosed => {
                println!("Connection closed normally (but no Close frame?).");
            }
            Error::Io(io_err) => {
                match io_err.kind() {
                    std::io::ErrorKind::ConnectionReset => {
                        println!("Client rudely dropped connection (ConnectionReset).");
                    }
                    std::io::ErrorKind::BrokenPipe => {
                        println!("Client terminated socket without handshake (BrokenPipe).");
                    }
                    std::io::ErrorKind::ConnectionAborted => {
                        println!("Client terminated socket with handshake (ConnectionAborted).");
                    }
                    _ => {
                        println!("Unexpected I/O error: {}", io_err);
                    }
                }
            }
            Error::Protocol(proto_err) => {
                println!("Protocol violation by client: {}", proto_err);
            }
            _ => {
                println!("Other WebSocket error: {}", error);
            }
        }
    }

    //helper function
    fn handle_create_session(&self, request: WebRTCMessage) -> WebRTCMessage{
        // create a new session
        let session_id = generate_uuid();
        let session = Session {
            session_id: session_id.clone(),
            creator_id: request.client_id.clone(),
            participants: vec![request.client_id.clone()],
            offer: None,
        };

        self.sessions.lock().unwrap().insert(session_id.clone(), session);

        let response = WebRTCMessage {
            client_id: request.client_id,
            message_type: CREATE_SESSION.to_string(),
            payload: session_id.into_bytes(),
            sdp: None,
            error: None,
        };
        response
    }

    fn handle_list_session(&self, request: WebRTCMessage) -> WebRTCMessage {
        // list all sessions
        let sessions = self.sessions.lock().unwrap();
        let session_ids: Vec<String> = sessions.keys().cloned().collect();

        let response = WebRTCMessage {
            client_id: request.client_id,
            message_type: LIST_SESSIONS.to_string(),
            payload: serde_json::to_string(&session_ids).unwrap().into_bytes(),
            sdp: None,
            error: None,
        };
        response
    }
}

#[derive(Default, Clone)]
pub struct WebRTCClient {
    client_id: String,  // This field is used to identify the client in the signaling server
    peer_addr: String,
}