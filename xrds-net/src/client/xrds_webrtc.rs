use coap::client;
use futures::{FutureExt, TryFutureExt};

use tokio::io::AsyncReadExt;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use tokio::sync::mpsc::{Sender, Receiver};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::{peer_connection::RTCPeerConnection, rtp_transceiver::rtp_codec::RTCRtpCodecCapability};
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::interceptor::registry::Registry;
use webrtc::api::APIBuilder;
use tokio::runtime::Runtime;

use tokio_tungstenite::WebSocketStream as WsStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use serde::{Serialize, Deserialize};
use std::error::Error;
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::common::data_structure::{NetResponse, WebRTCMessage};
use crate::common::generate_random_string;
use crate::common::data_structure::{CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER};

pub struct WebRTCClient {
    client_id: Option<String>,
    write: Option<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    incoming_rx: Option<mpsc::Receiver<WebRTCMessage>>,
    run_handle: Option<tokio::task::JoinHandle<()>>,
    session_id: Option<String>,
}

#[allow(dead_code)]
impl WebRTCClient {
    pub fn new() -> Self {
        Self {
            client_id: None,
            write: None,
            incoming_rx: None,
            run_handle: None,
            session_id: None,
        }
    }
    
    pub fn get_client_id(&self) -> Option<&String> {
        self.client_id.as_ref()
    }

    pub fn get_session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }

    pub async fn connect(&mut self, addr: &str) -> Result<(), Box<dyn Error>> {
        let (ws_stream, _) = connect_async(addr).await?;
        println!("Connected to {}", addr);

        let (write, mut read) = ws_stream.split();
        self.write = Some(write);

        let (tx, rx) = mpsc::channel::<WebRTCMessage>(100);
        self.incoming_rx = Some(rx);

        let run_handle = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        let msg: WebRTCMessage = serde_json::from_str(text.as_ref()).unwrap();

                        if tx.send(msg).await.is_err() {
                            println!("Receiver dropped, stopping run task");
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        println!("Connection closed by server");
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            println!("WebSocket run terminated");
        });

        self.run_handle = Some(run_handle);

        Ok(())
    }

    pub async fn receive_message(&mut self) -> Option<WebRTCMessage> {
        if let Some(ref mut rx) = self.incoming_rx {
            if let Some(msg) = rx.recv().await {
                if msg.message_type == "WELCOME" {
                    self.client_id = Some(msg.client_id.clone());
                } else if msg.message_type == CREATE_SESSION {
                    self.session_id = Some(String::from_utf8(msg.payload.clone()).unwrap());
                }
                return Some(msg);
            }
        }
        None
    }

    pub async fn create_session(mut self) -> Result<Self, Box<dyn Error>> {
        let client_id = self.client_id.as_ref().ok_or("client_id not set")?;
        
        let msg = WebRTCMessage {
            client_id: client_id.clone(),
            message_type: CREATE_SESSION.to_string(),
            payload: Vec::new(),
            sdp: None,
            error: None,
        };

        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        self.write.as_mut().ok_or("WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await?;

        Ok(self)
    }

    pub async fn send_message(&mut self, message: &str) -> Result<(), Box<dyn Error>> {
        if let Some(ref mut write) = self.write {
            write.send(Message::Text(message.to_string().into())).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }
        Ok(())
    }

    pub async fn close_connection(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(ref mut write) = self.write {
            write.send(Message::Close(None)).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        if let Some(handle) = self.run_handle.take() {
            handle.await?;
        }
        println!("WebRTCClient connection closed");

        Ok(())
    }

    pub async fn list_sessions(mut self) -> Result<Self, Box<dyn Error>> {
        let client_id = self.client_id.as_ref().ok_or("client_id not set")?;

        let msg = WebRTCMessage {
            client_id: client_id.clone(),
            message_type: LIST_SESSIONS.to_string(),
            payload: Vec::new(),
            sdp: None,
            error: None,
        };

        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        self.write.as_mut().ok_or("WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await?;

        Ok(self)
    }

    pub async fn close_session(&mut self, session_id: &str) -> Result<(), Box<dyn Error>> {
        // if self.client_id.is_none() {
        //     return Err("Client ID is not set".into());
        // }

        // let msg = WebRTCMessage {
        //     client_id: self.client_id.clone().unwrap(),
        //     message_type: CLOSE_SESSION.to_string(),
        //     payload: session_id.as_bytes().to_vec(),
        //     sdp: None,
        //     error: None,
        // };

        // let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        // self.send_msg(msg.as_str()).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn join_session(&mut self, session_id: &str) -> Result<(), Box<dyn Error>> {
        // if self.client_id.is_none() {
        //     return Err("Client ID is not set".into());
        // }

        // let msg = WebRTCMessage {
        //     client_id: self.client_id.clone().unwrap(),
        //     message_type: JOIN_SESSION.to_string(),
        //     payload: session_id.as_bytes().to_vec(),
        //     sdp: None,
        //     error: None,
        // };

        // // serialize msg into json
        // let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        // self.send_msg(msg.as_str()).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn leave_session(&mut self, session_id: &str) -> Result<(), Box<dyn Error>> {
        // if self.client_id.is_none() {
        //     return Err("Client ID is not set".into());
        // }

        // let msg = WebRTCMessage {
        //     client_id: self.client_id.clone().unwrap(),
        //     message_type: LEAVE_SESSION.to_string(),
        //     payload: session_id.as_bytes().to_vec(),
        //     sdp: None,
        //     error: None,
        // };

        // // serialize msg into json
        // let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        // self.send_msg(msg.as_str()).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn list_participants(&mut self, session_id: &str) -> Result<(), Box<dyn Error>> {
        // if self.client_id.is_none() {
        //     return Err("Client ID is not set".into());
        // }

        // let msg = WebRTCMessage {
        //     client_id: self.client_id.clone().unwrap(),
        //     message_type: LIST_PARTICIPANTS.to_string(),
        //     payload: session_id.as_bytes().to_vec(),
        //     sdp: None,
        //     error: None,
        // };

        // // serialize msg into json
        // let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        // self.send_msg(msg.as_str()).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    
}

impl Drop for WebRTCClient {
    fn drop(&mut self) {
        if let Some(handle) = self.run_handle.take() {
            handle.abort();
        }
        println!("WebRTCClient dropped");
    }
}

pub struct XrdsWebRTCPublisher {
    client: WebRTCClient,
    pc: Option<RTCPeerConnection>,
    api: Option<webrtc::api::API>,
    rtc_config: Option<RTCConfiguration>,
    offer: Option<RTCSessionDescription>,
}

impl XrdsWebRTCPublisher {
    pub fn new(client: WebRTCClient) -> Self {
        let mut publisher = XrdsWebRTCPublisher{
            client,
            pc: None,
            api: None,
            rtc_config: None,
            offer: None,
        };
        publisher.setup_api().unwrap();
        publisher
    }

    /**
     * Sends OFFER message with SDP to the signaling server
     */
    pub fn publish(&self, session_id: &str) -> Result<(), String> {
        // if self.client.client_id.is_none() {
        //     return Err("Client ID is not set".to_string());
        // }        

        // if self.offer.is_none() {
        //     return Err("Offer is not set".to_string());
        // }

        // let sdp = Some(self.offer.as_ref().unwrap().sdp.clone());
        // let msg = WebRTCMessage {
        //     client_id: self.client.client_id.clone().unwrap(),
        //     message_type: OFFER.to_string(),
        //     payload: session_id.as_bytes().to_vec(),
        //     sdp,
        //     error: None,
        // };

        // let ws_client = self.client.ws_client.as_ref().unwrap();

        // // serialize msg into json
        // let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        // ws_client.send_ws(Some("text"), msg.as_bytes().to_vec()).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn create_offer(&mut self) -> Result<(), String> {
        let api = match &self.api {
            Some(api) => api,
            None => return Err("API is not set".to_string()),
        };
    
        let rtc_config = match &self.rtc_config {
            Some(config) => config.clone(),
            None => return Err("RTC config is not set".to_string()),
        };

        let pc = Runtime::new().unwrap().block_on(api.new_peer_connection(rtc_config))
            .map_err(|e| e.to_string())?;
        
        
        self.pc = Some(pc);
        
        let offer = Runtime::new().unwrap().block_on(self.pc.as_ref().unwrap().create_offer(None))
            .map_err(|e| e.to_string())?;

        let _ = Runtime::new().unwrap().block_on(self.pc.as_ref().unwrap().set_local_description(offer.clone()))
            .map_err(|e| e.to_string())?;
        self.offer = Some(offer.clone());
        Ok(())
    }

    fn setup_api(&mut self) -> Result<(), String> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs().map_err(|e| e.to_string())?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine).map_err(|e| e.to_string())?;

        let config = create_default_webrtc_config();
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        self.rtc_config = Some(config);
        self.api = Some(api);

        Ok(())
    }
}

pub struct XrdsWebRTCSubscriber {
    client: WebRTCClient,
    pc: Option<RTCPeerConnection>,
    api: Option<webrtc::api::API>,
    rtc_config: Option<RTCConfiguration>,
    answer: Option<RTCSessionDescription>,
}

impl XrdsWebRTCSubscriber {
    pub fn new(client: WebRTCClient) -> Self {
        let mut subscriber = XrdsWebRTCSubscriber{
            client,
            pc: None,
            api: None,
            rtc_config: None,
            answer: None,
        };
        subscriber.setup_api().unwrap();
        subscriber
    }

    fn setup_api(&mut self) -> Result<(), String> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs().map_err(|e| e.to_string())?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine).map_err(|e| e.to_string())?;

        let config = create_default_webrtc_config();
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        self.rtc_config = Some(config);
        self.api = Some(api);

        Ok(())
    }

    pub fn create_answer(&mut self, offer: RTCSessionDescription) -> Result<(), String> {
        let api = match &self.api {
            Some(api) => api,
            None => return Err("API is not set".to_string()),
        };
    
        let rtc_config = match &self.rtc_config {
            Some(config) => config.clone(),
            None => return Err("RTC config is not set".to_string()),
        };

        let pc = Runtime::new().unwrap().block_on(api.new_peer_connection(rtc_config))
            .map_err(|e| e.to_string())?;
        
        self.pc = Some(pc);
        
        let answer = Runtime::new().unwrap().block_on(self.pc.as_ref().unwrap().create_answer(None))
            .map_err(|e| e.to_string())?;

        let _ = Runtime::new().unwrap().block_on(self.pc.as_ref().unwrap().set_local_description(answer.clone()))
            .map_err(|e| e.to_string())?;
        self.answer = Some(answer.clone());
        Ok(())
    }


}

fn create_default_webrtc_config() -> RTCConfiguration {
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };
    config
}