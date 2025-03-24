use coap::client;
use futures::io::BufReader;
use futures::{FutureExt, TryFutureExt};

use tokio::io::AsyncReadExt;
use webrtc::media::io::h264_reader::H264Reader;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use tokio::sync::mpsc::{Sender, Receiver};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::{peer_connection::RTCPeerConnection, rtp_transceiver::rtp_codec::RTCRtpCodecCapability};
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::interceptor::registry::Registry;
use webrtc::api::APIBuilder;
use std::fs::File;
use webrtc::media::Sample;
use tokio::runtime::Runtime;
use webrtc::peer_connection::offer_answer_options::RTCOfferOptions;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_tungstenite::WebSocketStream as WsStream;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use tokio_tungstenite::MaybeTlsStream;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use std::error::Error;
use tokio::sync::mpsc;

use crate::common::data_structure::{NetResponse, WebRTCMessage};
use crate::common::data_structure::{CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, 
        LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER, WELCOME};

pub struct WebRTCClient {
    client_id: Option<String>,
    write: Option<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    incoming_rx: Option<mpsc::Receiver<WebRTCMessage>>,
    run_handle: Option<tokio::task::JoinHandle<()>>,
    session_id: Option<String>,

    // WebRTC specific fields
    pc: Option<RTCPeerConnection>,
    api: Option<webrtc::api::API>,
    rtc_config: Option<RTCConfiguration>,
    offer: Option<RTCSessionDescription>,
    answer: Option<RTCSessionDescription>,
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

            pc: None,
            api: None,
            rtc_config: None,
            offer: None,
            answer: None,
        }
    }
    
    pub fn get_client_id(&self) -> Option<&String> {
        self.client_id.as_ref()
    }

    pub fn get_session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }

    pub fn get_answer(&self) -> Option<&RTCSessionDescription> {
        self.answer.as_ref()
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
                if msg.message_type == WELCOME {
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

    pub async fn close_session(mut self, session_id: &str) -> Result<Self, Box<dyn Error>> {
        if self.client_id.is_none() {
            return Err("[close_session]Client ID is not set".into());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: CLOSE_SESSION.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        self.write.as_mut().ok_or("[close_session]WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await?;

        Ok(self)
    }

    pub async fn join_session(mut self, session_id: &str) -> Result<Self, Box<dyn Error>> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".into());
        }

        self.session_id = Some(session_id.to_string());

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: JOIN_SESSION.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        println!("Joining session: {}", session_id);

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        println!("Sending message: {}", msg);
        self.write.as_mut().ok_or("WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await?;

        Ok(self)
    }

    pub async fn leave_session(mut self, session_id: &str) -> Result<Self, Box<dyn Error>> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".into());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: LEAVE_SESSION.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        self.write.as_mut().ok_or("WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await?;

        Ok(self)
    }

    pub async fn list_participants(mut self, session_id: &str) -> Result<Self, Box<dyn Error>> {

        let msg = WebRTCMessage {
            client_id: "".to_string(),
            message_type: LIST_PARTICIPANTS.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        // // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        println!("Sending message: {}", msg);

        self.write.as_mut().ok_or("WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await?;

        Ok(self)
    }

    /* ****************************************** */
    /* WebRTC specific methods */
    /* ****************************************** */
    pub async fn start_streaming(&mut self, sample_file: Option<&str>) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        if (sample_file.is_none()) {
            // stream from camera
        } else {
            // stream from file
        }

        Ok(())
    }

    async fn stream_from_camera(&mut self) -> Result<(), String> {
        Ok(())
    }

    async fn stream_from_file(&mut self, sample_file: &str) -> Result<(), String> {
        Ok(())
    }

    /**
     * Create an offer and sent it to the server.
     */
    pub async fn publish(&mut self, session_id: &str, sample_file: Option<&str>) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        // track must be setup before creating offer
        self.create_offer(sample_file).await.map_err(|e| e.to_string())?;

        let sdp = Some(self.offer.as_ref().unwrap().sdp.clone());
        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: OFFER.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp,
            error: None,
        };

        // // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        self.write.as_mut().ok_or("WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn setup_webrtc(&mut self) -> Result<(), String> {
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
            ..Default::default()
        };

        self.api = Some(api);
        self.rtc_config = Some(rtc_config.clone());
        let pc = self.api.as_ref().unwrap().new_peer_connection(rtc_config)
            .await.map_err(|e| e.to_string())?;
        self.pc = Some(pc);

        Ok(())
    }

    pub async fn test_offer_creation(&mut self) -> Result<(), String> {
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
            ..Default::default()
        };

        let video_file = Some("D:/samples/tsm_1080p.mp4");

        let pc = api.new_peer_connection(rtc_config)
            .await.map_err(|e| e.to_string())?;

        if let Some(video_file) = video_file {
            // Create a video track
            let video_track = Arc::new(TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    ..Default::default()
                },
                "video".to_owned(),
                "webrtc-rs".to_owned(),
            ));
    
            // Add this newly created track to the PeerConnection
            let rtp_sender = pc
                .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
                .await;
        }


        
        pc.on_ice_candidate(Box::new(|candidate| {
            Box::pin(async move {
                if let Some(cand) = candidate {
                    println!("ICE Candidate: {:?}", cand);
                } else {
                    println!("ICE gathering completed");
                }
            })
        }));

        pc.on_ice_connection_state_change(Box::new(|state| {
            Box::pin(async move {
                println!("ICE Connection State: {:?}", state);
            })
        }));


        let offer = pc.create_offer(None).await.map_err(|e| e.to_string())?;
        pc.set_local_description(offer.clone()).await.map_err(|e| e.to_string())?;

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let final_offer = pc.local_description().await.unwrap();
        println!("Final offer: {:?}", final_offer.sdp);

        Ok(())
    }

    pub async fn create_offer(&mut self, file_name: Option<&str>) -> Result<(), String> {
        self.setup_webrtc().await.map_err(|e| e.to_string())?;
        
        let api = match &self.api {
            Some(api) => api,
            None => return Err("API is not set".to_string()),
        };
    
        let rtc_config = match &self.rtc_config {
            Some(config) => config.clone(),
            None => return Err("RTC config is not set".to_string()),
        };

        // Connection to the server from the publisher
        let pc = api.new_peer_connection(rtc_config).await.map_err(|e| e.to_string())?;
        self.pc = Some(pc);

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        let audio_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                ..Default::default()
            },
            "audio".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        // Add this newly created track to the PeerConnection
        let _ = self.pc.as_ref().unwrap().add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>).await.map_err(|e| e.to_string())?;
        let _ = self.pc.as_ref().unwrap().add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>).await.map_err(|e| e.to_string())?;
        
        self.pc.as_ref().unwrap().on_ice_candidate(Box::new(|candidate| {
            Box::pin(async move {
                if let Some(cand) = candidate {
                    println!("ICE Candidate: {:?}", cand);
                } else {
                    println!("ICE gathering completed");
                }
            })
        }));

        self.pc.as_ref().unwrap().on_ice_connection_state_change(Box::new(|state| {
            Box::pin(async move {
                println!("ICE Connection State: {:?}", state);
            })
        }));

        let offer = self.pc.as_ref().unwrap().create_offer(None).await
            .map_err(|e| e.to_string())?;
        self.pc.as_ref().unwrap().set_local_description(offer.clone()).await.map_err(|e| e.to_string())?;

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        // println!("Created offer: {:?}", self.pc.as_ref().unwrap().local_description().await.unwrap());
        self.offer = Some(offer.clone());
        Ok(())
    }

    /**
     * Client works as a subscriber and creates an answer to the offer received from the server.
     */
    pub async fn handle_offer(&mut self, offer_string: String) -> Result<(), String> {
        let offer = RTCSessionDescription::offer(offer_string.clone()).map_err(|e| e.to_string())?;
        
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }
        self.setup_webrtc().await.map_err(|e| e.to_string())?;

        self.create_answer(offer).await.map_err(|e| e.to_string())?;

        let sdp = Some(self.answer.as_ref().unwrap().sdp.clone());
        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: ANSWER.to_string(),
            payload: self.session_id.as_ref().unwrap().as_bytes().to_vec(),
            sdp,
            error: None,
        };

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        self.write.as_mut().ok_or("WebSocket write stream not initialized")?
            .send(Message::Text(msg.into())).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn create_answer(&mut self, offer: RTCSessionDescription) -> Result<(), String> {
        let api = match &self.api {
            Some(api) => api,
            None => return Err("API is not set".to_string()),
        };
    
        let rtc_config = match &self.rtc_config {
            Some(config) => config.clone(),
            None => return Err("RTC config is not set".to_string()),
        };

        // Connection to the server from the subscriber
        let pc = api.new_peer_connection(rtc_config).await.map_err(|e| e.to_string())?;
        self.pc = Some(pc);

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        let audio_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                ..Default::default()
            },
            "audio".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        // Add this newly created track to the PeerConnection
        let _ = self.pc.as_ref().unwrap().add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>).await.map_err(|e| e.to_string())?;
        let _ = self.pc.as_ref().unwrap().add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>).await.map_err(|e| e.to_string())?;

        let _ = self.pc.as_ref().unwrap().set_remote_description(offer.clone()).await.map_err(|e| e.to_string())?;

        let answer = self.pc.as_ref().unwrap().create_answer(None).await.map_err(|e| e.to_string())?;
        let _ = self.pc.as_ref().unwrap().set_local_description(answer.clone()).await.map_err(|e| e.to_string())?;
        self.answer = Some(answer.clone());
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