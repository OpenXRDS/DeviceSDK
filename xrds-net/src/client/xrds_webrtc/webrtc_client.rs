use std::io::{Read, BufRead};
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

use crate::common::data_structure::WebRTCMessage;
use crate::common::data_structure::{CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, 
        LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER, WELCOME, ICE_CANDIDATE, ICE_CANDIDATE_ACK};
use crate::client::xrds_webrtc::webcam_reader::WebcamReader;
use crate::client::xrds_webrtc::media::audio_capturer::AudioCapturer;
use crate::client::xrds_webrtc::media::audio_capturer::resample_and_convert;

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
    audio_track: Option<Arc<TrackLocalStaticSample>>,

    ice_candidates: Option<Arc<Mutex<Vec<RTCIceCandidate>>>>,
    rtp_sender: Option<Arc<RTCRtpSender>>,

    read_flag: bool,
    debug_file_path: Option<String>,

    // fields for sending video stream from webcam
    video_stream_shutdown: Option<std::sync::Arc<AtomicBool>>,
    video_stream_handles: Vec<JoinHandle<()>>,
    video_stream_frame_tx: Option<UnboundedSender<Vec<u8>>>,

    pub data_channel: Option<std::sync::Arc<RTCDataChannel>>,

    audio_stream_shutdown: Option<std::sync::Arc<AtomicBool>>,
    audio_input_stream: Option<Stream>,

    audio_capturer: Option<AudioCapturer>,
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

            video_stream_shutdown: None,
            video_stream_handles: Vec::new(),
            video_stream_frame_tx: None,

            data_channel: None,

            audio_stream_shutdown: None,

            audio_track: None,
            audio_input_stream: None,
            audio_capturer: None,
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

        let audio_track = self.audio_track.as_ref().ok_or("Audio track not set")?.clone();

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
                let audio_capturer = AudioCapturer::new().await?;
                self.stream_from_webcam(webcam_reader, audio_capturer, video_track, audio_track).await
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
        audio_capturer: AudioCapturer,
        video_track: Arc<TrackLocalStaticSample>,
        audio_track: Arc<TrackLocalStaticSample>,
    ) -> Result<(), String> {
        use crate::client::xrds_webrtc::media::transcoding::jpeg2h264::{Jpeg2H264Transcoder, H264Packet};
        use std::sync::Arc;
        use tokio::sync::mpsc;

        let video_shutdown = Arc::new(AtomicBool::new(false));
        let (video_frame_tx, mut video_frame_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (video_packet_tx, mut video_packet_rx) = mpsc::unbounded_channel::<Vec<H264Packet>>();

        // store control handles on self
        self.video_stream_shutdown = Some(Arc::clone(&video_shutdown));
        self.video_stream_frame_tx = Some(video_frame_tx.clone());

        let mut video_handles: Vec<JoinHandle<()>> = Vec::new();

        // capture task: read_single_frame() -> send JPEG frames
        {
            let capture_shutdown = Arc::clone(&video_shutdown);
            let capture_tx = video_frame_tx.clone();
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
            video_handles.push(capture_handle);
        }   // end of capture task

        // transcode task: jpeg -> H.264 packets
        {
            let trans_shutdown = Arc::clone(&video_shutdown);
            let tx = video_packet_tx.clone();
            let trans_handle: JoinHandle<()> = tokio::spawn(async move {
                let mut transcoder = match Jpeg2H264Transcoder::new(1920, 1080, 30) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("failed to create transcoder: {:?}", e);
                        return;
                    }
                };

                while let Some(jpeg_frame) = video_frame_rx.recv().await {
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
            video_handles.push(trans_handle);
        }   // end of transcode task

        // writer task: take H.264 packets and write NALs to video_track
        {
            let write_shutdown = Arc::clone(&video_shutdown);
            let vt = video_track.clone();
            let write_handle: JoinHandle<()> = tokio::spawn(async move {
                // send SPS/PPS before frames if available
                while let Some(pkts) = video_packet_rx.recv().await {
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
            video_handles.push(write_handle);
        }   // end of writer task

        // audio capture and send task
        {
            let mut capturer = audio_capturer;
            match capturer.init() {
                Ok(_) => {
                    capturer.connect_to_webrtc(audio_track.clone()).await?;
                    self.audio_capturer = Some(capturer);
                }
                Err(e) => {
                    eprintln!("⚠️ Audio initialization failed, continuing with video only: {}", e);
                }
            }
        }
        
        self.video_stream_handles = video_handles;

        Ok(())
    }

    pub async fn stop_stream(&mut self) -> Result<(), String> {
        // Stop video first
        if let Some(flag) = &self.video_stream_shutdown {
            flag.store(true, Ordering::Relaxed);
        }

        // Stop audio capturer BEFORE waiting for video tasks
        if let Some(mut capturer) = self.audio_capturer.take() {
            capturer.stop_capture(); // Add this method to AudioCapturer
            println!("🔚 Audio capturer stopped");
        }

        // Then wait for video tasks
        while let Some(handle) = self.video_stream_handles.pop() {
            let res = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
            if res.is_err() {
                println!("⚠️ Video task timeout during shutdown");
            }
        }

        self.video_stream_shutdown = None;
        self.video_stream_frame_tx = None;
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
        self.create_answer_for_offer(offer).await?;
        self.send_answer_to_server().await?;

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

    async fn create_answer_for_offer(&mut self, offer: RTCSessionDescription) -> Result<(), String> {
        let api = self.api.as_ref().ok_or("API is not set")?;
        let rtc_config = self.rtc_config.as_ref().ok_or("RTC config is not set")?.clone();

        let pc = Arc::new(api.new_peer_connection(rtc_config).await.map_err(|e| e.to_string())?);

        self.setup_subscriber_event_handlers(Arc::clone(&pc)).await?;

        // Set remote description and create answer
        pc.set_remote_description(offer).await.map_err(|e| e.to_string())?;
        let answer = pc.create_answer(None).await.map_err(|e| e.to_string())?;
        pc.set_local_description(answer.clone()).await.map_err(|e| e.to_string())?;
        
        self.pc = Some(pc);
        self.answer = Some(answer);

        Ok(())
    }

    async fn setup_subscriber_event_handlers(&mut self, pc: Arc<RTCPeerConnection>) -> Result<(), String> {
        self.setup_subscriber_data_channel_handler(Arc::clone(&pc))?;

        self.setup_subscriber_ice_handling(Arc::clone(&pc))?;

        self.setup_subscriber_connection_monitoring(Arc::clone(&pc))?;

        self.setup_subscriber_media_handlers(Arc::clone(&pc)).await?;

        Ok(())
    }

    fn setup_subscriber_data_channel_handler(&mut self, pc: Arc<RTCPeerConnection>) -> Result<(), String> {
        let data_channel_ref = std::sync::Arc::new(std::sync::Mutex::new(None::<std::sync::Arc<RTCDataChannel>>));
        let dc_ref_clone = data_channel_ref.clone();

        pc.on_data_channel(Box::new(move |data_channel| {
            let dc_ref = dc_ref_clone.clone();
            Box::pin(async move {
                let label = data_channel.label();
                log::info!("Remote created data channel: {}", label);

                *dc_ref.lock().unwrap() = Some(data_channel.clone());

                let dc_clone = data_channel.clone();
                data_channel.on_message(Box::new(move |msg| {
                    let dc_inner = dc_clone.clone();
                    Box::pin(async move {
                        Self::handle_data_channel_message(dc_inner, msg).await;
                    })
                }));

                let dc_clone2 = data_channel.clone();
                data_channel.on_open(Box::new(move || {
                    let dc = dc_clone2.clone();
                    Box::pin(async move {
                        log::info!("Remote data channel opened: {}", dc.label());
                    })
                }));
            })
        }));

        Ok(())
    }

    async fn handle_data_channel_message(dc: Arc<RTCDataChannel>, msg: webrtc::data_channel::data_channel_message::DataChannelMessage) {
        if msg.is_string {
            if let Ok(s) = std::str::from_utf8(&msg.data) {
                println!("📩 Data channel '{}' message: {}", dc.label(), s);
                
                // Echo back for testing
                if let Err(e) = dc.send_text(format!("Echo: {}", s)).await {
                    eprintln!("Failed to echo message: {:?}", e);
                }
            } else {
                println!("📩 Data channel '{}' received invalid UTF-8", dc.label());
            }
        } else {
            println!("📩 Data channel '{}' binary message: {} bytes", dc.label(), msg.data.len());
        }
    }

    fn setup_subscriber_ice_handling(&mut self, pc: Arc<RTCPeerConnection>) -> Result<(), String> {
        self.ice_candidates = Some(Arc::new(Mutex::new(Vec::new())));
        let candidates_vec = Arc::clone(&self.ice_candidates.as_ref().unwrap());

        pc.on_ice_candidate(Box::new({
            let candidates = Arc::clone(&candidates_vec);
            move |candidate| {
                let candidates = Arc::clone(&candidates);
                Box::pin(async move {
                    if let Some(cand) = candidate {
                        log::trace!("subscriber.ICE candidate: {:?}", cand);
                        // accumulate the candidate to the vector
                        candidates.lock().await.push(cand.clone());
                    } else {
                        let count = candidates.lock().await.len();
                        log::info!("subscriber.ICE gathering completed: {}", count);
                    }
                })
            }
             
        }));

        Ok(())
    }

    fn setup_subscriber_connection_monitoring(&mut self, pc: Arc<RTCPeerConnection>) -> Result<(), String> {
        pc.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                match connection_state {
                    RTCIceConnectionState::Connected => {
                        log::info!("✅ Subscriber ICE connected successfully");
                    }
                    RTCIceConnectionState::Completed => {
                        log::info!("✅ Subscriber ICE completed");
                    }
                    RTCIceConnectionState::Checking => {
                        log::info!("🔍 Subscriber ICE checking...");
                    }
                    RTCIceConnectionState::Closed | RTCIceConnectionState::Disconnected => {
                        log::warn!("❌ Subscriber ICE disconnected/closed");
                    }
                    RTCIceConnectionState::Failed => {
                        log::error!("💥 Subscriber ICE connection failed");
                    }
                    _ => {
                        log::info!("Subscriber ICE connection state changed: {:?}", connection_state);
                }
                }
                Box::pin(async {})
                },
        ));

        pc.on_peer_connection_state_change(Box::new(move |state| {
            log::info!("Subscriber Peer Connection State has changed: {:?}", state);
            Box::pin(async {})
        }));

        Ok(())
    }

    /**
     * Temporarily save received media to a file for debugging
     */
    async fn setup_subscriber_media_handlers(&mut self, pc: Arc<RTCPeerConnection>) -> Result<(), String> {
        let mut debug_file_path = self.get_subscriber_output_path()?;
        debug_file_path = debug_file_path.trim_end_matches('/').to_string();
        let file_name_by_time = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();

        // generate video file name by time
        let video_file_name = format!(
            "{}/{}.h264",
            debug_file_path.clone(),
            file_name_by_time
        );
        let audio_file_name = format!(
            "{}/{}.opus",
            debug_file_path.clone(),
            file_name_by_time
        );

        log::info!("Saving received video to {}", video_file_name);
        log::info!("Saving received audio to {}", audio_file_name);

        let h264_writer: Arc<Mutex<dyn webrtc::media::io::Writer + Send + Sync>> =
            Arc::new(Mutex::new(H264Writer::new(File::create(&video_file_name).map_err(|e| e.to_string())?)));

        let pc_clone = Arc::clone(&pc);
        let h264_writer_clone = Arc::clone(&h264_writer);

        pc.on_track(Box::new(move |track, _, _| {
            let media_ssrc = track.ssrc();
            let pc_for_rtcp = Arc::clone(&pc_clone);
            let writer = Arc::clone(&h264_writer_clone);
            let audio_file_name_clone = audio_file_name.clone();

            let codec = track.codec();
            let mime_type = codec.capability.mime_type.to_lowercase();
            println!("🎯 Track received: MIME={}, SSRC={}, Kind={:?}", 
                mime_type, media_ssrc, track.kind());

            // Setup RTCP feedback for video quality
            Self::setup_rtcp_feedback(pc_for_rtcp, media_ssrc);

            /*
             * Currently handlers simply write received media to disk for debugging.
             * TODO: Open an interface to connect to user-defined processing pipelines.
             */
            Box::pin(async move {
                match mime_type.as_str() {
                    "video/h264" | MIME_TYPE_H264 => {
                        tokio::spawn(Self::handle_subscriber_video_track(track, writer));
                    }
                    MIME_TYPE_OPUS => {
                        tokio::spawn(async move {
                            println!("🎵 Processing audio track...");
                            if let Err(e) = save_audio_to_disk(track, audio_file_name_clone.clone()).await {
                                eprintln!("❌ Audio save error: {:?}", e);
                            } else {
                                println!("✅ Audio saved to: {}", audio_file_name_clone);
                            }
                        });
                    }
                    _ => {
                        println!("❓ Received unknown track type: {} (SSRC: {})", mime_type, media_ssrc);
                    }
                }
            })
        }));

        Ok(())
    }

    fn setup_rtcp_feedback(pc: Arc<RTCPeerConnection>, media_ssrc: u32) {
        tokio::spawn(async move {
            let mut result = AnyResult::<usize>::Ok(0);
            let mut rtcp_count = 0;
            
            while result.is_ok() {
                tokio::time::sleep(Duration::from_secs(3)).await;
                rtcp_count += 1;
                
                result = pc.write_rtcp(&[Box::new(PictureLossIndication {
                    sender_ssrc: 0,
                    media_ssrc,
                })]).await.map_err(Into::into);
                
                if rtcp_count % 10 == 0 {
                    println!("📡 Sent {} RTCP PLI packets for SSRC {}", rtcp_count, media_ssrc);
                }
            }
            
            if let Err(e) = result {
                println!("⚠️ RTCP feedback ended: {:?}", e);
            }
        });
    }

    /**
     * Temporarily save received audio to a file for debugging
     */
    async fn handle_subscriber_video_track(
        track: Arc<TrackRemote>,
        writer: Arc<Mutex<dyn webrtc::media::io::Writer + Send + Sync>>
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Starting video track processing...");
    
        let notify = Arc::new(Notify::new());
        save_to_disk_by_writer2(writer, track, notify).await?;
        
        Ok(())
    }

    async fn handle_subscriber_audio_track(output_path: String, track: Arc<TrackRemote>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Starting audio track processing...");

        save_audio_to_disk(track, output_path).await?;
        Ok(())
    }

    fn get_subscriber_output_path(&self) -> Result<String, String> {
        match &self.debug_file_path {
            Some(path) => Ok(path.clone()),
            None => {
                let output_dir = "test_output";
                std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
                Ok(output_dir.to_string())
            }
        }
    }

    async fn send_answer_to_server(&mut self) -> Result<(), String> {
        let sdp = Some(self.answer.as_ref().unwrap().sdp.clone());
        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            session_id: self.session_id.clone().unwrap(),
            message_type: ANSWER.to_string(),
            ice_candidates: None,
            sdp,
            error: None,
        };

        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        
        if let Some(write) = &self.write {
            let mut write_guard = write.lock().await;
            write_guard.send(Message::Text(msg.into())).await.map_err(|e| e.to_string())?;
            println!("📤 Answer sent to server");
        } else {
            return Err("WebSocket write stream not initialized".into());
        }

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

    pub async fn set_debug_file_path(&mut self, path: &str) 
        -> Result<(), String> {
        // check if path is valid
        if !Path::new(path).exists() {
            // create the directory
            let result = std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
            if result != () {
                return Err("Failed to create directory".to_string());
            }
        }
        self.debug_file_path = Some(path.to_string());

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
        self.audio_stream_shutdown = Some(thread_running.clone());
        
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
                            match encode_pcm_to_opus(&mut encoder, &resampled_frame) {
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

    /**
     * For TESTING PURPOSES ONLY
     */
    pub fn stop_audio_capture(&mut self) {
        // 스레드 중지 신호
        if let Some(running_flag) = &self.audio_stream_shutdown {
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
        self.audio_stream_shutdown = None;
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

async fn save_audio_to_disk(
    track: Arc<TrackRemote>,
    output_path: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::fs::File;
        use ogg::writing::{PacketWriter, PacketWriteEndInfo};
        use rand::Rng;

        println!("🎵 Starting audio track save to: {}", output_path);

        // Ensure output directory exists
        if let Some(parent) = std::path::Path::new(output_path.as_str()).parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Opus parameters (standard for WebRTC)
        let opus_sample_rate = 48000u32;
        let opus_channels = 2u8;
        let pre_skip = 312u16; // Standard pre-skip for Opus at 48kHz

        // Create output file and packet writer
        let file = File::create(output_path)?;
        let mut pw = PacketWriter::new(file);
        let stream_serial: u32 = rand::thread_rng().gen();

        // Write OpusHead header (19 bytes total)
        let mut opus_head = Vec::new();
        opus_head.extend_from_slice(b"OpusHead");                   // 8 bytes: magic signature
        opus_head.push(1);                                          // 1 byte: version
        opus_head.push(opus_channels);                              // 1 byte: channel count
        opus_head.extend_from_slice(&pre_skip.to_le_bytes());       // 2 bytes: pre-skip
        opus_head.extend_from_slice(&opus_sample_rate.to_le_bytes()); // 4 bytes: input sample rate
        opus_head.extend_from_slice(&0u16.to_le_bytes());           // 2 bytes: output gain (0 dB)
        opus_head.push(0);                                          // 1 byte: channel mapping family

        println!("Writing OpusHead: {} bytes", opus_head.len());
        pw.write_packet(
            opus_head.into_boxed_slice(),
            stream_serial,
            PacketWriteEndInfo::EndPage,
            0
        )?;

        // Write OpusTags header
        let vendor = b"webrtc-rs-receiver";
        let mut opus_tags = Vec::new();
        opus_tags.extend_from_slice(b"OpusTags");                   // 8 bytes: magic signature
        opus_tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes()); // 4 bytes: vendor string length
        opus_tags.extend_from_slice(vendor);                        // vendor string
        opus_tags.extend_from_slice(&0u32.to_le_bytes());           // 4 bytes: user comment list length

        println!("Writing OpusTags: {} bytes", opus_tags.len());
        pw.write_packet(
            opus_tags.into_boxed_slice(),
            stream_serial,
            PacketWriteEndInfo::EndPage,
            0
        )?;

        // Process RTP packets and extract Opus payload
        let mut packet_count = 0;
        let mut granule_pos: u64 = 0;
        let mut total_payload_size = 0;
        let opus_frame_samples_per_channel = 960; // 20ms at 48kHz

        println!("🎵 Starting to process audio packets...");

        while let Ok((rtp_packet, _)) = track.read_rtp().await {
            packet_count += 1;
            let payload_size = rtp_packet.payload.len();
            total_payload_size += payload_size;

            // Skip empty packets
            if payload_size == 0 {
                println!("⚠️ Skipping empty RTP packet #{}", packet_count);
                continue;
            }

            // Extract Opus payload from RTP packet
            // RTP payload for Opus is the raw Opus packet
            let opus_payload = rtp_packet.payload;

            // Update granule position (samples per channel for this frame)
            granule_pos += opus_frame_samples_per_channel;

            // Write Opus packet to Ogg container
            match pw.write_packet(
                opus_payload.to_vec().into_boxed_slice(),
                stream_serial,
                PacketWriteEndInfo::NormalPacket,
                granule_pos
            ) {
                Ok(_) => {
                    // Log progress every 50 packets (every ~1 second for 20ms frames)
                    if packet_count % 50 == 0 {
                        let duration_secs = granule_pos as f64 / opus_sample_rate as f64;
                        log::trace!(
                            "🎵 Packet #{}: granule_pos={}, duration={:.2}s, payload_size={}",
                            packet_count, granule_pos, duration_secs, payload_size
                        );
                    }
                }
                Err(e) => {
                    log::error!("❌ Failed to write Opus packet #{}: {:?}", packet_count, e);
                    break;
                }
            }
        }

        // Don't write explicit EOS packet - let PacketWriter handle it on drop
        println!(
            "✅ Audio save completed: {} packets, {:.2}s duration, {} bytes total",
            packet_count,
            granule_pos as f64 / opus_sample_rate as f64,
            total_payload_size
        );

        Ok(())
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

/**
 * Test capturing audio from microphone and encoding to Opus file.
 */
#[tokio::test]
async fn test_realtime_mic_to_opus_file() {
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