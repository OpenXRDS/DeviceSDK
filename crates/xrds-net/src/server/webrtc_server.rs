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
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::error::Error;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream as WsStream;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::policy::ice_transport_policy::RTCIceTransportPolicy;

use crate::common::data_structure::ICE_CANDIDATE_ACK;
use crate::common::data_structure::{WebRTCMessage, WELCOME};
use crate::common::data_structure::{
    ANSWER, CLOSE_SESSION, CREATE_SESSION, ICE_CANDIDATE, JOIN_SESSION, LEAVE_SESSION,
    LIST_PARTICIPANTS, LIST_SESSIONS, OFFER,
};
use crate::common::generate_uuid;

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
    sessions: Arc<AsyncMutex<HashMap<String, Session>>>,     // <session_id, Session>
    api: Option<webrtc::api::API>,                           // in case of SFU
    rtc_config: Option<RTCConfiguration>,
}

/**
 * This represents a session between two or more clients.
 * like a chat room
 */
#[derive(Clone)]
pub struct Session {
    session_id: String,
    creator_id: String,        // client_id. Only creator can close the session
    participants: Vec<String>, // a vector of client_ids
    offer: Option<String>,     // SDP offer from the creator. Participants will receive this.
    answers: Option<HashMap<String, String>>, // <client_id, SDP answer>
}

type WebSocketSenderType = Vec<(
    String,
    Arc<AsyncMutex<SplitSink<WsStream<TcpStream>, Message>>>,
)>;

#[allow(dead_code)]
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
        server.setup_webrtc().unwrap(); // setup webrtc
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
            ice_servers: crate::webrtc_ice_config::build_ice_servers(),
            ice_transport_policy: RTCIceTransportPolicy::All,
            ..Default::default()
        };

        self.api = Some(api);
        self.rtc_config = Some(rtc_config.clone());

        Ok(())
    }

    async fn add_client(
        &self,
        client_id: &String,
        peer_addr: &String,
        ws_stream: WsStream<TcpStream>,
    ) {
        println!("wait for client lock"); // temporal log
        let mut clients = self.clients.lock().await;
        clients.insert(
            client_id.clone(),
            WebRTCClient::new(client_id.to_string(), peer_addr.to_string(), ws_stream),
        );
        println!("Client {} added", client_id);
        drop(clients); // release the lock
    }

    /// Cleans up everything tracked for a client once its connection ends —
    /// removes it from the client map, and from every session: as a
    /// participant if it just joined others' sessions, or by removing the
    /// whole session if it was the creator (nothing can publish to it
    /// anymore). Called unconditionally once `handle_connection`'s read
    /// loop ends, regardless of *how* it ended (graceful close, a read
    /// error, or the stream just ending) — previously this only ran on an
    /// explicit WS close frame, so a dropped connection (network blip, a
    /// crashed client) leaked its session/participant entries forever. See
    /// docs/done/xrds-net-release-readiness.md Phase 2.
    async fn handle_client_disconnect(&self, client_id: &str) {
        self.clients.lock().await.remove(client_id);

        let mut sessions = self.sessions.lock().await;
        let mut sessions_to_remove = Vec::new();
        for (session_id, session) in sessions.iter_mut() {
            if session.creator_id == client_id {
                sessions_to_remove.push(session_id.clone());
            } else {
                session.participants.retain(|p| p != client_id);
            }
        }
        for session_id in sessions_to_remove {
            sessions.remove(&session_id);
        }
    }

    pub async fn run(self: Arc<Self>, port: u32) -> Result<(), Box<dyn std::error::Error>> {
        self.run_reporting_port(port, None).await
    }

    /// Same as `run`, but if `port_tx` is given, sends the actual bound port
    /// (relevant when `port == 0`, letting the OS assign one) once the
    /// listener is up and before entering the accept loop.
    pub async fn run_reporting_port(
        self: Arc<Self>,
        port: u32,
        port_tx: Option<tokio::sync::oneshot::Sender<u16>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let host_addr = "0.0.0.0".to_owned() + ":" + &port.to_string();
        let try_socket = TcpListener::bind(host_addr.clone()).await;
        let listener = match try_socket {
            Ok(l) => {
                println!("WebRTC Signaling server started on {}", host_addr); // temporal log
                l
            }
            Err(e) => {
                println!("Error binding to {}: {}", host_addr, e);
                return Err(Box::new(e));
            }
        };

        if let Some(tx) = port_tx {
            let _ = tx.send(listener.local_addr()?.port());
        }

        while let Ok((stream, addr)) = listener.accept().await {
            println!("Accepted connection from {}", addr); // temporal log

            let self_clone = Arc::clone(&self);
            tokio::spawn({
                async move {
                    match accept_async(stream).await {
                        Ok(ws_stream) => {
                            // connection established
                            let client_id = generate_uuid(); // generate client's unique id
                            let peer_addr = addr.to_string();
                            // println!("Generated client id: {}", client_id);  // temporal log
                            self_clone
                                .add_client(&client_id.clone(), &peer_addr, ws_stream)
                                .await;
                            self_clone
                                .handle_connection(client_id)
                                .await
                                .unwrap_or_else(|e| {
                                    println!("Error handling connection: {}", e);
                                });
                        }
                        Err(e) => {
                            println!("Error accepting WebSocket connection from {}: {}", addr, e)
                        }
                    }
                }
            });
        }

        Ok(())
    }

    /// Runs the connection's welcome + signaling-message loop, then always
    /// cleans up the client's tracked state — regardless of which of the
    /// inner function's return/break paths was taken. See
    /// `handle_client_disconnect`'s docs for why "always" matters here.
    async fn handle_connection(&self, client_id: String) -> Result<(), Box<dyn std::error::Error>> {
        // `handle_connection_inner`'s error is a plain `String` (rather than
        // `Box<dyn Error>`) so it's `Send` and can be held across the
        // `.await` below — a boxed `dyn Error` is not `Send` in general,
        // which broke `tokio::spawn`'s `Send` requirement on the outer
        // per-connection task.
        let result = self.handle_connection_inner(&client_id).await;
        self.handle_client_disconnect(&client_id).await;
        result.map_err(|e| e.into())
    }

    async fn handle_connection_inner(&self, client_id: &str) -> Result<(), String> {
        let (sender, receiver) = {
            let clients = self.clients.lock().await;
            let Some(client) = clients.get(client_id) else {
                return Err(format!("client {client_id} vanished immediately after connecting"));
            };
            (Arc::clone(&client.sender), Arc::clone(&client.receiver))
        }; // lock on clients is released here

        // Send welcome message with the issued client id
        let welcome_msg = WebRTCMessage {
            client_id: client_id.to_string(),
            session_id: "".to_string(),
            message_type: WELCOME.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };

        let client_id_msg_json = serde_json::to_string(&welcome_msg).unwrap();
        let client_id_msg = Message::text(client_id_msg_json);
        {
            let mut sender_guard = sender.lock().await;
            if let Err(e) = sender_guard.send(client_id_msg).await {
                println!("Error sending welcome message: {}", e);
                return Err(e.to_string());
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
                        let mut sender_guard = sender.lock().await;
                        if let Err(close_err) = sender_guard.send(Message::Close(None)).await {
                            println!("[Server]Failed to send close frame: {}", close_err);
                        }
                        break;
                    }
                };

                if msg.is_close() {
                    println!("[Server]Connection closed by client");
                    break;
                }

                // println!("preparing message back to client");
                let result = self.signaling_handler(msg.into_data().to_vec()).await;
                // prepare message back to client
                if let Some(result) = result {
                    // send message in text since it's json only
                    let msg = Message::text(String::from_utf8_lossy(&result).to_string());
                    let mut sender_guard = sender.lock().await;
                    if let Err(e) = sender_guard.send(msg).await {
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
        let raw = String::from_utf8_lossy(&input);
        // println!("Received message: {}", msg);  // temporal log
        let msg: WebRTCMessage = match serde_json::from_str(raw.as_ref()) {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("Dropping malformed signaling message from client: {e}");
                return None;
            }
        };
        let message_type = msg.clone().message_type;

        // handle message by matching message_types
        match message_type.as_str() {
            CREATE_SESSION => {
                let response = self.handle_create_session(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            }
            LIST_SESSIONS => {
                let response = self.handle_list_session(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            }
            CLOSE_SESSION | JOIN_SESSION | LEAVE_SESSION | LIST_PARTICIPANTS | ICE_CANDIDATE
            | ICE_CANDIDATE_ACK => {
                let session_id = msg.session_id.clone();

                // print session id
                println!("signalhandling.Session ID: {}", session_id); // temporal log

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
            }
            OFFER => {
                let response = self.handle_offer(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            }
            ANSWER => {
                let response = self.handle_answer(msg).await;
                Some(serde_json::to_string(&response).unwrap().into_bytes())
            }
            _ => None, // unknown message type
        }
    }

    /****************** Message Handler Functions **************** */

    /// Builds an error response of the given message type — used instead of
    /// panicking when a client references an unknown/stale session or client
    /// id, or omits a required field. See docs/done/xrds-net-release-readiness.md
    /// Phase 2: these lookups used to `.unwrap()` on client-supplied ids,
    /// which let one malformed or out-of-order message from a single client
    /// crash the whole signaling task.
    fn error_response(
        client_id: &str,
        session_id: &str,
        message_type: &str,
        error: impl Into<String>,
    ) -> WebRTCMessage {
        WebRTCMessage {
            client_id: client_id.to_string(),
            session_id: session_id.to_string(),
            message_type: message_type.to_string(),
            ice_candidates: None,
            sdp: None,
            error: Some(error.into()),
        }
    }

    /**
     * This function handles the answer message from the subscriber.
     * It updates the session with the answer
     */
    async fn handle_answer(&self, request: WebRTCMessage) -> WebRTCMessage {
        // println!("Answer {:?}", request);  // temporal log

        let session_id = request.session_id.clone();
        let subscriber_id = request.client_id.clone();

        let Some(sdp) = request.sdp.clone() else {
            return Self::error_response(
                &subscriber_id,
                &session_id,
                ANSWER,
                "answer message missing sdp",
            );
        };

        let publisher_id = {
            let mut sessions = self.sessions.lock().await;
            let Some(session) = sessions.get_mut(&session_id) else {
                return Self::error_response(
                    &subscriber_id,
                    &session_id,
                    ANSWER,
                    format!("unknown session_id: {session_id}"),
                );
            };

            if session.answers.is_none() {
                session.answers = Some(HashMap::new());
            }
            // Store the corresponding answer in the session for each subscriber
            session
                .answers
                .as_mut()
                .unwrap()
                .insert(subscriber_id.clone(), sdp.clone());
            println!(
                "Number of answers in session {}: {}",
                session_id,
                session.answers.as_ref().unwrap().len()
            );

            session.creator_id.clone()
        };

        // send answer to the publisher
        println!("publisher id: {}", publisher_id); // temporal log
        let publisher_msg = WebRTCMessage {
            client_id: publisher_id.clone(),
            session_id: session_id.clone(),
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp: Some(sdp),
            error: None,
        };

        self.broadcast_message(vec![publisher_id], publisher_msg)
            .await;

        // response to the subscriber
        WebRTCMessage {
            client_id: subscriber_id,
            session_id,
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    async fn handle_create_session(&self, request: WebRTCMessage) -> WebRTCMessage {
        // create a new session
        let session_id = generate_uuid();
        let session = Session {
            session_id: session_id.clone(),
            creator_id: request.client_id.clone(),
            participants: vec![request.client_id.clone()],
            offer: None,
            answers: None,
        };

        self.sessions
            .lock()
            .await
            .insert(session_id.clone(), session);

        println!("Session {} created by {}", session_id, request.client_id);

        WebRTCMessage {
            client_id: request.client_id,
            session_id: session_id.clone(),
            message_type: CREATE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    async fn handle_list_session(&self, request: WebRTCMessage) -> WebRTCMessage {
        // list all sessions
        let sessions = self.sessions.lock().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();
        let session_ids_str = session_ids.join(",");

        WebRTCMessage {
            client_id: request.client_id,
            session_id: session_ids_str.clone(),
            message_type: LIST_SESSIONS.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    async fn handle_offer(&self, request: WebRTCMessage) -> WebRTCMessage {
        // handle offer
        let session_id = request.session_id.clone();
        let publisher_id = request.client_id.clone();

        let Some(sdp) = request.sdp.clone() else {
            return Self::error_response(
                &publisher_id,
                &session_id,
                OFFER,
                "offer message missing sdp",
            );
        };

        let participants = {
            let mut sessions = self.sessions.lock().await;
            let Some(session) = sessions.get_mut(&session_id) else {
                return Self::error_response(
                    &publisher_id,
                    &session_id,
                    OFFER,
                    format!("unknown session_id: {session_id}"),
                );
            };
            session.offer = Some(sdp.clone());

            // participants to send the offer to, excluding the creator
            session
                .participants
                .iter()
                .filter(|x| *x != &publisher_id)
                .cloned()
                .collect::<Vec<String>>()
        };

        let offer_msg = WebRTCMessage {
            // message to be sent to participants
            client_id: publisher_id.clone(),
            session_id: session_id.clone(),
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: Some(sdp.clone()),
            error: None,
        };

        // send offer to all participants except the creator
        self.broadcast_message(participants, offer_msg).await;

        // make a result for publisher
        WebRTCMessage {
            client_id: publisher_id,
            session_id,
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: Some(sdp),
            error: None,
        }
    }

    /**
     * Returns a response message for closing a session with remaining session lists.
     */
    async fn close_session(&self, session_id: &str) -> WebRTCMessage {
        self.sessions.lock().await.remove(session_id);

        // get remaining session list
        let sessions = self.sessions.lock().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();

        // print session list
        println!("Remaining sessions: {:?}", session_ids);

        WebRTCMessage {
            client_id: "".to_string(),
            session_id: session_ids.join(","),
            message_type: CLOSE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    /**
     * Subscriber joins the session. Note: the server tracks *bookkeeping*
     * only (participant lists, offer/answer relay) — it never holds a real
     * `RTCPeerConnection` (those live entirely client-side; see
     * `WebRTCClient::setup_webrtc`), so there's no server-side peer
     * connection object to reset here.
     */
    async fn join_session(&self, session_id: String, client_id: &str) -> WebRTCMessage {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.get_mut(&session_id) else {
            return Self::error_response(
                client_id,
                &session_id,
                JOIN_SESSION,
                format!("unknown session_id: {session_id}"),
            );
        };
        if session.participants.contains(&client_id.to_string()) {
            // Re-joining: reset bookkeeping for this client rather than
            // duplicating it in the participant list.
            session.participants.retain(|x| x != client_id);
        }

        session.participants.push(client_id.to_string());
        log::debug!(
            "Client {} joined session {}",
            client_id,
            session.session_id.clone()
        );

        // if sdp exists, send it to the client
        let sdp = session.offer.clone().unwrap_or_default();

        WebRTCMessage {
            client_id: client_id.to_string(),
            session_id: session_id.clone(),
            message_type: JOIN_SESSION.to_string(),
            ice_candidates: None,
            sdp: Some(sdp),
            error: None,
        }
    }

    async fn leave_session(&self, session_id: String, client_id: &str) -> WebRTCMessage {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.get_mut(&session_id) else {
            return Self::error_response(
                client_id,
                &session_id,
                LEAVE_SESSION,
                format!("unknown session_id: {session_id}"),
            );
        };

        session.participants.retain(|x| x != client_id);

        WebRTCMessage {
            client_id: client_id.to_string(),
            session_id: session_id.clone(),
            message_type: LEAVE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    async fn list_participants(&self, session_id: &str) -> WebRTCMessage {
        let sessions = self.sessions.lock().await;
        let Some(session) = sessions.get(session_id) else {
            return Self::error_response(
                "",
                session_id,
                LIST_PARTICIPANTS,
                format!("unknown session_id: {session_id}"),
            );
        };
        let participants_str = session.participants.join(",");
        WebRTCMessage {
            client_id: "".to_string(),
            session_id: session_id.to_string(),
            message_type: LIST_PARTICIPANTS.to_string(),
            ice_candidates: Some(participants_str),
            sdp: None,
            error: None,
        }
    }

    async fn handle_ice_candidate(&self, message: WebRTCMessage) -> WebRTCMessage {
        println!("Handling ICE candidate: {:?}", message); // temporal log

        // pass ice candidate to the subscriber
        let session_id = message.session_id.clone();

        let participants = {
            let sessions = self.sessions.lock().await;
            let Some(session) = sessions.get(&session_id) else {
                return Self::error_response(
                    &message.client_id,
                    &session_id,
                    ICE_CANDIDATE,
                    format!("unknown session_id: {session_id}"),
                );
            };
            // collect client ids from the session, excluding the sender
            session
                .participants
                .iter()
                .filter(|x| *x != &message.client_id)
                .cloned()
                .collect::<Vec<String>>()
        };

        println!(
            "Target clients: {:?}, {:?}",
            message.client_id, participants
        ); // temporal log
        self.broadcast_message(participants, message.clone()).await;

        // message back to the caller
        WebRTCMessage {
            client_id: message.client_id.clone(),
            session_id,
            message_type: ICE_CANDIDATE.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    async fn handle_ice_candidate_ack(&self, message: WebRTCMessage) -> WebRTCMessage {
        // pass ice candidate ack to the publisher
        let session_id = message.session_id.clone();

        let publisher_id = {
            let sessions = self.sessions.lock().await;
            let Some(session) = sessions.get(&session_id) else {
                return Self::error_response(
                    &message.client_id,
                    &session_id,
                    ICE_CANDIDATE_ACK,
                    format!("unknown session_id: {session_id}"),
                );
            };
            session.creator_id.clone()
        };
        println!("Publisher id: {}", publisher_id); // temporal log

        // send ice candidate ack to the publisher
        let publisher_msg = WebRTCMessage {
            client_id: publisher_id.clone(),
            session_id: session_id.clone(),
            message_type: ICE_CANDIDATE_ACK.to_string(),
            ice_candidates: message.ice_candidates.clone(),
            sdp: None,
            error: None,
        };

        // get a sender of the publisher, if it's still connected — it may
        // have disconnected already (e.g. this ack raced its own
        // disconnect), which is not a crash, just nothing to deliver to.
        let publisher_sender = {
            let clients = self.clients.lock().await;
            clients.get(&publisher_id).map(|c| c.sender.clone())
        };

        match publisher_sender {
            Some(publisher_sender) => {
                let mut publisher_sender = publisher_sender.lock().await;
                println!(
                    "Ice candidate to publisher: {:?}",
                    publisher_msg.ice_candidates
                ); // temporal log
                let msg = Message::text(serde_json::to_string(&publisher_msg).unwrap());
                if let Err(e) = publisher_sender.send(msg).await {
                    println!("Error sending ICE candidate ack to publisher: {}", e);
                }
            }
            None => {
                println!(
                    "Publisher {publisher_id} is no longer connected — dropping ICE candidate ack"
                );
            }
        }

        // message back to the subscriber
        WebRTCMessage {
            client_id: message.client_id.clone(),
            session_id,
            message_type: ICE_CANDIDATE_ACK.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    async fn broadcast_message(&self, client_ids: Vec<String>, message: WebRTCMessage) {
        let clients = self.clients.lock().await;
        let senders: WebSocketSenderType = client_ids
            .into_iter()
            .filter_map(|client_id| {
                clients
                    .get(&client_id)
                    .map(|client| (client_id, Arc::clone(&client.sender)))
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
        Error::Io(io_err) => match io_err.kind() {
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
        },
        Error::Protocol(proto_err) => {
            println!("Protocol violation by client: {}", proto_err);
        }
        _ => {
            println!("Other WebSocket error: {}", error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::payload_str_to_vector_str;

    fn create_request(client_id: &str) -> WebRTCMessage {
        WebRTCMessage {
            client_id: client_id.to_string(),
            session_id: String::new(),
            message_type: CREATE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        }
    }

    // ICE server config (STUN/TURN URL scheme) is now unit-tested once, at
    // its single source of truth — see
    // crate::webrtc_ice_config::tests::build_ice_servers_uses_turns_scheme_for_the_tls_secured_port
    // — rather than duplicated here against this module's copy.

    #[tokio::test]
    async fn create_session_registers_creator_as_sole_participant() {
        let server = WebRTCServer::new();
        let response = server.handle_create_session(create_request("client-1")).await;

        assert_eq!(response.message_type, CREATE_SESSION);
        assert!(uuid::Uuid::parse_str(&response.session_id).is_ok());

        let participants = server.list_participants(&response.session_id).await;
        assert_eq!(participants.ice_candidates.unwrap(), "client-1");
    }

    #[tokio::test]
    async fn join_session_adds_a_new_participant() {
        let server = WebRTCServer::new();
        let session_id = server
            .handle_create_session(create_request("creator"))
            .await
            .session_id;

        server.join_session(session_id.clone(), "subscriber").await;

        let participants = server.list_participants(&session_id).await;
        let list = payload_str_to_vector_str(&participants.ice_candidates.unwrap());
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"creator".to_string()));
        assert!(list.contains(&"subscriber".to_string()));
    }

    #[tokio::test]
    async fn joining_twice_resets_rather_than_duplicates_the_participant() {
        let server = WebRTCServer::new();
        let session_id = server
            .handle_create_session(create_request("creator"))
            .await
            .session_id;

        server.join_session(session_id.clone(), "subscriber").await;
        server.join_session(session_id.clone(), "subscriber").await;

        let participants = server.list_participants(&session_id).await;
        let list = payload_str_to_vector_str(&participants.ice_candidates.unwrap());
        assert_eq!(
            list.iter().filter(|c| *c == "subscriber").count(),
            1,
            "joining the same session twice should not duplicate the participant: {list:?}"
        );
    }

    #[tokio::test]
    async fn leave_session_removes_the_participant() {
        let server = WebRTCServer::new();
        let session_id = server
            .handle_create_session(create_request("creator"))
            .await
            .session_id;
        server.join_session(session_id.clone(), "subscriber").await;

        server.leave_session(session_id.clone(), "subscriber").await;

        let participants = server.list_participants(&session_id).await;
        let list = payload_str_to_vector_str(&participants.ice_candidates.unwrap());
        assert_eq!(list, vec!["creator".to_string()]);
    }

    #[tokio::test]
    async fn close_session_removes_it_from_the_session_list() {
        let server = WebRTCServer::new();
        let session_id = server
            .handle_create_session(create_request("creator"))
            .await
            .session_id;

        let response = server.close_session(&session_id).await;
        let remaining = payload_str_to_vector_str(&response.session_id);
        assert!(
            !remaining.contains(&session_id),
            "closed session {session_id} should not appear in the remaining list: {remaining:?}"
        );
    }

    #[tokio::test]
    async fn list_session_reflects_every_created_session() {
        let server = WebRTCServer::new();
        let s1 = server
            .handle_create_session(create_request("a"))
            .await
            .session_id;
        let s2 = server
            .handle_create_session(create_request("b"))
            .await
            .session_id;

        let listed = server.handle_list_session(create_request("anyone")).await;
        let ids = payload_str_to_vector_str(&listed.session_id);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&s1));
        assert!(ids.contains(&s2));
    }

    // These four exercise the crash-risk fix from
    // docs/done/xrds-net-release-readiness.md Phase 2: every handler taking a
    // client-supplied session_id used to `.unwrap()` the lookup, so an
    // unknown/stale id panicked the whole signaling task instead of
    // returning an error to that one client.

    #[tokio::test]
    async fn join_session_with_unknown_id_returns_error_instead_of_panicking() {
        let server = WebRTCServer::new();
        let response = server
            .join_session("does-not-exist".to_string(), "client-1")
            .await;
        assert!(response.error.is_some());
    }

    #[tokio::test]
    async fn leave_session_with_unknown_id_returns_error_instead_of_panicking() {
        let server = WebRTCServer::new();
        let response = server
            .leave_session("does-not-exist".to_string(), "client-1")
            .await;
        assert!(response.error.is_some());
    }

    #[tokio::test]
    async fn list_participants_with_unknown_id_returns_error_instead_of_panicking() {
        let server = WebRTCServer::new();
        let response = server.list_participants("does-not-exist").await;
        assert!(response.error.is_some());
    }

    #[tokio::test]
    async fn close_session_with_unknown_id_is_a_harmless_no_op() {
        // `close_session` already used `HashMap::remove` (never panics on a
        // missing key), unlike the other lookups — covered here mainly to
        // document that it was already safe, not to fix a bug.
        let server = WebRTCServer::new();
        let response = server.close_session("does-not-exist").await;
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn offer_and_answer_with_unknown_session_id_return_errors() {
        let server = WebRTCServer::new();

        let offer_request = WebRTCMessage {
            client_id: "publisher".to_string(),
            session_id: "does-not-exist".to_string(),
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: Some("v=0".to_string()),
            error: None,
        };
        assert!(server.handle_offer(offer_request).await.error.is_some());

        let answer_request = WebRTCMessage {
            client_id: "subscriber".to_string(),
            session_id: "does-not-exist".to_string(),
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp: Some("v=0".to_string()),
            error: None,
        };
        assert!(server.handle_answer(answer_request).await.error.is_some());
    }

    #[tokio::test]
    async fn offer_missing_sdp_returns_error_instead_of_panicking() {
        let server = WebRTCServer::new();
        let session_id = server
            .handle_create_session(create_request("publisher"))
            .await
            .session_id;

        let offer_request = WebRTCMessage {
            client_id: "publisher".to_string(),
            session_id,
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp: None, // missing — used to panic on request.sdp.clone().unwrap()
            error: None,
        };
        assert!(server.handle_offer(offer_request).await.error.is_some());
    }

    // Disconnect cleanup (the resource-leak fix, same Phase 2 finding):
    // previously only ran on an explicit WS close frame, and even then
    // never touched session/participant bookkeeping (the "TODO: remove
    // the session if the client is the creator" comment). Tested directly
    // against `handle_client_disconnect` — no real WebSocket needed, since
    // it's plain bookkeeping over the same maps the tests above exercise.

    #[tokio::test]
    async fn disconnect_removes_a_session_the_client_created() {
        let server = WebRTCServer::new();
        let session_id = server
            .handle_create_session(create_request("creator"))
            .await
            .session_id;

        server.handle_client_disconnect("creator").await;

        let listed = server.handle_list_session(create_request("anyone")).await;
        let ids = payload_str_to_vector_str(&listed.session_id);
        assert!(
            !ids.contains(&session_id),
            "session created by a now-disconnected client should be removed: {ids:?}"
        );
    }

    #[tokio::test]
    async fn disconnect_removes_the_client_from_sessions_it_only_joined() {
        let server = WebRTCServer::new();
        let session_id = server
            .handle_create_session(create_request("creator"))
            .await
            .session_id;
        server.join_session(session_id.clone(), "subscriber").await;

        server.handle_client_disconnect("subscriber").await;

        let participants = server.list_participants(&session_id).await;
        let list = payload_str_to_vector_str(&participants.ice_candidates.unwrap());
        assert_eq!(
            list,
            vec!["creator".to_string()],
            "disconnected participant should be gone, session (created by someone else) should survive"
        );
    }
}
