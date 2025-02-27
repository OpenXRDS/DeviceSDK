use std::sync::Arc;

use coap::client;
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



use crate::common::data_structure::{NetResponse, WebRTCMessage};
use crate::common::generate_random_string;
use crate::client::xrds_websocket::XrdsWebsocket;

use tokio::sync::Notify;

use async_trait::async_trait;

use base64::{engine::general_purpose::STANDARD, Engine as _};


static CREATE_SESSION: &str = "create_session"; // publisher to server
static LIST_SESSIONS: &str = "list_sessions";   // subscriber to server
static JOIN_SESSION: &str = "join_session";     // subscriber to server
static LEAVE_SESSION: &str = "leave_session";   // subscriber to server
static CLOSE_SESSION: &str = "close_session";   // publisher to server
static OFFER: &str = "offer";                   // server to subscriber
static ANSWER: &str = "answer";                 // subscriber to server
static WELCOME: &str = "welcome";               // server to client (publisher or subscriber)

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

#[derive(Clone)]
pub struct WebRTCClient {
    client_id: Option<String>,  // This field is used to identify the client in the signaling server
    ws_client: Option<XrdsWebsocket>,
    session_id: Option<String>, // session_id participating in

}

#[allow(dead_code)]
impl WebRTCClient {
    pub fn new() -> Self {
        WebRTCClient {
            client_id: None,
            ws_client: None,
            session_id: None,
        }
    }

    /**
     * This procedure contains request for client id from the signaling server
     * - This handles welcome message containing client id issued by the server
     */
    pub fn connect(&mut self, ws_url: &str) -> Result<(), String> {
        let ws_client = XrdsWebsocket::new().connect(ws_url);
        if ws_client.is_err() {
            return Err(ws_client.err().unwrap());
        }

        self.ws_client = Some(ws_client.unwrap());

        // handle welcome message to obtain client id
        let ws_client = self.ws_client.as_ref().unwrap();
        let rcv_result = ws_client.rcv_ws();

        if rcv_result.is_err() {
            return Err(rcv_result.err().unwrap());
        }

        let rcv_result = rcv_result.unwrap();
        let rcv_result = String::from_utf8(rcv_result);
        if rcv_result.is_err() {
            return Err(rcv_result.err().unwrap().to_string());
        }

        let rtc_msg_json = rcv_result.unwrap();
        let desirialize_result = serde_json::from_str(rtc_msg_json.as_str());

        if desirialize_result.is_err() {
            return Err(desirialize_result.err().unwrap().to_string());
        }

        let msg: WebRTCMessage = desirialize_result.unwrap();

        println!("Welcome message: {:?}", msg);
        self.client_id = Some(msg.client_id.clone());


        Ok(())
    }

    pub fn close_connection(&self) -> Result<(), String> {
        let ws_client = self.ws_client.as_ref().unwrap();
        let close_result = ws_client.close_ws();
        if close_result.is_err() {
            return Err(close_result.err().unwrap());
        }
        Ok(())
    }

    /**
     * Send create_session message to the signaling server
     * - This message is used to create a new session in the signaling server
     * 
     */
    pub fn create_session(&mut self) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }        

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: CREATE_SESSION.to_string(),
            payload: Vec::new(),
            sdp: None,
            error: None,
        };

        let ws_client = self.ws_client.as_ref().unwrap();

        // serialize msg into json
        let msg = serde_json::to_string(&msg);
        if msg.is_err() {
            return Err(msg.err().unwrap().to_string());
        }

        let msg = msg.unwrap();
        let send_result = ws_client.send_ws(Some("text"), msg.as_bytes().to_vec());
        if send_result.is_err() {
            return Err(send_result.err().unwrap());
        }

        // wait for response
        let rcv_result = ws_client.rcv_ws();
        if rcv_result.is_err() {    // handle receive error
            return Err(rcv_result.err().unwrap());
        }

        let rcv_result = rcv_result.unwrap();
        let rcv_result = String::from_utf8(rcv_result);
        if rcv_result.is_err() {    // handle string conversion error
            return Err(rcv_result.err().unwrap().to_string());
        }

        let rtc_msg_json = rcv_result.unwrap();
        let desirialize_result = serde_json::from_str(rtc_msg_json.as_str());

        if desirialize_result.is_err() {    // handle desirialization error
            return Err(desirialize_result.err().unwrap().to_string());
        }

        let msg: WebRTCMessage = desirialize_result.unwrap();

        println!("Create Session response: {:?}", msg);
        println!("session_id: {:?}", msg.client_id.clone());

        self.session_id = Some(msg.client_id.clone());

        Ok(())
    }
}