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
use std::sync::Arc;

use futures::stream::SplitSink;
use futures::stream::SplitStream;
use futures::{SinkExt, StreamExt};
use tokio::sync::Mutex as AsyncMutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::error::Error;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream as WsStream;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::interceptor::registry::Registry;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_server::RTCIceServer;

use crate::common::data_structure::ICE_CANDIDATE_ACK;
use crate::common::data_structure::{WebRTCMessage, WELCOME};
use crate::common::generate_uuid;
use crate::common::data_structure::{
    CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, LEAVE_SESSION, 
    CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER, ICE_CANDIDATE};


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
    sessions: Arc<AsyncMutex<HashMap<String, Session>>>,  // <session_id, Session>
    api: Option<webrtc::api::API>,   // in case of SFU
    rtc_config: Option<RTCConfiguration>,
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
    offer: Option<String>,  // SDP offer from the creator. Participants will receive this.
    answers: Option<HashMap<String, String>>,  // <client_id, SDP answer>
    publisher_ice_candidates: Option<Vec<String>>,  // json str of RTCIceCandidate
    // publisher_pc: Option<Arc<AsyncMutex<RTCPeerConnection>>>,    // publisher's RTCPeerConnection
    // subscriber_pcs: Option<Arc<AsyncMutex<HashMap<String, RTCPeerConnection>>>>,  // <client_id, RTCPeerConnection>
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

impl WebRTCServer {
    pub fn new() -> Self {
        let mut server = WebRTCServer {
            clients: Arc::new(AsyncMutex::new(HashMap::new())),
            sessions: Arc::new(AsyncMutex::new(HashMap::new())),
            api: None,
            rtc_config: None,
        };
        server.setup_webrtc().unwrap();  // setup webrtc
        server
    }

    fn setup_webrtc(&mut self) -> Result<(), String> {
        let mut m = MediaEngine::default();
        let _ = m.register_default_codecs();

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m).map_err(|e| e.to_string())?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let rtc_config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ice_candidate_pool_size: 10,
            ..Default::default()
        };

        self.api = Some(api);
        self.rtc_config = Some(rtc_config.clone());

        Ok(())
    }

    async fn add_client(&self, client_id: &String, peer_addr: &String, ws_stream: WsStream<TcpStream>) {
        println!("wait for client lock");  // temporal log
        let mut clients = self.clients.lock().await;
        clients.insert(client_id.clone(), WebRTCClient::new(client_id.to_string(), peer_addr.to_string(), ws_stream));
        println!("Client {} added", client_id);
        drop(clients);  // release the lock
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
                            // println!("Generated client id: {}", client_id);  // temporal log
                            self_clone.add_client(&client_id.clone(), &peer_addr, ws_stream).await;
                            self_clone.handle_connection(client_id).await.unwrap_or_else(|e| {
                                println!("Error handling connection: {}", e);
                            });
                        }
                        Err(e) => println!("Error accepting WebSocket connection from {}: {}", addr, e),
                    }
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(&self, client_id: String) 
    -> Result<(), Box<dyn std::error::Error>> {

        let (sender, receiver) = {
            let clients = self.clients.lock().await;
            let client = clients.get(&client_id).unwrap();
            (
                Arc::clone(&client.sender),
                Arc::clone(&client.receiver),
            )
        };  // lock on clients is released here
        
        // Send welcome message with the issued client id
        let welcome_msg = WebRTCMessage {
            client_id: client_id.clone(),
            session_id: "".to_string(),
            message_type: WELCOME.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };
        
        let client_id_msg_json = serde_json::to_string(&welcome_msg).unwrap();
        let client_id_msg = Message::text(client_id_msg_json.to_string());
        {
            let mut sender = sender.lock().await;
            if let Err(e) = sender.send(client_id_msg).await {
                println!("Error sending welcome message: {}", e);
                return Err(Box::new(e));
            }
        }
        
        {
            let mut receiver = receiver.lock().await;
            // handle incoming messages
            while let Some(msg) = receiver.next().await {
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(e) => {
                        log_error_connection(e);
                        let mut sender = sender.lock().await;
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

                // println!("preparing message back to client");
                let result = self.signaling_handler(msg.into_data().to_vec()).await;
                // prepare message back to client
                if result.is_some() {
                    // send message in text since it's json only
                    let msg = Message::text(String::from_utf8_lossy(&result.unwrap()).to_string());
                    let mut sender = sender.lock().await;
                    if let Err(e) = sender.send(msg).await {
                        println!("Error sending message: {}", e);
                        continue;
                    }
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
    async fn signaling_handler(&self, input: Vec<u8>) -> Option<Vec<u8>> {
        // parse input
        let msg = String::from_utf8_lossy(&input);
        // println!("Received message: {}", msg);  // temporal log
        let msg: WebRTCMessage = serde_json::from_str(msg.as_ref()).unwrap();
        let message_type = msg.clone().message_type;

        // handle message by matching message_types
        match message_type.as_str() {
            CREATE_SESSION => {
                let response = self.handle_create_session(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            LIST_SESSIONS => {
                let response = self.handle_list_session(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            CLOSE_SESSION | JOIN_SESSION | LEAVE_SESSION | 
                LIST_PARTICIPANTS | ICE_CANDIDATE | ICE_CANDIDATE_ACK=> {
                let session_id = msg.session_id.clone();
                
                // print session id
                println!("signalhandling.Session ID: {}", session_id);  // temporal log
                
                let response = match message_type.as_str() {
                    CLOSE_SESSION => self.close_session(&session_id).await,
                    JOIN_SESSION => self.join_session(session_id.clone(), &msg.client_id).await,
                    LEAVE_SESSION => self.leave_session(session_id, &msg.client_id).await,
                    LIST_PARTICIPANTS => self.list_participants(&session_id).await,
                    ICE_CANDIDATE => self.handle_ice_candidate(msg).await,
                    ICE_CANDIDATE_ACK => self.handle_ice_candidate_ack(msg).await,
                    _ => unreachable!(), // This won't happen due to the outer match
                };
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            OFFER => {
                let response = self.handle_offer(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            ANSWER => {
                let response = self.handle_answer(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            },
            _ => None, // unknown message type
        }
    }

    /****************** Message Handler Functions **************** */

    /**
     * This function handles the answer message from the subscriber.
     * It updates the session with the answer
     */
    async fn handle_answer(&self, request: WebRTCMessage) -> WebRTCMessage {
        // println!("Answer {:?}", request);  // temporal log

        let session_id = request.session_id.clone();
        let subscriber_id = request.client_id.clone();
        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(&session_id).unwrap();

        if session.answers.is_none() {
            session.answers = Some(HashMap::new());
        }
        // Store the corresponding answer in the session for each subscriber
        session.answers.as_mut().unwrap().insert(subscriber_id.clone(), request.sdp.clone().unwrap());
        println!("Number of answers in session {}: {}", session_id, session.answers.as_ref().unwrap().len());
        
        // send answer to the publisher
        let publisher_id = session.creator_id.clone();
        println!("publisher id: {}", publisher_id);  // temporal log
        let publisher_msg = WebRTCMessage {
            client_id: publisher_id.clone(),
            session_id: session_id.clone(),
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp: request.sdp.clone(),
            error: None,
        };
        
        let _ = self.broadcast_message(vec![publisher_id], publisher_msg.clone()).await;

        // response to the subscriber
        let answer_msg = WebRTCMessage {
            client_id: subscriber_id.clone(),
            session_id: session_id.clone(),
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };        
        answer_msg
    }

    async fn handle_create_session(&self, request: WebRTCMessage) -> WebRTCMessage{
        // create a new session
        let session_id = generate_uuid();
        let session = Session {
            session_id: session_id.clone(),
            creator_id: request.client_id.clone(),
            participants: vec![request.client_id.clone()],
            offer: None,
            answers: None,
            publisher_ice_candidates: None,
            // publisher_pc: None,
            // subscriber_pcs: None,
        };

        self.sessions.lock().await.insert(session_id.clone(), session);

        println!("Session {} created by {}", session_id, request.client_id);

        let response = WebRTCMessage {
            client_id: request.client_id,
            session_id: session_id.clone(),
            message_type: CREATE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };
        response
    }

    async fn handle_list_session(&self, request: WebRTCMessage) -> WebRTCMessage {
        // list all sessions
        let sessions = self.sessions.lock().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();
        let session_ids_str = session_ids.join(",");

        let response = WebRTCMessage {
            client_id: request.client_id,
            session_id: session_ids_str.clone(),
            message_type: LIST_SESSIONS.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };
        response
    }

    async fn handle_offer(&self, request: WebRTCMessage) -> WebRTCMessage {
        // handle offer
        let session_id = request.session_id.clone();
        let publisher_id = request.client_id.clone();
        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.offer = Some(request.sdp.clone().unwrap());

        // send offer to all participants except the creator
        let participants = session.participants.clone();
        let participants = participants.into_iter().filter(|x| x != &publisher_id).collect::<Vec<String>>();

        let offer_msg = WebRTCMessage { // message to be sent to participants
            client_id: publisher_id.clone(),
            session_id: session_id.clone(),
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: session.offer.clone(),
            error: None,
        };

        // send offer to all participants except the creator
        let _ = self.broadcast_message(participants, offer_msg.clone());

        // make a result for publisher
        let response = WebRTCMessage {
            client_id: publisher_id.clone(),
            session_id: session_id.clone(),
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: session.offer.clone(),
            error: None,
        };
        response
    }

    /**
     * Returns a response message for closing a session with remaining session lists.
     */
    async fn close_session(&self, session_id: &str) -> WebRTCMessage{
        self.sessions.lock().await.remove(session_id);

        // get remaining session list
        let sessions = self.sessions.lock().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();

        // print session list
        println!("Remaining sessions: {:?}", session_ids);

        let response = WebRTCMessage {
            client_id: "".to_string(),
            session_id: session_ids.join(","),
            message_type: CLOSE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };
        response
    }

    /**
     * Subscriber joins the session
     */
    async fn join_session(&self, session_id: String, client_id: &str) -> WebRTCMessage{
        let mut sessions = self.sessions.lock().await;
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
            session_id: session_id.clone(),
            message_type: JOIN_SESSION.to_string(),
            ice_candidates: None,
            sdp: Some(sdp),
            error: None,
        };
        response
    }

    async fn leave_session(&self, session_id: String, client_id: &str) -> WebRTCMessage{
        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(&session_id).unwrap();

        session.participants.retain(|x| x != client_id);

        // TODO: remove the WebRTC connection too. (not implemented yet)

        let response = WebRTCMessage {
            client_id: client_id.to_string(),
            session_id: session_id.clone(),
            message_type: LEAVE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };
        response
    }

    async fn list_participants(&self, session_id: &str) -> WebRTCMessage {
        let sessions = self.sessions.lock().await;
        let session = sessions.get(session_id).unwrap();
        let participants_str = session.participants.clone().join(",");
        let response = WebRTCMessage {
            client_id: "".to_string(),
            session_id: session_id.to_string(),
            message_type: LIST_PARTICIPANTS.to_string(),
            ice_candidates: Some(participants_str),
            sdp: None,
            error: None,
        };
        response
    }

    async fn handle_ice_candidate(&self, message: WebRTCMessage) -> WebRTCMessage {
        println!("Handling ICE candidate: {:?}", message);  // temporal log

        // pass ice candidate to the subscriber
        let session_id = message.session_id.clone();
        let sessions = self.sessions.lock().await;
        let mut session = sessions.get(&session_id).unwrap().clone();
        
        let ice_candidates_json = message.ice_candidates.clone().unwrap_or_default();
        let ice_candidates: Vec<String> = serde_json::from_str(&ice_candidates_json).unwrap_or_default();
        // println!("ICE candidates: {:?}", ice_candidates);  // temporal log
        session.publisher_ice_candidates = Some(ice_candidates.clone());

        // collect client ids from the session
        let participants = session.participants.clone();
        let participants = participants.into_iter().filter(|x| x != &message.client_id).collect::<Vec<String>>();

        println!("Target clients: {:?}, {:?}", message.client_id, participants);  // temporal log
        self.broadcast_message(participants, message.clone()).await;        

        // message back to the caller
        let response = WebRTCMessage {
            client_id: message.client_id.clone(),
            session_id: session_id.clone(),
            message_type: ICE_CANDIDATE.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };
        response
    }

    async fn handle_ice_candidate_ack(&self, message: WebRTCMessage) -> WebRTCMessage {
        // pass ice candidate ack to the publisher
        let session_id = message.session_id.clone();
        let sessions = self.sessions.lock().await;
        let session = sessions.get(&session_id).unwrap().clone();

        // collect creator id from the session
        let publisher_id = session.creator_id.clone();
        println!("Publisher id: {}", publisher_id);  // temporal log
        
        // send ice candidate ack to the publisher
        let publisher_msg = WebRTCMessage {
            client_id: publisher_id.clone(),
            session_id: session_id.clone(),
            message_type: ICE_CANDIDATE_ACK.to_string(),
            ice_candidates: message.ice_candidates.clone(),
            sdp: None,
            error: None,
        };

        // get a sender of the publisher
        let clients = self.clients.lock().await;
        let publisher_sender = clients.get(&publisher_id).unwrap().sender.clone();
        let mut publisher_sender = publisher_sender.lock().await;

        println!("Ice candidate to publisher: {:?}", publisher_msg.ice_candidates.clone());  // temporal log
        let msg = Message::text(serde_json::to_string(&publisher_msg).unwrap());
        if let Err(e) = publisher_sender.send(msg).await {
            println!("Error sending ICE candidate ack to publisher: {}", e);
        }

        // message back to the subscriber
        let response = WebRTCMessage {
            client_id: message.client_id.clone(),
            session_id: session_id.clone(),
            message_type: ICE_CANDIDATE_ACK.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };
        response    
    }

    async fn broadcast_message(&self, client_ids: Vec<String>, message: WebRTCMessage) {
        let clients = self.clients.lock().await;
        let senders: Vec<(String, Arc<AsyncMutex<SplitSink<WsStream<TcpStream>, Message>>>)> = client_ids
            .into_iter()
            .filter_map(|client_id| {
                clients.get(&client_id).map(|client| (client_id, Arc::clone(&client.sender)))
            })
            .collect();
        drop(clients);

        for (client_id, sender) in senders {
            let mut sender = sender.lock().await;
            let msg = Message::text(serde_json::to_string(&message).unwrap());
            if let Err(e) = sender.send(msg).await {
                println!("Error sending message to {}: {}", client_id, e);
            }
        }
        // println!("Broadcast message: {:?}", message);  // temporal log
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

