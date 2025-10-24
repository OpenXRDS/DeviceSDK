use std::io::{Read, BufRead, Write};
use std::io::BufReader;
use std::path::Path;
use anyhow::Result as AnyResult;
use cpal::traits::StreamTrait;
use webrtc::data_channel::RTCDataChannel;
use std::time::Duration;
use tokio::sync::Mutex;
use webrtc::media::io::h264_reader::H264Reader;
use webrtc::media::io::h264_writer::H264Writer;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::RTCPFeedback;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::peer_connection::policy::ice_transport_policy::RTCIceTransportPolicy;
use webrtc::rtp::packet::Packet;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::{peer_connection::RTCPeerConnection, rtp_transceiver::rtp_codec::RTCRtpCodecCapability};
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::interceptor::registry::Registry;
use webrtc::api::APIBuilder;
use webrtc::track::track_remote::TrackRemote;
use std::fs::File;
use webrtc::media::Sample;
use std::sync::Arc;
use tokio_tungstenite::WebSocketStream as WsStream;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use tokio_tungstenite::MaybeTlsStream;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use futures_util::stream::SplitSink;
use std::error::Error;
use tokio::sync::{mpsc, Notify};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;
use bytes::Bytes;
use tokio::sync::mpsc::UnboundedSender;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use cpal::Stream;
use ogg::writing::{PacketWriter, PacketWriteEndInfo};

use crate::common::data_structure::WebRTCMessage;
use crate::common::data_structure::{CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, 
        LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER, WELCOME, ICE_CANDIDATE, ICE_CANDIDATE_ACK};
use crate::client::xrds_webrtc::webcam_reader::WebcamReader;

pub struct NetworkStreamReader {
    reader: tokio::io::BufReader<TcpStream>,
}

// Implement std::io::Read for NetworkStreamReader using blocking read
impl std::io::Read for NetworkStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use tokio::io::AsyncReadExt;
        // Use block_in_place to allow blocking in async context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.reader.read(buf))
        })
    }
}

impl NetworkStreamReader {
    pub async fn new(url: &str) -> Result<Self, String> {
        // Parse URL and connect to stream
        let stream = TcpStream::connect(url)
            .await
            .map_err(|e| format!("Failed to connect to stream: {}", e))?;
        
        Ok(NetworkStreamReader {
            reader: tokio::io::BufReader::new(stream),
        })
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> tokio::io::Result<usize> {
        use tokio::io::AsyncReadExt;
        self.reader.read(buf).await
    }
}

pub enum StreamSource {
    File(String),
    Webcam(u32), // device ID
    MediaStream(Box<dyn Read + Send>),
    RawH264(Vec<u8>),
}

// For creating different types of readers
pub struct StreamReaderFactory;

impl StreamReaderFactory {
    pub fn from_file(path: &str) -> Result<impl Read, std::io::Error> {
        File::open(path)
    }
    
    pub fn from_bytes(data: Vec<u8>) -> impl Read {
        std::io::Cursor::new(data)
    }
    
    // pub async fn from_webcam(device_id: u32) -> Result<impl Read + Send, String> {
    //     WebcamReader::new(device_id).await
    // }

    // pub async fn from_webcam_auto() -> Result<impl Read + Send, String> {
    //     let devices = WebcamReader::list_available_devices().await?;
    //     if devices.is_empty() {
    //         return Err("No webcam devices found".to_string());
    //     }
        
    //     println!("Available devices on {}: {:?}", Self::get_platform_info(), devices);
    //     WebcamReader::new(0).await  // Use first available device
    // }
    
    pub fn get_platform_info() -> String {
        #[cfg(target_os = "linux")]
        return "Linux (V4L2)".to_string();
        
        #[cfg(target_os = "windows")]
        return "Windows (DirectShow)".to_string();
        
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        return "Unsupported Platform".to_string();
    }

    pub async fn from_network_stream(url: &str) -> Result<impl Read + Send, String> {
        NetworkStreamReader::new(url).await
    }
}

pub struct WebRTCClient {
    client_id: Option<String>,
    write: Option<Arc<Mutex<SplitSink<WsStream<MaybeTlsStream<TcpStream>>, Message>>>>,
    incoming_rx: Option<mpsc::Receiver<WebRTCMessage>>,
    run_handle: Option<tokio::task::JoinHandle<()>>,
    session_id: Option<String>,

    // WebRTC specific fields
    pc: Option<Arc<RTCPeerConnection>>,
    api: Option<webrtc::api::API>,
    rtc_config: Option<RTCConfiguration>,
    offer: Option<RTCSessionDescription>,
    answer: Option<RTCSessionDescription>,

    video_track: Option<Arc<TrackLocalStaticSample>>,
    // audio_track: Option<Arc<TrackLocalStaticSample>>,

    ice_candidates: Option<Arc<Mutex<Vec<RTCIceCandidate>>>>,
    rtp_sender: Option<Arc<RTCRtpSender>>,

    read_flag: bool,
    debug_file_path: Option<String>,

    // fields for sending video stream from webcam
    stream_shutdown: Option<std::sync::Arc<AtomicBool>>,
    stream_handles: Vec<JoinHandle<()>>,
    stream_frame_tx: Option<UnboundedSender<Vec<u8>>>,

    pub data_channel: Option<std::sync::Arc<RTCDataChannel>>,

    audio_track: Option<Arc<TrackLocalStaticSample>>,
    audio_input_stream: Option<Stream>,
    
    // For file testing
    audio_output_file: Option<Arc<std::sync::Mutex<File>>>,
    audio_sample_rate: u32,
    audio_channels: u16,

    audio_capture_running: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
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
            rtp_sender: None,

            read_flag: false,
            debug_file_path: None,

            stream_shutdown: None,
            stream_handles: Vec::new(),
            stream_frame_tx: None,

            data_channel: None,
            audio_track: None,
            audio_input_stream: None,

            audio_output_file: None,
            audio_sample_rate: 48000,
            audio_channels: 2,
            audio_capture_running: None,
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

    /**
     * Connect to the WebRTC server using WebSocket.
     */
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

    /**
     * This function is supposed to run in a separate thread.
     * It will receive messages from the server and process them.
     */
    pub async fn run(&mut self) {
        self.read_flag = true;
        while self.read_flag {
            self.handle_incoming_message().await;
        }
    }
 
    /**
     * These messages are from peer connection via Signaling server.
     */
    async fn handle_incoming_message(&mut self) {
        if let Some(ref mut rx) = self.incoming_rx {
            if let Some(msg) = rx.recv().await {

                if msg.message_type == WELCOME {
                    self.client_id = Some(msg.client_id.clone());
                } else if msg.message_type == CREATE_SESSION {  // 
                    self.session_id = msg.session_id.clone().into();
                } else if msg.message_type == OFFER {
                    self.handle_offer(msg.sdp.unwrap()).await.unwrap();
                } else if msg.message_type == ANSWER {
                    self.handle_answer(msg).await.unwrap();
                    self.send_ice_candidates(false).await.unwrap();
                } else if msg.message_type == ICE_CANDIDATE {
                    self.handle_ice_candidate(msg).await.unwrap();
                    self.send_ice_candidates(true).await.unwrap();
                } else if msg.message_type == ICE_CANDIDATE_ACK {
                    self.handle_ice_candidate(msg).await.unwrap();
                }
                else {
                    println!("Unhandled message type: {}", msg.message_type);
                }
            }
        }
    }

    /**
     * This function is designed for step by step unit testing.
     * For actual usage, use run_receive_loop instead.
     */
    pub async fn receive_message(&mut self) -> Option<WebRTCMessage> {
        if let Some(ref mut rx) = self.incoming_rx {
            if let Some(msg) = rx.recv().await {
                let msg_clone = msg.clone();
                if msg.message_type == WELCOME {
                    self.client_id = Some(msg.client_id.clone());
                } else if msg.message_type == CREATE_SESSION {
                    self.session_id = msg.session_id.clone().into();
                } else {
                    // do nothing
                }
                return Some(msg_clone);
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
    pub async fn start_streaming(&mut self, source: Option<StreamSource>) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        let video_track = self.video_track.as_ref()
            .ok_or("Video track not set")?
            .clone();

        // Wait for ICE connection
        let pc = self.pc.as_ref().ok_or("PeerConnection is not set")?.clone();
        let ice_connection_result = tokio::time::timeout(
            Duration::from_secs(10),
            self.wait_for_ice_connection(pc.clone())
        ).await;

        match ice_connection_result {
            Ok(Ok(_)) => println!("ICE connection established!"),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("ICE connection timeout after 10 seconds".to_string()),
        }

        match source {
            Some(StreamSource::File(path)) => {
                let file = File::open(&path).map_err(|e| e.to_string())?;
                self.stream_from_buf_read(BufReader::new(file), video_track).await
            }
            Some(StreamSource::Webcam(device_id)) => {
                let webcam_reader = WebcamReader::new(device_id).await?;
                self.stream_from_webcam(webcam_reader, video_track).await
            }
            Some(StreamSource::MediaStream(stream_reader)) => {
                self.stream_from_buf_read(BufReader::new(stream_reader), video_track).await
            }
            Some(StreamSource::RawH264(data)) => {
                let cursor = std::io::Cursor::new(data);
                self.stream_from_buf_read(BufReader::new(cursor), video_track).await
            }
            None => {
                // self.stream_from_media_stream().await.map_err(|e| e.to_string())
                Err("No stream source provided".to_string())
            }
        }
    }

    /**
     * Sned video stream from webcam device to the peer connection via video track.
     */
    async fn stream_from_webcam(
        &mut self,
        webcam_reader: WebcamReader,
        video_track: Arc<TrackLocalStaticSample>
    ) -> Result<(), String> {
        
        use crate::client::xrds_webrtc::media::transcoding::jpeg2h264::{Jpeg2H264Transcoder, H264Packet};
        use std::sync::Arc;
        use tokio::sync::mpsc;

        let shutdown = Arc::new(AtomicBool::new(false));
        let (frame_tx, mut frame_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (packet_tx, mut packet_rx) = mpsc::unbounded_channel::<Vec<H264Packet>>();

        // store control handles on self
        self.stream_shutdown = Some(Arc::clone(&shutdown));
        self.stream_frame_tx = Some(frame_tx.clone());

        let mut handles: Vec<JoinHandle<()>> = Vec::new();

        // capture task: read_single_frame() -> send JPEG frames
        {
            let capture_shutdown = Arc::clone(&shutdown);
            let capture_tx = frame_tx.clone();
            let mut reader = webcam_reader;
            let capture_handle: JoinHandle<()> = tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(33));
                while !capture_shutdown.load(Ordering::Relaxed) {
                    interval.tick().await;
                    match reader.read_single_frame(1).await {
                        Ok(frame_bytes) => {
                            let _ = capture_tx.send(frame_bytes);
                        }
                        Err(e) => {
                            eprintln!("webcam read error: {:?}", e);
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                    }
                }
                let _ = reader.stop_webcam().await;
            });
            handles.push(capture_handle);
        }   // end of capture task

        // transcode task: jpeg -> H.264 packets
        {
            let trans_shutdown = Arc::clone(&shutdown);
            let tx = packet_tx.clone();
            let trans_handle: JoinHandle<()> = tokio::spawn(async move {
                let mut transcoder = match Jpeg2H264Transcoder::new(1920, 1080, 30) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("failed to create transcoder: {:?}", e);
                        return;
                    }
                };

                while let Some(jpeg_frame) = frame_rx.recv().await {
                    if trans_shutdown.load(Ordering::Relaxed) { break; }
                    match transcoder.transcode_jpeg_to_h264_packet(&jpeg_frame) {
                        Ok(pkts) => {
                            if !pkts.is_empty() {
                                let _ = tx.send(pkts);
                            }
                        }
                        Err(e) => eprintln!("transcode error: {:?}", e),
                    }
                }

                // flush remaining packets on end
                match transcoder.flush_to_packets() {
                    Ok(final_pkts) => {
                        if !final_pkts.is_empty() {
                            let _ = tx.send(final_pkts);
                        }
                    }
                    Err(e) => eprintln!("flush error: {:?}", e),
                }
            });
            handles.push(trans_handle);
        }   // end of transcode task

        // writer task: take H.264 packets and write NALs to video_track
        {
            let write_shutdown = Arc::clone(&shutdown);
            let vt = video_track.clone();
            let write_handle: JoinHandle<()> = tokio::spawn(async move {
                // send SPS/PPS before frames if available
                while let Some(pkts) = packet_rx.recv().await {
                    if write_shutdown.load(Ordering::Relaxed) { break; }

                    for pkt in pkts {
                        if pkt.data.is_empty() { continue; }
                        // NAL type: lowest 5 bits of first byte (assuming annex-b payload)
                        let nalu_type = pkt.data[0] & 0x1F;
                        // duration: best-effort 33ms per frame
                        let sample = Sample {
                            data: Bytes::from(pkt.data),
                            duration: std::time::Duration::from_millis(33),
                            ..Default::default()
                        };

                        // write sample; ignore failures but log
                        if let Err(e) = vt.write_sample(&sample).await {
                            eprintln!("video_track.write_sample error: {:?}", e);
                        }

                        // optional: log SPS/PPS or keyframes
                        if nalu_type == 7 || nalu_type == 8 {
                            // sps / pps
                        }
                    }
                }
            });
            handles.push(write_handle);
        }   // end of writer task

        self.stream_handles = handles;
        Ok(())
    }

    pub async fn stop_stream(&mut self) -> Result<(), String> {
        if let Some(flag) = &self.stream_shutdown {
            flag.store(true, Ordering::Relaxed);
        } else {
            return Ok(());
        }

        // await/abort handles
        while let Some(handle) = self.stream_handles.pop() {
            // try waiting a short time, then abort if hung
            let res = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
            if res.is_err() {
                // task didn't finish in time; the handle is already consumed by timeout
                // no need to abort as timeout already handles it
            }
        }

        self.stream_shutdown = None;
        self.stream_frame_tx = None;
        Ok(())
    }

    async fn stream_from_buf_read<R: BufRead + Send + 'static>(
        &self,
        reader: R,
        video_track: Arc<TrackLocalStaticSample>
    ) -> Result<(), String> {
        let mut h264_reader = H264Reader::new(reader, 1_048_576);
        let mut ticker = tokio::time::interval(Duration::from_millis(42));
        let mut total_nal_size = 0;
        let mut nal_count = 0;
        let mut sent_metadata = false;

        tokio::spawn(async move {
            let start_time = std::time::Instant::now();
            
            while let Ok(nal) = h264_reader.next_nal() {
                nal_count += 1;
                total_nal_size += nal.data.len();
                let nalu_type = nal.data[0] & 0x1F;
                
                if !sent_metadata && (nalu_type == 7 || nalu_type == 8) {
                    let _ = video_track.write_sample(&Sample {
                        data: nal.data.freeze(),
                        duration: Duration::from_millis(42),
                        ..Default::default()
                    }).await;
                } else {
                    let _ = video_track.write_sample(&Sample {
                        data: nal.data.freeze(),
                        duration: Duration::from_millis(42),
                        ..Default::default()
                    }).await;
                    if nalu_type != 7 && nalu_type != 8 {
                        sent_metadata = true;
                    }
                }
                ticker.tick().await;
            }
            
            println!("Stream ended - elapsed: {:?}, NAL count: {}, Total size: {}", 
                start_time.elapsed(), nal_count, total_nal_size);
        });

        Ok(())
    }

    pub async fn get_available_webcams() -> Result<Vec<String>, String> {
        WebcamReader::list_available_devices().await
    }

    /**
     * START OF CONNECTION SETUP METHODS 
     */
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
            ice_servers: vec![
                // STUN servers for NAT discovery
                RTCIceServer {
                    urls: vec![
                        "stun:stun.l.google.com:19302".to_owned(),
                        "stun:stun1.l.google.com:3478".to_owned(),
                        "stun:stun2.l.google.com:19302".to_owned(),
                        "stun:stun.keti.xrds.kr:13478".to_owned(),
                        "stun:stun.keti.xrds.kr:13478?transport=tcp".to_owned(),
                        "stun:stun.keti.xrds.kr:13479".to_owned(),
                        "stun:stun.keti.xrds.kr:13479?transport=tcp".to_owned(),
                    ],
                    ..Default::default()
                },
                // TURN server for relay when direct connection fails
                RTCIceServer {
                    urls: vec![
                        "turn:turn.keti.xrds.kr:13478".to_owned(),
                        "turn:turn.keti.xrds.kr:13478?transport=tcp".to_owned(),
                        "turn:turn.keti.xrds.kr:13479".to_owned(),
                        "turn:turn.keti.xrds.kr:13479?transport=tcp".to_owned(),
                    ],
                    username: "gganjang".to_owned(),
                    credential: "keti007".to_owned(),
                    ..Default::default()
                },
            ],
            ice_transport_policy: RTCIceTransportPolicy::All, // Use this for testing
            ..Default::default()
        };

        self.api = Some(api);
        self.rtc_config = Some(rtc_config.clone());
        let pc = self.api.as_ref().unwrap().new_peer_connection(rtc_config)
            .await.map_err(|e| e.to_string())?;

        // Add ICE gathering state monitoring
        pc.on_ice_gathering_state_change(Box::new(move |state| {
            println!("ICE Gathering State changed to: {:?}", state);
            Box::pin(async {})
        }));

        self.pc = Some(Arc::new(pc));

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
                rtcp_feedback: vec![
                    RTCPFeedback { typ: "nack".to_string(), parameter: "".to_string() },
                    RTCPFeedback { typ: "nack".to_string(), parameter: "pli".to_string() },
                ],
                clock_rate: 90000,
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        let audio_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: 48000,
                channels: 2,
                ..Default::default()
            },
            "audio".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        // Add this newly created track to the PeerConnection
        let rtp_sender = pc.add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>).await.map_err(|e| e.to_string())?;
        let _audio_sender = pc.add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>).await.map_err(|e| e.to_string())?;

        // Create a data channel for messaging
        let pc_arc = Arc::new(pc);
        let dc_open_result = self.create_data_channel(Arc::clone(&pc_arc), "msg").await;
        if let Err(e) = dc_open_result {
            eprintln!("Data channel creation error: {}", e);
        }
        
        self.pc = Some(pc_arc);
        self.rtp_sender = Some(rtp_sender);
        self.video_track = Some(video_track);
        self.audio_track = Some(audio_track);

        let offer = self.pc.as_ref().unwrap().create_offer(None).await.map_err(|e| e.to_string())?;
        self.pc.as_ref().unwrap().set_local_description(offer.clone()).await.map_err(|e| e.to_string())?;

        println!("Waiting for ICE candidate collection...");
        let candidates = self.collect_ice_candidates(15).await?; // 15 second timeout
        println!("Collected {} ICE candidates during offer", candidates.len());

        self.ice_candidates = Some(Arc::new(Mutex::new(candidates)));
        // for the given candidate, accumulate it to the vector, then send them all to the server when completed
        self.offer = Some(offer.clone());

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
        
        // send the answer to the server, so that it can be delivered to the publisher
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

        pc.on_data_channel(Box::new({
            let client_dc_ref = std::sync::Arc::new(std::sync::Mutex::new(None::<std::sync::Arc<RTCDataChannel>>));
            move |data_channel| {
                let client_dc_ref = client_dc_ref.clone();
                Box::pin(async move {
                    let label = data_channel.label();
                    println!("Remote created data channel: {}", label);

                    // set message handler for remote channel
                    let dc_clone = data_channel.clone();
                    data_channel.on_message(Box::new(move |msg| {
                        let dc_inner = dc_clone.clone();
                        Box::pin(async move {
                            if msg.is_string {
                                if let Ok(s) = std::str::from_utf8(&msg.data) {
                                    println!("Remote DC '{}' message: {}", dc_inner.label(), s);
                                }
                            } else {
                                println!("Remote DC '{}' binary {} bytes", dc_inner.label(), msg.data.len());
                            }
                        })
                    }));

                    // Optionally store it somewhere accessible (not shown here)
                    // *client_dc_ref.lock().await = Some(std::sync::Arc::new(data_channel.clone()));
                })
            }
        }));

        // collect ICE candidates of the subscriber
        self.ice_candidates = Some(Arc::new(Mutex::new(Vec::new())));
        let candidates_vec = Arc::clone(&self.ice_candidates.as_ref().unwrap());
        let notify_tx = Arc::new(Notify::new());
        let notify_rx = notify_tx.clone();
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

        let notify_tx_clone = Arc::clone(&notify_tx);
        pc.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                println!("subscriber.Connection State has changed {connection_state}");
    
                if connection_state == RTCIceConnectionState::Connected {
                    println!("Ctrl+C the remote client to stop the demo");
                } else if connection_state == RTCIceConnectionState::Closed 
                || connection_state == RTCIceConnectionState::Disconnected {
                    println!("subscriber.Connection closed");
                    notify_tx_clone.notify_waiters();
                } else if connection_state == RTCIceConnectionState::Failed {
                    println!("subscriber.Connection failed");
                }
                Box::pin(async {})
            },
        ));

        self.pc = Some(Arc::new(pc));
        self.pc.as_ref().unwrap().set_remote_description(offer.clone()).await.map_err(|e| e.to_string())?;

        let answer = self.pc.as_ref().unwrap().create_answer(None).await.map_err(|e| e.to_string())?;
        let _ = self.pc.as_ref().unwrap().set_local_description(answer.clone()).await.map_err(|e| e.to_string())?;
        self.answer = Some(answer.clone());
        
        let notify_rx2 = Arc::clone(&notify_rx);
        let pc_clone = self.pc.clone().unwrap();
        let video_file = match &self.debug_file_path {
            Some(_) => self.debug_file_path.clone().unwrap(),
            None => {
                // Create a dedicated output directory in project root
                let output_dir = "test_output";
                std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
                format!("{}/received.h264", output_dir)
            }
        };
        println!("Saving received video to {}", video_file);

        let h264_writer: Arc<Mutex<dyn webrtc::media::io::Writer + Send + Sync>> =
            Arc::new(Mutex::new(H264Writer::new(File::create(&video_file).map_err(|e| e.to_string())?)));
        let h264_writer2 = Arc::clone(&h264_writer);

        // set handlers for processing video/audio tracks
        self.pc.as_ref().unwrap().on_track(Box::new(move |track, _, _| {
            // get a reference of self.pc
            let media_ssrc = track.ssrc();
            let pc2 = Arc::clone(&pc_clone);

            tokio::spawn(async move {
                let mut result = AnyResult::<usize>::Ok(0);
                while result.is_ok() {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    if let Some(pc) = Some(pc2.clone()) {
                        result = pc.write_rtcp(&[Box::new(PictureLossIndication {
                            sender_ssrc: 0,
                            media_ssrc,
                        })]).await.map_err(Into::into);
                    } else {
                        break;
                    }
                };
            });

            let notify_rx2 = Arc::clone(&notify_rx2);
            let h264_writer2 = Arc::clone(&h264_writer2);
            Box::pin(async move {
                let notify_rx2 = Arc::clone(&notify_rx2);
                let codec = track.codec();
                let mime_type = codec.capability.mime_type.to_lowercase();

                if mime_type == MIME_TYPE_H264.to_lowercase() {
                    println!("Got h264 track, saving to disk as received.h264");
                    tokio::spawn(async move {
                        let track = track.clone();
                        let notify = notify_rx2.clone();

                        let writer = Arc::clone(&h264_writer2);
                        save_to_disk_by_writer2(writer, track, notify).await.unwrap();
                    });

                }
            })
        }));

        Ok(())
    }

    pub async fn collect_ice_candidates(&mut self, timeout_secs: u64) -> Result<Vec<RTCIceCandidate>, String> {
        let pc = self.pc.as_ref().ok_or("PeerConnection not set")?;
    
        let (ice_complete_tx, mut ice_complete_rx) = tokio::sync::oneshot::channel();
        let (candidate_tx, mut candidate_rx) = tokio::sync::mpsc::channel(100);
        
        let ice_complete_tx = Arc::new(Mutex::new(Some(ice_complete_tx)));
        let mut candidates = Vec::new();
        
        // Set up ICE candidate collection
        pc.on_ice_candidate(Box::new({
            let candidate_tx = candidate_tx.clone();
            let ice_complete_tx = Arc::clone(&ice_complete_tx);
            move |candidate| {
                let candidate_tx = candidate_tx.clone();
                let ice_complete_tx = Arc::clone(&ice_complete_tx);
                Box::pin(async move {
                    if let Some(cand) = candidate {
                        println!("ICE candidate collected: {} (type: {})", cand.address, cand.typ);
                        let _ = candidate_tx.send(cand).await;
                    } else {
                        println!("ICE gathering completed");
                        if let Some(tx) = ice_complete_tx.lock().await.take() {
                            let _ = tx.send(());
                        }
                    }
                })
            }
        }));
        
        // Wait for completion or timeout
        let collection_result = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
            loop {
                tokio::select! {
                    Some(candidate) = candidate_rx.recv() => {
                        candidates.push(candidate);
                    }
                    _ = &mut ice_complete_rx => {
                        println!("ICE gathering completed with {} candidates", candidates.len());
                        break;
                    }
                }
            }
        }).await;
        
        match collection_result {
            Ok(_) => {
                if candidates.is_empty() {
                    Err("No ICE candidates collected".to_string())
                } else {
                    Ok(candidates)
                }
            }
            Err(_) => {
                if candidates.is_empty() {
                    Err(format!("ICE candidate collection timeout after {} seconds with no candidates", timeout_secs))
                } else {
                    println!("ICE candidate collection timeout but got {} candidates", candidates.len());
                    Ok(candidates)
                }
            }
        }
    }
    
    async fn wait_for_ice_connection(&self, pc: Arc<RTCPeerConnection>) -> Result<(), String> {
        loop {
            let ice_state = pc.ice_connection_state();
            
            match ice_state {
                RTCIceConnectionState::Connected => return Ok(()),
                RTCIceConnectionState::Failed => return Err("ICE connection failed".to_string()),
                RTCIceConnectionState::Disconnected => return Err("ICE connection disconnected".to_string()),
                RTCIceConnectionState::Closed => return Err("ICE connection closed".to_string()),
                _ => {
                    println!("Waiting for ICE connection state to be connected: {:?}", ice_state);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
    async fn create_data_channel(&mut self, peer_connection: Arc<RTCPeerConnection>, label: &str) -> Result<(), String> {
        let pc = peer_connection;

        let init = RTCDataChannelInit {
            ordered: Some(true),
            max_retransmits: None,
            max_packet_life_time: None,
            negotiated: None,
            protocol: None,
        };

        let create_data_channel_result = pc.create_data_channel(label, Some(init)).await;
        if let Err(e) = create_data_channel_result {
            return Err(format!("Failed to create data channel: {}", e));
        } else {
            println!("Data channel '{}' created", label);
        }
         let data_channel = create_data_channel_result.unwrap();

        let data_channel_clone = data_channel.clone();
        data_channel.on_open(Box::new(move || {
            let dc = data_channel_clone.clone();
            Box::pin(async move {
                println!("Data channel '{}' is open", dc.label());
            })
        }));
        
        let data_channel_clone2 = data_channel.clone();
        data_channel.on_message(Box::new(move |msg| {
            let dc = data_channel_clone2.clone();
            Box::pin(async move {   // message handling
                if msg.is_string {
                    if let Ok(s) = std::str::from_utf8(&msg.data) {
                        println!("Data channel '{}' received message: {}", dc.label(), s);
                    } else {
                        println!("Data channel '{}' received invalid UTF-8 message", dc.label());
                    }
                } else {
                    let data = msg.data;
                    println!("Data channel '{}' received binary message: {} bytes", dc.label(), data.len());
                }
            })
        }));

        self.data_channel = Some(data_channel);
        Ok(())
    }
    /**
     * END OF CONNECTION SETUP METHODS
     * **********************************************************************************************************
     */

    pub async fn send_data_channel_message(&self, message: &str) -> Result<(), String> {
        if let Some(dc) = &self.data_channel {
            dc.send_text(message.to_string()).await.map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Data channel not initialized".to_string())
        }
    }

    pub async fn set_debug_file_path(&mut self, path: &str, file: Option<&str>) 
        -> Result<(), String> {
        // check if path is valid
        if !Path::new(path).exists() {
            // create the directory
            let result = std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
            if result != () {
                return Err("Failed to create directory".to_string());
            }
        }
        
        // concat path and file name
        self.debug_file_path = match file {
            Some(f) => Some(format!("{}/{}", path, f)),
            None => Some(format!("{}/webrtc_client_debug", path)),
        };

        // create the file if not exists
        let debug_file_full_path = self.debug_file_path.clone().unwrap();
        let _ = File::create(&debug_file_full_path).map_err(|e| e.to_string())?;

        Ok(())
    }

    /**
     * FOR TESTING PURPOSES ONLY
     * To verify audio capture and encoding, capture audio from default input device,
     * encode it to Opus format, and save to the specified output file path.
     */
    pub fn capture_audio_encode_to_file(&mut self, output_path: &str) -> Result<(), String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use cpal::SampleFormat;
        use std::fs::File;
        use ogg::writing::{PacketWriter, PacketWriteEndInfo};
        use rand::Rng;
        use crate::client::xrds_webrtc::media::transcoding::pcm2opus::encode_pcm_to_opus;

        // choose host & device
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No input device")?;
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        println!("Supported config: {:?}", supported);

        // ensure output dir exists
        if let Some(parent) = std::path::Path::new(output_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Opus parameters (we target 48k / stereo)
        let device_sample_rate = supported.sample_rate().0;
        let device_channels = supported.channels();

        let opus_sample_rate = 48000;
        let opus_channels = 2;
        let frame_ms = 20;
        let opus_frame_samples_per_channel = (opus_sample_rate / 1000 * frame_ms) as i32; // 960
        let device_frame_samples_per_channel = (device_sample_rate / 1000 * frame_ms) as i32; // 320 for 16kHz
        let device_frame_total_samples = (device_frame_samples_per_channel * device_channels as i32) as usize;

        let pre_skip = 312u16;

        // channel to move PCM into encoder thread
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<i16>>(10);

        // build input stream with correct sample format handling
        let tx_cb = tx.clone();
        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                device.build_input_stream(
                    &supported.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mut buf = Vec::with_capacity(data.len());
                        for &s in data {
                            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            buf.push(v);
                        }
                        let _ = tx_cb.send(buf);
                    },
                    move |e| eprintln!("cpal input stream error: {:?}", e),
                ).map_err(|e| e.to_string())?
            }
            SampleFormat::I16 => {
                device.build_input_stream(
                    &supported.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let _ = tx_cb.send(data.to_vec());
                    },
                    move |e| eprintln!("cpal input stream error: {:?}", e),
                ).map_err(|e| e.to_string())?
            }
            SampleFormat::U16 => {
                device.build_input_stream(
                    &supported.into(),
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let mut tmp = Vec::with_capacity(data.len());
                        for &u in data {
                            tmp.push((u as i32 - 0x8000) as i16);
                        }
                        let _ = tx_cb.send(tmp);
                    },
                    move |e| eprintln!("cpal input stream error: {:?}", e),
                ).map_err(|e| e.to_string())?
            }
        };
        
        // start and keep stream alive
        stream.play().map_err(|e| e.to_string())?;
        println!("Started audio capture and encoding to Ogg/Opus file: {}", output_path);
        self.audio_input_stream = Some(stream);

        // spawn encoder + ogg muxer thread
        let out_path = output_path.to_string();
        
        let thread_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let thread_running_clone = thread_running.clone();
        self.audio_capture_running = Some(thread_running.clone());
        
        let _handle = std::thread::spawn(move || {
            println!("Encoder thread started");
            
            // create opus encoder inside thread
            let mut encoder = match opus::Encoder::new(opus_sample_rate, opus::Channels::Stereo, opus::Application::Audio) {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("Failed to create Opus encoder: {:?}", err);
                    return;
                }
            };

            // open file and packet writer
            let file = match File::create(&out_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Failed to create output file {}: {:?}", out_path, e);
                    return;
                }
            };
            let mut pw = PacketWriter::new(file);
            let stream_serial: u32 = rand::thread_rng().gen();

            // OpusHead with CORRECT pre-skip
            let mut opus_head = Vec::new();
            opus_head.extend_from_slice(b"OpusHead"); // 8
            opus_head.push(1); // version
            opus_head.push(opus_channels as u8);    // output channels
            opus_head.extend_from_slice(&pre_skip.to_le_bytes()); // FIXED: use pre_skip variable
            opus_head.extend_from_slice(&opus_sample_rate.to_le_bytes()); // output sample rate
            opus_head.extend_from_slice(&0u16.to_le_bytes()); // output gain
            opus_head.push(0); // channel mapping family

            println!("OpusHead size: {} bytes", opus_head.len());

            if let Err(e) = pw.write_packet(opus_head.into_boxed_slice(), stream_serial, PacketWriteEndInfo::EndPage, 0) {
                eprintln!("Failed to write OpusHead: {:?}", e);
                return;
            }

            // OpusTags (minimal)
            let vendor = b"webrtc-rs";
            let mut opus_tags = Vec::new();
            opus_tags.extend_from_slice(b"OpusTags"); // 8
            opus_tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
            opus_tags.extend_from_slice(vendor);
            opus_tags.extend_from_slice(&0u32.to_le_bytes()); // user comment list length

            if let Err(e) = pw.write_packet(opus_tags.into_boxed_slice(), stream_serial, PacketWriteEndInfo::EndPage, 0) {
                eprintln!("Failed to write OpusTags: {:?}", e);
                return;
            }

            // encode loop with CORRECTED granule position
            let mut acc: Vec<i16> = Vec::new();
            let mut granule_pos: u64 = pre_skip as u64; // START with pre_skip
            let mut packet_count = 0;
            
            println!("Starting encode loop...");

            loop {
                if !thread_running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("Thread stopping signal received");
                    break;
                }

                match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(pcm_chunk) => {
                        acc.extend_from_slice(&pcm_chunk);
                        while acc.len() >= device_frame_total_samples {
                            let device_frame: Vec<i16> = acc.drain(..device_frame_total_samples).collect();

                            let resampled_frame = resample_and_convert(&device_frame, device_sample_rate, device_channels, opus_sample_rate, opus_channels as u16);
                            match encode_pcm_to_opus(&mut encoder, &resampled_frame, opus_frame_samples_per_channel) {
                                Ok(encoded) => {
                                    packet_count += 1;
                                    // granule position increases by samples per channel
                                    granule_pos += opus_frame_samples_per_channel as u64;

                                    if let Err(e) = pw.write_packet(encoded.into_boxed_slice(), stream_serial, PacketWriteEndInfo::NormalPacket, granule_pos) {
                                        eprintln!("Failed to write Ogg packet: {:?}", e);
                                    }

                                    // Debug output every second
                                    if packet_count % 50 == 0 { // 50 packets = 1 second
                                        println!("Encoded packet #{}, granule_pos: {}, time: {:.2}s", 
                                            packet_count, granule_pos, (granule_pos - pre_skip as u64) as f64 / opus_sample_rate as f64);
                                    }
                                }
                                Err(err) => {
                                    eprintln!("Opus encode error: {}", err);
                                }
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // 타임아웃은 정상 - 계속 진행
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        println!("Audio input disconnected");
                        break;
                    }
                }
            }

            // EOS packet with final granule position
            println!("Writing EOS packet with final granule_pos: {}", granule_pos);
            if let Err(e) = pw.write_packet(Vec::<u8>::new().into_boxed_slice(), stream_serial, PacketWriteEndInfo::EndStream, granule_pos) {
                eprintln!("Failed to write EOS packet: {:?}", e);
            }

            println!("Audio encoding completed. Total packets: {}, Duration: {:.2}s", 
                packet_count, (granule_pos - pre_skip as u64) as f64 / opus_sample_rate as f64);
        });

        Ok(())
    }

    pub fn stop_audio_capture(&mut self) {
        // 스레드 중지 신호
        if let Some(running_flag) = &self.audio_capture_running {
            running_flag.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        
        // 스트림 중지
        if let Some(stream) = self.audio_input_stream.take() {
            let _ = stream.pause();
            println!("Audio stream stopped");
        }
        
        // 스레드가 정리될 시간을 줌
        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        // 플래그 정리
        self.audio_capture_running = None;
    }

    /**
     * Start audio capture from microphone and send to the audio track.
     * TODO : TESTING
     */
    pub fn start_audio_capture(&mut self) -> Result<(), String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use opus::{Encoder as OpusEncoder, Application, Channels};
        use std::sync::mpsc::sync_channel;
        use bytes::Bytes;

        let audio_track = match &self.audio_track {
            Some(t) => t.clone(),
            None => return Err("audio_track not initialized".into()),
        };

        // choose host & device
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No input device")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;

        let sample_rate = 48000u32;
        let channels: Channels = Channels::Stereo;
        let frame_samples = (sample_rate / 1000 * 20) as usize;

        // create opus encoder
        let mut opus_enc = OpusEncoder::new(sample_rate as u32, channels, Application::Audio)
            .map_err(|e| e.to_string())?;

        // channel to move PCM slices into spawned task
        let (tx, rx) = sync_channel::<Vec<i16>>(10);

        // input stream callback: collect samples and send to encoder task
        let tx_cb = tx.clone();
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // convert f32 -> i16 per channel
                let mut buf: Vec<i16> = Vec::with_capacity(data.len());
                for s in data.iter() {
                    let v = (s * i16::MAX as f32) as i16;
                    buf.push(v);
                }
                let _ = tx_cb.send(buf);
            },
            move |e| eprintln!("cpal input stream error: {:?}", e),
        ).map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;

        // spawn encoder + send task
        tokio::spawn(async move {
            // buffer accumulator for frame_samples * channels
            let mut acc: Vec<i16> = Vec::new();
            while let Ok(mut pcm) = rx.recv() {
                acc.append(&mut pcm);
                // encode while we have enough samples for a 20ms frame
                let stride = frame_samples * channels as usize;
                while acc.len() >= stride {
                    let frame = acc.drain(..stride).collect::<Vec<i16>>();
                    // encode to opus
                    let mut opus_output = vec![0u8; 4000]; // Buffer for encoded data
                    match opus_enc.encode(&frame, &mut opus_output) {
                        Ok(encoded_len) => {
                            opus_output.truncate(encoded_len); // Resize to actual encoded length
                            // write to audio_track
                            let sample = Sample {
                                data: Bytes::from(opus_output),
                                duration: std::time::Duration::from_millis(20),
                                ..Default::default()
                            };
                            if let Err(e) = audio_track.write_sample(&sample).await {
                                eprintln!("audio_track write_sample error: {:?}", e);
                                // continue trying; do not break
                            }
                        }
                        Err(e) => eprintln!("Opus encode error: {:?}", e),
                    }
                }
            }
        });

        Ok(())
    }

    /**
     * Record webcam video to MP4 file for a specified duration.
     */
    pub async fn realtime_webcam_to_mp4 (
        device_id: u32,
        output_path: &str,
        duration_seconds: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {

        use crate::client::xrds_webrtc::webcam_reader::WebcamReader;
        use crate::client::xrds_webrtc::media::transcoding::jpeg2h264::{Jpeg2H264Transcoder, H264Packet};
        use crate::client::xrds_webrtc::media::streaming_mp4_writer::StreamingMP4Writer;
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        use tokio::sync::mpsc;

        println!("🎥 Starting real-time webcam to MP4: {} seconds", duration_seconds);

            // Setup webcam reader
        let mut webcam_reader = WebcamReader::new(device_id).await?;
        
        // Setup transcoder and MP4 writer
        let mut transcoder = Jpeg2H264Transcoder::new(1920, 1080, 30)?;
        let mut mp4_writer = StreamingMP4Writer::new(output_path, 1920, 1080, 30)?;
        
        // Setup channels for communication
        let (frame_sender, mut frame_receiver) = mpsc::unbounded_channel::<Vec<u8>>();
        let (packet_sender, mut packet_receiver) = mpsc::unbounded_channel::<Vec<H264Packet>>();

        let shutdown_flag = Arc::new(AtomicBool::new(false));

        // Task 1: Frame capture (30fps)
        let capture_shutdown = Arc::clone(&shutdown_flag);
        let capture_task = tokio::spawn(async move {
            let mut frame_count = 0;
            let frame_interval = tokio::time::Duration::from_millis(33); // ~30fps
            let mut interval = tokio::time::interval(frame_interval);
            
            while !capture_shutdown.load(Ordering::Relaxed) {
                interval.tick().await;
                
                match webcam_reader.read_single_frame(1).await {
                    Ok(jpeg_frame) => {
                        if let Err(_) = frame_sender.send(jpeg_frame) {
                            println!("Frame channel closed");
                            break;
                        }
                        frame_count += 1;
                        
                        if frame_count % 90 == 0 { // Every 3 seconds
                            println!("📸 Captured {} frames", frame_count);
                        }
                    }
                    Err(e) => {
                        eprintln!("Frame capture error: {:?}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
            
            // Stop webcam before ending task
            webcam_reader.stop_webcam().await;
            println!("🔚 Frame capture ended: {} frames", frame_count);
        }); // end of capture task

        // Task 2: JPEG to H.264 transcoding
        let transcode_shutdown = Arc::clone(&shutdown_flag);
        let transcode_task = tokio::spawn(async move {
        let mut processed_frames = 0;
            
            while let Some(jpeg_frame) = frame_receiver.recv().await {
                if transcode_shutdown.load(Ordering::Relaxed) {
                    break;
                }
                
                match transcoder.transcode_jpeg_to_h264_packet(&jpeg_frame) {
                    Ok(h264_packets) => {
                        processed_frames += 1;
                        
                        if !h264_packets.is_empty() {
                            if let Err(_) = packet_sender.send(h264_packets) {
                                println!("Packet channel closed");
                                break;
                            }
                        }
                        
                        if processed_frames % 90 == 0 {
                            println!("🔄 Processed {} frames", processed_frames);
                        }
                    }
                    Err(e) => {
                        eprintln!("Transcoding error: {:?}", e);
                    }
                }
            }
            
            // Flush remaining packets
            match transcoder.flush_to_packets() {
                Ok(final_packets) => {
                    if !final_packets.is_empty() {
                        println!("🔄 Flushed {} final packets", final_packets.len());
                        let _ = packet_sender.send(final_packets);
                    }
                }
                Err(e) => eprintln!("Flush error: {:?}", e),
            }
            
            println!("🔚 Transcoding ended: {} frames processed", processed_frames);
        }); // end of transcoding task

        // Task 3: H.264 packets to MP4 writing
        let write_task = tokio::spawn(async move {
            let mut total_packets = 0;
            
            while let Some(h264_packets) = packet_receiver.recv().await {
                match mp4_writer.write_packets(&h264_packets) {
                    Ok(_) => {
                        total_packets += h264_packets.len();
                    }
                    Err(e) => {
                        eprintln!("MP4 write error: {:?}", e);
                        break;
                    }
                }
            }
            
            // Finalize MP4
            match mp4_writer.finalize() {
                Ok(_) => println!("✅ MP4 finalized with {} packets", total_packets),
                Err(e) => eprintln!("MP4 finalization error: {:?}", e),
            }
        }); // end of writing task

        // Run for specified duration
        tokio::time::sleep(tokio::time::Duration::from_secs(duration_seconds as u64)).await;
    
        // Signal shutdown
        shutdown_flag.store(true, Ordering::Relaxed);
        // Wait for tasks to complete (webcam is stopped in capture task)
        let _ = tokio::join!(capture_task, transcode_task, write_task);
        
        println!("✅ Real-time recording completed: {}", output_path);

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

async fn save_to_disk_by_writer2(
    writer: Arc<Mutex<dyn webrtc::media::io::Writer + Send + Sync>>,
    track: Arc<TrackRemote>,
    notify: Arc<Notify>,
) -> AnyResult<()> {
    let mut total_size = 0;
    let mut last_seq = None;
    let mut received_count = 0;
    let (tx, mut rx) = mpsc::channel::<Packet>(1000);

    // 읽기 태스크
    let track_clone = track.clone();
    let notify_clone = notify.clone();
    tokio::spawn(async move {
        while let Ok((rtp_packet, _)) = track_clone.read_rtp().await {
            if tx.send(rtp_packet).await.is_err() {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            Some(rtp_packet) = rx.recv() => {
                total_size += rtp_packet.payload.len();
                if let Some(last) = last_seq {
                    let current = rtp_packet.header.sequence_number as u32;
                    if current != (last as u32 + 1) % 65536 {
                        println!("Packet loss: {} -> {} (missed: {})", last, current, (current.wrapping_sub(last as u32 + 1)) % 65536);
                    }
                }
                last_seq = Some(rtp_packet.header.sequence_number);
                received_count += 1;
                let mut w = writer.lock().await;
                w.write_rtp(&rtp_packet)?;
            }
            _ = notify_clone.notified() => break,
        }
    }
    println!("Total received packets: {}", received_count);
    println!("Total size: {}", total_size);
    Ok(())
}

fn resample_and_convert(input: &[i16], input_rate:u32, input_channels: u16, output_rate: u32, output_channels: u16) -> Vec<i16> {
    // 16kHz stereo -> 48kHz stereo: 3배 업샘플링
    let ratio = output_rate as f32 / input_rate as f32; // 3.0
    let output_len = ((input.len() as f32 * ratio) as usize / output_channels as usize) * output_channels as usize;
    let mut output = Vec::with_capacity(output_len);

    if input_channels == output_channels {
        // 채널 수가 같으면 단순 업샘플링
        for i in 0..output_len {
            let input_idx = ((i as f32 / ratio) as usize).min(input.len() - 1);
            output.push(input[input_idx]);
        }
    } else if input_channels == 2 && output_channels == 2 {
        // 스테레오 -> 스테레오 리샘플링
        let frames_out = output_len / 2;
        for frame in 0..frames_out {
            let input_frame = ((frame as f32 / ratio) as usize).min(input.len() / 2 - 1);
            output.push(input[input_frame * 2]);     // left
            output.push(input[input_frame * 2 + 1]); // right
        }
    } else {
        // 다른 채널 조합은 일단 단순 복사/확장
        for i in 0..output_len {
            let input_idx = ((i as f32 / ratio) as usize).min(input.len() - 1);
            output.push(input[input_idx]);
        }
    }
    
    output
}

#[tokio::test]
async fn test_realtime_webcam_to_mp4() {
    std::env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();

    let device_id = 0; // Adjust based on your system
    let output_path = "test_output/realtime_webcam.mp4";
    let duration_seconds = 10; // Record for 10 seconds

    match WebRTCClient::realtime_webcam_to_mp4(device_id, output_path, duration_seconds).await {
        Ok(_) => println!("Test completed successfully."),
        Err(e) => eprintln!("Test failed: {:?}", e),
    }
}

#[tokio::test]
async fn test_realtime_mic_to_opus() {
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();

    let mut client = WebRTCClient::new();
    let output_path = "test_output/realtime_mic.opus";

    match client.capture_audio_encode_to_file(output_path) {
        Ok(_) => println!("Audio capture started successfully"),
        Err(e) => {
            eprintln!("Failed to start audio capture: {}", e);
            return;
        }
    }

    // Capture audio for 15 seconds
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
    client.stop_audio_capture();
    println!("Audio capture test completed.");
}