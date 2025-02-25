use std::sync::Arc;

use webrtc::{peer_connection::RTCPeerConnection, rtp_transceiver::rtp_codec::RTCRtpCodecCapability};
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_remote::TrackRemote;
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use std::fs::File;
use std::io::BufReader;
use webrtc::media::io::h264_reader::H264Reader;
use webrtc::media::io::ogg_reader::OggReader;
use tokio::sync::mpsc::{Sender, Receiver};
use std::sync::Mutex;

use tokio::sync::Notify;

use async_trait::async_trait;

use base64::{engine::general_purpose::STANDARD, Engine as _};

pub fn create_default_webrtc_config() -> RTCConfiguration {
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };
    config
}

#[derive(Debug, Clone)]
struct WebRTCClient {
    peer: Arc<RTCPeerConnection>,

    notify_tx: Arc<Notify>,
    done_tx: Sender<()>,
    done_rx: Arc<Mutex<Receiver<()>>>,
}


#[async_trait]
trait Signaling {
    async fn create_offer(&self) -> Result<String, Box<dyn std::error::Error>>;
    async fn set_answer(&self, answer: &str) -> Result<(), Box<dyn std::error::Error>>;
    async fn create_answer(&self) -> Result<String, Box<dyn std::error::Error>>;
}

impl WebRTCClient {
    pub async fn new(config: RTCConfiguration) -> Result<Self, Box<dyn std::error::Error>> {
        // let mut registry = Registry::new();
        
        let api = APIBuilder::new().build();

        let peer = Arc::new(api.new_peer_connection(config).await?);
        Ok(WebRTCClient { 
            peer, 
            notify_tx: Arc::new(Notify::new()),
            done_tx: tokio::sync::mpsc::channel(1).0,
            done_rx: Arc::new(Mutex::new(tokio::sync::mpsc::channel(1).1)),
        })
    }
}

#[async_trait]
impl Signaling for WebRTCClient {
    

    async fn create_offer(&self) -> Result<String, Box<dyn std::error::Error>> {
        let offer = self.peer.create_offer(None).await?;
        self.peer.set_local_description(offer.clone()).await?;
        let json = serde_json::to_string(&offer)?;
        let encoded = STANDARD.encode(json.as_bytes()); // padding included
        Ok(encoded)
    }

    async fn set_answer(&self, answer: &str) -> Result<(), Box<dyn std::error::Error>> {
        let decoded = STANDARD.decode(answer.as_bytes())?;
        let answer_desc: RTCSessionDescription = serde_json::from_slice(&decoded)?;
        self.peer.set_remote_description(answer_desc).await?;
        Ok(())
    }

    async fn create_answer(&self) -> Result<String, Box<dyn std::error::Error>> {
        let answer = self.peer.create_answer(None).await?;
        self.peer.set_local_description(answer.clone()).await?;
        let json = serde_json::to_string(&answer)?;
        let encoded = STANDARD.encode(json.as_bytes()); // padding included
        Ok(encoded)
    }

}

/**
 * WebRTC Publisher
 * - creates the offer
 * - accept answer from subscriber(s)
 */
#[derive(Debug, Clone)]
pub struct WebRTCPublisher {
    client: WebRTCClient,
}

impl WebRTCPublisher {
    pub async fn new(config: Option<RTCConfiguration>) -> Self {
        let client;
        if config.is_none() {   // default RTC configuration
            let config = create_default_webrtc_config();
            client = WebRTCClient::new(config).await.unwrap();
        } else {
            client = WebRTCClient::new(config.unwrap()).await.unwrap();
        }

        WebRTCPublisher {
            client,
        }
    }

    pub async fn run_publisher(&self) -> Result<(), Box<dyn std::error::Error>> {
        

        Ok(())
    }

    pub async fn add_track_from_file(&mut self, path: &str, is_video: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let notify = self.client.notify_tx.clone();
        let done_tx = self.client.done_tx.clone();
        
        let notify_tx = self.client.notify_tx.clone();
        self.client.peer.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                println!("Connection State has changed {connection_state}");
                if connection_state == RTCIceConnectionState::Connected {
                    notify_tx.notify_waiters();
                }
                Box::pin(async {})
            },
        ));
    
        // Set the handler for Peer connection state
        // This will notify you when the peer has connected/disconnected
        self.client.peer.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            println!("Peer Connection State has changed: {s}");
    
            if s == RTCPeerConnectionState::Failed {
                // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
                // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
                // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
                println!("Peer Connection has gone to failed exiting");
                let _ = done_tx.try_send(());
            }
    
            Box::pin(async {})
        }));

        let track = if is_video {
            Some(Arc::new(TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(), // TODO: deal with other codecs
                    ..Default::default()
                },
                "video".to_owned(),
                "webrtc-rs".to_owned(),
            )))
        } else {    // audio
            Some(Arc::new(TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_OPUS.to_owned(), // TODO: deal with other codecs
                    ..Default::default()
                },
                "audio".to_owned(),
                "webrtc-rs".to_owned(),
            )))
        };
        
        let rtp_sender = self.client.peer.add_track(track.unwrap()).await?;

        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
            
        });

        let file_name = path.split("/").last().unwrap().to_owned();
        if is_video {
            tokio::spawn(async move {
                let file_res = File::open(&file_name);
                if file_res.is_err() {
                    return;
                }
                let file = file_res.unwrap();
                let reader = BufReader::new(file);
                let mut h264 = H264Reader::new(reader, 1_048_576);

                notify.notified().await;


            });
        } else {

        }

        
        Ok(self.clone())
    }

    pub async fn create_offer(&self) -> Result<String, Box<dyn std::error::Error>> {
        self.client.create_offer().await
    }

    pub async fn set_answer(&self, answer: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.client.set_answer(answer).await
    }
}

#[derive(Debug, Clone)]
pub struct WebRTCSubscriber {
    client: WebRTCClient,
}

impl WebRTCSubscriber {
    pub async fn new(config: Option<RTCConfiguration>) -> Self {
        let client;
        if config.is_none() {   // default RTC configuration
            let config = create_default_webrtc_config();
            client = WebRTCClient::new(config).await.unwrap();
        } else {
            client = WebRTCClient::new(config.unwrap()).await.unwrap();
        }
        WebRTCSubscriber { client }
    }

    pub async fn subscribe<F>(&self, on_track: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(Arc<TrackRemote>) + Send + Sync + 'static,
    {
        let peer = Arc::clone(&self.client.peer);
        peer.on_track(Box::new(move |track, _, _| {
            on_track(track);
            Box::pin(async {})
        }));
        Ok(())
    }

    pub async fn create_offer(&self) -> Result<String, Box<dyn std::error::Error>> {
        self.client.create_offer().await
    }

    pub async fn set_answer(&self, answer: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.client.set_answer(answer).await
    }

    pub async fn create_answer(&self) -> Result<String, Box<dyn std::error::Error>> {
        self.client.create_answer().await
    }
}