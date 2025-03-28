use futures::{FutureExt, TryFutureExt};
use serde::Deserialize;
use webrtc::data::message::message_type;
use webrtc::sdp::description::session;
use std::io::{BufReader, Write};
use std::thread::sleep;
use std::time::Duration;
use tokio::sync::Mutex;
use webrtc::media::io::h264_reader::H264Reader;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
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
use std::sync::Arc;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;
use tokio_tungstenite::WebSocketStream as WsStream;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use tokio_tungstenite::MaybeTlsStream;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use futures_util::stream::SplitSink;
use std::error::Error;
use tokio::sync::mpsc;

use crate::common::data_structure::{NetResponse, WebRTCMessage};
use crate::common::data_structure::{CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, 
        LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER, WELCOME, ICE_CANDIDATE, ICE_CANDIDATE_ACK};

pub struct WebRTCClient {
    client_id: Option<String>,
    write: Option<Arc<Mutex<SplitSink<WsStream<MaybeTlsStream<TcpStream>>, Message>>>>,
    incoming_rx: Option<mpsc::Receiver<WebRTCMessage>>,
    run_handle: Option<tokio::task::JoinHandle<()>>,
    session_id: Option<String>,

    // WebRTC specific fields
    pc: Option<RTCPeerConnection>,
    api: Option<webrtc::api::API>,
    rtc_config: Option<RTCConfiguration>,
    offer: Option<RTCSessionDescription>,
    answer: Option<RTCSessionDescription>,

    video_track: Option<Arc<TrackLocalStaticSample>>,
    // audio_track: Option<Arc<TrackLocalStaticSample>>,

    ice_candidates: Option<Arc<Mutex<Vec<RTCIceCandidate>>>>,
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

            video_track: None,
            // audio_track: None,

            ice_candidates: None,
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
        self.write = Some(Arc::new(Mutex::new(write)));

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
                    self.session_id = msg.session_id.clone().into();
                }
                return Some(msg);
            }
        }
        None
    }

    pub async fn create_session(self) -> Result<Self, Box<dyn Error>> {
        let client_id = self.client_id.as_ref().ok_or("client_id not set")?;
        
        let msg = WebRTCMessage {
            client_id: client_id.clone(),
            session_id: "".to_string(),
            message_type: CREATE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };

        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Text(msg.into())).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(self)
    }

    pub async fn send_message(&mut self, message: &str) -> Result<(), Box<dyn Error>> {
        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Text(message.into())).await?;
            
        } else {
            return Err("WebSocket write stream not initialized".into());
        }
        Ok(())
    }

    pub async fn close_connection(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Close(None)).await?;
            
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        if let Some(handle) = self.run_handle.take() {
            handle.await?;
        }
        println!("WebRTCClient connection closed");

        Ok(())
    }

    pub async fn list_sessions(self) -> Result<Self, Box<dyn Error>> {
        let client_id = self.client_id.as_ref().ok_or("client_id not set")?;

        let msg = WebRTCMessage {
            client_id: client_id.clone(),
            session_id: "".to_string(),
            message_type: LIST_SESSIONS.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };

        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Text(msg.into())).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(self)
    }

    pub async fn close_session(self, session_id: &str) -> Result<Self, Box<dyn Error>> {
        if self.client_id.is_none() {
            return Err("[close_session]Client ID is not set".into());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            session_id: session_id.to_string(),
            message_type: CLOSE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };

        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        println!("Sending message: {}", msg);

        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Text(msg.into())).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(self)
    }

    pub async fn join_session(mut self, session_id: &str) -> Result<Self, Box<dyn Error>> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".into());
        }

        self.session_id = Some(session_id.to_string());

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            session_id: session_id.to_string(),
            message_type: JOIN_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };

        println!("Joining session: {}", session_id);

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        println!("Sending message: {}", msg);
        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Text(msg.into())).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(self)
    }

    pub async fn leave_session(self, session_id: &str) -> Result<Self, Box<dyn Error>> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".into());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            session_id: session_id.to_string(),
            message_type: LEAVE_SESSION.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            let _ = write_guard.send(Message::Text(msg.into())).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(self)
    }

    pub async fn list_participants(self, session_id: &str) -> Result<Self, Box<dyn Error>> {

        let msg = WebRTCMessage {
            client_id: "".to_string(),
            session_id: session_id.to_string(),
            message_type: LIST_PARTICIPANTS.to_string(),
            ice_candidates: None,
            sdp: None,
            error: None,
        };

        // // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        println!("Sending message: {}", msg);

        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Text(msg.into())).await?;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(self)
    }

    /* ****************************************** */
    /* WebRTC specific methods */
    /* ****************************************** */
    pub async fn start_streaming(&mut self, sample_file: Option<&str>) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        if sample_file.is_none() {
            // stream from camera
        } else {
            // stream from file
            self.stream_from_file(sample_file.unwrap()).await.map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    async fn stream_from_camera(&mut self) -> Result<(), String> {
        Ok(())
    }

    /**
     * Currently only supports H264
     * TODO: add support for other codecs by using ffmpeg
     */
    async fn stream_from_file(&mut self, sample_file: &str) -> Result<(), String> {
        let file = File::open(sample_file).map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);

        //TODO: read a file with ffmpeg to divide tracks

        let mut h264_reader = H264Reader::new(reader, 1_048_576);

        let mut ticker = tokio::time::interval(Duration::from_millis(33));  // 30 fps
        let video_track = self.video_track.as_ref().ok_or("Video track not set")?.clone();
        
        // start loop with a separate thread
        tokio::spawn(async move {
            loop {
                let nal = match h264_reader.next_nal() {
                    Ok(nal) => {
                        // println!("Reading NAL...");
                        nal
                    },
                    Err(e) => {
                        eprintln!("Error reading NAL: {}", e);
                        break;
                    }
                };
                
                let _ = video_track.write_sample(&Sample {
                    data: nal.data.freeze(),
                    duration: Duration::from_secs(1),
                    ..Default::default()
                }).await;

                let _ = ticker.tick().await;
            }
        });

        Ok(())
    }

    /**
     * Create an offer and sent it to the server.
     */
    pub async fn publish(&mut self, session_id: &str) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        // track must be setup before creating offer
        self.create_offer().await.map_err(|e| e.to_string())?;

        let sdp = Some(self.offer.as_ref().unwrap().sdp.clone());
        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            session_id: session_id.to_string(),
            message_type: OFFER.to_string(),
            ice_candidates: None,
            sdp,
            error: None,
        };

        // // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        // println!("Sending message: {}", msg);

        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            let _ = write_guard.send(Message::Text(msg.into())).await;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

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

    pub async fn send_ice_candidates(&mut self, is_ack: bool) -> Result<(), String> {
        let message_type = if is_ack {
            ICE_CANDIDATE_ACK.to_string()
        } else {
            ICE_CANDIDATE.to_string()
        };

        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        if self.pc.as_ref().is_none() {
            return Err("PeerConnection is not set".to_string());
        }

        // println!("ICE candidates: {:?}", self.ice_candidates);
        let candidates = self.ice_candidates.as_ref().ok_or("ICE candidates not set")?;
        let candidates_vec = candidates.lock().await;
        let ice_candidates = serde_json::to_string(&*candidates_vec).map_err(|e| e.to_string())?;

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            session_id: self.session_id.clone().unwrap(),
            message_type,
            ice_candidates: Some(ice_candidates),
            sdp: None,
            error: None,
        };

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        // println!("Sending message: {}", msg);

        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            let _ = write_guard.send(Message::Text(msg.into())).await;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(())
    }

    pub async fn create_offer(&mut self) -> Result<(), String> {
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

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        self.ice_candidates = Some(Arc::new(Mutex::new(Vec::new())));
        let candidates_vec = Arc::clone(&self.ice_candidates.as_ref().unwrap());
        // for the given candidate, accumulate it to the vector, then send them all to the server when completed
        pc.on_ice_candidate(Box::new({
            let candidates = Arc::clone(&candidates_vec);
            move |candidate| {
                let candidates = Arc::clone(&candidates);
                Box::pin(async move {
                    if let Some(cand) = candidate {
                        println!("ICE candidate: {:?}", cand);
                        // accumulate the candidate to the vector
                        candidates.lock().await.push(cand.clone());
                    } else {
                        // store the candidates to the client
                        println!("ICE gathering completed: {}", candidates.lock().await.len());
                    }
                })
            }
        }));

        pc.on_ice_connection_state_change(Box::new(|state| {
            Box::pin(async move {
                println!("publisher.ICE Connection State: {:?}", state);
            })
        }));

        // Add this newly created track to the PeerConnection
        let _ = pc.add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>).await.map_err(|e| e.to_string())?;
        
        self.video_track = Some(video_track);

        let offer = pc.create_offer(None).await
            .map_err(|e| e.to_string())?;
        pc.set_local_description(offer.clone()).await.map_err(|e| e.to_string())?;

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        // println!("Created offer: {:?}", self.pc.as_ref().unwrap().local_description().await.unwrap());
        self.offer = Some(offer.clone());
        self.pc = Some(pc);
        Ok(())
    }

    /**
     * Client works as a publisher and receives an answer from the subscriber
     */
    pub async fn handle_answer(&mut self, msg: WebRTCMessage) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        self.answer = Some(RTCSessionDescription::answer(msg.sdp.unwrap()).map_err(|e| e.to_string())?);
        
        // set remote description
        let pc = match &self.pc {
            Some(pc) => pc,
            None => return Err("PeerConnection is not set".to_string()),
        };
        pc.set_remote_description(self.answer.clone().unwrap()).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    /**
     * Client works as a subscriber and creates an answer to the offer received from publisher
     * 
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
            session_id: self.session_id.clone().unwrap(),
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp,
            error: None,
        };

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        // println!("Sending message: {}", msg);

        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            let _ = write_guard.send(Message::Text(msg.into())).await;
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

        Ok(())
    }

    pub async fn handle_ice_candidate(&mut self, msg: WebRTCMessage) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        let pc = match &self.pc {
            Some(pc) => pc,
            None => return Err("PeerConnection is not set".to_string()),
        };

        let ice_candidates_str = msg.ice_candidates.clone().ok_or("ICE candidates not set")?;
        let ice_candidates: Vec<RTCIceCandidate> = serde_json::from_str(&ice_candidates_str).map_err(|e| e.to_string())?;

        // add all ice candidates to the peer connection
        for ice_candidate in ice_candidates {
            let ice_candidate_init = ice_candidate.to_json().map_err(|e| e.to_string())?;
            pc.add_ice_candidate(ice_candidate_init).await.map_err(|e| e.to_string())?;
        }
        println!("[{:?}] ICE candidate added", self.client_id.clone().unwrap());

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // print ice state
        let ice_state = pc.ice_connection_state();
        println!("ICE connection state: {:?},{:?}", self.client_id, ice_state);
        Ok(())
    }

    async fn create_answer(&mut self, offer: RTCSessionDescription) -> Result<(), String> {
        println!("Creating answer");
        let api = match &self.api {
            Some(api) => api,
            None => return Err("API is not set".to_string()),
        };
    
        let rtc_config = match &self.rtc_config {
            Some(config) => config.clone(),
            None => return Err("RTC config is not set".to_string()),
        };
        
        // p2p connection to the publisher
        let pc = api.new_peer_connection(rtc_config).await.map_err(|e| e.to_string())?;

        // collect ICE candidates of the subscriber
        self.ice_candidates = Some(Arc::new(Mutex::new(Vec::new())));
        let candidates_vec = Arc::clone(&self.ice_candidates.as_ref().unwrap());
        // for the given candidate, accumulate it to the vector, then send them all to the server when completed
        pc.on_ice_candidate(Box::new({
            let candidates = Arc::clone(&candidates_vec);
            move |candidate| {
                let candidates = Arc::clone(&candidates);
                Box::pin(async move {
                    if let Some(cand) = candidate {
                        println!("subscriber.ICE candidate: {:?}", cand);
                        // accumulate the candidate to the vector
                        candidates.lock().await.push(cand.clone());
                    } else {
                        // store the candidates to the client
                        println!("subscriber.ICE gathering completed: {}", candidates.lock().await.len());
                    }
                })
            }
        }));

        pc.on_ice_connection_state_change(Box::new(|state| {
            Box::pin(async move {
                println!("subscriber.ICE Connection State: {:?}", state);
            })
        }));

        self.pc = Some(pc);
        self.pc.as_ref().unwrap().set_remote_description(offer.clone()).await.map_err(|e| e.to_string())?;

        let answer = self.pc.as_ref().unwrap().create_answer(None).await.map_err(|e| e.to_string())?;
        let _ = self.pc.as_ref().unwrap().set_local_description(answer.clone()).await.map_err(|e| e.to_string())?;
        self.answer = Some(answer.clone());
        
        // set handlers for processing video/audio tracks
        self.pc.as_ref().unwrap().on_track(Box::new(move |track, _, _| {    // temporary closure
            println!("On track");
            
            Box::pin(async move {   // temporal handling
                if track.kind() == RTPCodecType::Video {
                    println!("Received video track: {:?}", track);
                } else if track.kind() == RTPCodecType::Audio {
                    println!("Received audio track: {:?}", track);
                }
            })
        }));

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