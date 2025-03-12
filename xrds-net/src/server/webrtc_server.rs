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
use tokio::sync::Mutex as AsyncMutex;

use futures::stream::SplitSink;
use futures::stream::SplitStream;
use futures::{SinkExt, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::error::Error;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream as WsStream;

use crate::common::data_structure::WebRTCMessage;
use crate::common::generate_uuid;
use crate::common::data_structure::{CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER};


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
    clients: Arc<AsyncMutex<HashMap<String, WebRTCClient>>>, // simple client_id, WebRTCClient
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
            clients: Arc::new(AsyncMutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn add_client(&self, client_id: &String, peer_addr: &String, ws_stream: WsStream<TcpStream>) {
        let mut clients = self.clients.lock().await;
        clients.insert(client_id.clone(), WebRTCClient::new(client_id.to_string(), peer_addr.to_string(), ws_stream));
        println!("Client {} added", client_id);
    }

    fn remove_client(&self, client_id: &String) {
        let mut clients = self.clients.blocking_lock();
        clients.remove(client_id);
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

                            if let Err(e) = self_clone.handle_connection(client_id, peer_addr, ws_stream).await {
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

    async fn handle_connection(&self, client_id: String, peer_addr: String, ws_stream: WsStream<TcpStream>) 
    -> Result<(), Box<dyn std::error::Error>> {
        self.add_client(&client_id.clone(), &peer_addr, ws_stream).await;
        let clients = self.clients.lock().await;
        let client = clients.get(&client_id).unwrap();
        let mut sender = client.sender.lock().await;
        let mut receiver = client.receiver.lock().await;
        
        // Send welcome message with the issued client id
        let welcome_msg = WebRTCMessage {
            client_id: client_id.clone(),
            message_type: "WELCOME".to_string(),
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
            // println!("WebRTC Server Received message: {:?}", msg);
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    log_error_connection(e);

                    if let Err(close_err) = sender.send(Message::Close(None)).await {
                        println!("Failed to send close frame: {}", close_err);
                    }
                    break;
                }
            };

            if msg.is_close() {
                self.remove_client(&client_id);
                // TODO: remove the session if the client is the creator
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
        // println!("Received message: {}", msg);  // temporal log
        let msg: WebRTCMessage = serde_json::from_str(msg.as_ref()).unwrap();
        let message_type = msg.clone().message_type;

        // handle message by matching message_types
        match message_type.as_str() {
            CREATE_SESSION => {
                let response = self.handle_create_session(msg);
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            LIST_SESSIONS => {
                let response = self.handle_list_session(msg);
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            CLOSE_SESSION | JOIN_SESSION | LEAVE_SESSION | LIST_PARTICIPANTS => {
                let session_id = String::from_utf8_lossy(&msg.payload).to_string();
                
                let response = match message_type.as_str() {
                    CLOSE_SESSION => self.close_session(&session_id),
                    JOIN_SESSION => self.join_session(session_id.clone(), &msg.client_id),
                    LEAVE_SESSION => self.leave_session(session_id, &msg.client_id),
                    LIST_PARTICIPANTS => self.list_participants(&session_id),
                    _ => unreachable!(), // This won't happen due to the outer match
                };
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            OFFER => {
                let response = self.handle_offer(msg);
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            ANSWER | _ => None  // Handle OFFER, ANSWER, and any other message type
        }
    }

    /****************** Message Handler Functions **************** */
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

        println!("Session {} created by {}", session_id, request.client_id);

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

    fn handle_offer(&self, request: WebRTCMessage) -> WebRTCMessage {
        // handle offer
        let session_id = String::from_utf8_lossy(&request.payload).to_string();
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(&session_id).unwrap();
        session.offer = Some(request.sdp.clone().unwrap());

        // send offer to all participants except the creator
        let participants = session.participants.clone();

        let participants = participants.into_iter().filter(|x| x != &session.creator_id).collect::<Vec<String>>();
        
        let offer_msg = WebRTCMessage { // message to be sent to participants
            client_id: session.creator_id.clone(),
            message_type: OFFER.to_string(),
            payload: session_id.clone().into_bytes(),
            sdp: session.offer.clone(),
            error: None,
        };

        // send offer to all participants except the creator
        let _ = self.broadcast_message(participants, offer_msg.clone());

        // make result for creator
        let response = WebRTCMessage {
            client_id: session.creator_id.clone(),
            message_type: OFFER.to_string(),
            payload: session_id.into_bytes(),
            sdp: None,
            error: None,
        };
        response
    }

    /**
     * Returns a response message for closing a session with remaining session lists.
     */
    fn close_session(&self, session_id: &str) -> WebRTCMessage{
        self.sessions.lock().unwrap().remove(session_id);

        // get remaining session list
        let sessions = self.sessions.lock().unwrap();
        let session_ids: Vec<String> = sessions.keys().cloned().collect();

        let response = WebRTCMessage {
            client_id: "".to_string(),
            message_type: CLOSE_SESSION.to_string(),
            payload: session_ids.join(",").into_bytes(),
            sdp: None,
            error: None,
        };
        response
    }

    fn join_session(&self, session_id: String, client_id: &str) -> WebRTCMessage{
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(&session_id).unwrap();
        if session.participants.contains(&client_id.to_string()) {  // reset the session for the client
            // remove the client from the session
            session.participants.retain(|x| x != client_id);
            // TODO: remove the WebRTC connection too. (not implemented yet)
            // setup the webrtc connection again
        }

        session.participants.push(client_id.to_string());

        // if sdp exists, send it to the client
        let sdp = session.offer.clone().unwrap_or_default();

        let response = WebRTCMessage {
            client_id: client_id.to_string(),
            message_type: JOIN_SESSION.to_string(),
            payload: session_id.into_bytes(),
            sdp: Some(sdp),
            error: None,
        };
        response
    }

    fn leave_session(&self, session_id: String, client_id: &str) -> WebRTCMessage{
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(&session_id).unwrap();

        session.participants.retain(|x| x != client_id);

        // TODO: remove the WebRTC connection too. (not implemented yet)

        let response = WebRTCMessage {
            client_id: client_id.to_string(),
            message_type: LEAVE_SESSION.to_string(),
            payload: session_id.into_bytes(),
            sdp: None,
            error: None,
        };
        response
    }

    fn list_participants(&self, session_id: &str) -> WebRTCMessage {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(session_id).unwrap();
        let participants = session.participants.clone();
        let response = WebRTCMessage {
            client_id: "".to_string(),
            message_type: LIST_PARTICIPANTS.to_string(),
            payload: serde_json::to_string(&participants).unwrap().into_bytes(),
            sdp: None,
            error: None,
        };
        response
    }

    async fn broadcast_message(&self, client_ids: Vec<String>, message: WebRTCMessage) {
        let clients = self.clients.lock().await;
        for client_id in client_ids {

            if let Some(client) = clients.get(&client_id) {
                let mut sender = client.sender.lock().await;
                let msg = Message::text(serde_json::to_string(&message).unwrap());
                if let Err(e) = sender.send(msg).await {
                    println!("Error sending message to {}: {}", client_id, e);
                }
            }
        }
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

struct WebRTCClient {
    client_id: String,
    peer_addr: String,
    sender: Arc<AsyncMutex<SplitSink<WsStream<TcpStream>, Message>>>,
    receiver: Arc<AsyncMutex<SplitStream<WsStream<TcpStream>>>>,
}

impl WebRTCClient {
    pub fn new(client_id: String, peer_addr: String, ws_stream: WsStream<TcpStream>) -> Self {
        let (sender, receiver) = ws_stream.split();
        WebRTCClient {
            client_id,
            peer_addr,
            sender: Arc::new(AsyncMutex::new(sender)),
            receiver: Arc::new(AsyncMutex::new(receiver)),
        }
    }
}