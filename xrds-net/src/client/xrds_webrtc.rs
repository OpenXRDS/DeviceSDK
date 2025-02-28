use std::sync::Arc;

use coap::client;
use webrtc::sdp::description::session;
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
use crate::common::data_structure::{CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER};

use tokio::sync::Notify;

use async_trait::async_trait;

use base64::{engine::general_purpose::STANDARD, Engine as _};




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
            session_id: None,   // allow multiple sessions or not?
        }
    }

    /**
     * This procedure contains request for client id from the signaling server
     * - This handles welcome message containing client id issued by the server
     */
    pub fn connect(&mut self, ws_url: &str) -> Result<(), String> {
        let ws_client = XrdsWebsocket::new()
            .connect(ws_url)
            .map_err(|e| e.to_string())?;

        self.ws_client = Some(ws_client);

        let rtc_msg_json = self.ws_client.as_ref().unwrap()
            .rcv_ws()
            .map_err(|e| e.to_string())
            .and_then(|data| String::from_utf8(data).map_err(|e| e.to_string()))?;

        let msg: WebRTCMessage = serde_json::from_str(&rtc_msg_json)
            .map_err(|e| e.to_string())?;

        println!("Welcome message: {:?}", msg);
        
        self.client_id = Some(msg.client_id.clone());
        
        Ok(())
    }

    pub fn close_connection(&self) -> Result<(), String> {
        self.ws_client.as_ref().unwrap()
            .close_ws()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /**
     * Send create_session message to the signaling server
     * - This message is used to create a new session in the signaling server
     * 
     */
    pub fn create_session(&mut self) -> Result<String, String> {
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
    
        // Serialize and send message
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        ws_client.send_ws(Some("text"), msg.as_bytes().to_vec()).map_err(|e| e)?;
    
        // Receive and process response
        let rcv_data = ws_client.rcv_ws().map_err(|e| e)?;
        let response_str = String::from_utf8(rcv_data).map_err(|e| e.to_string())?;
        let msg: WebRTCMessage = serde_json::from_str(&response_str).map_err(|e| e.to_string())?;
        
        // Extract session ID
        let session_id = String::from_utf8(msg.payload.clone()).map_err(|e| e.to_string())?;
        let session_id = session_id.replace("\"", "");
        
        self.session_id = Some(session_id.clone());
    
        Ok(session_id)
    }
    

    pub fn list_sessions(&self) -> Result<Vec<String>, String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: LIST_SESSIONS.to_string(),
            payload: Vec::new(),
            sdp: None,
            error: None,
        };

        let ws_client = self.ws_client.as_ref().unwrap();

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        ws_client.send_ws(Some("text"), msg.as_bytes().to_vec()).map_err(|e| e.to_string())?;

        // wait for response
        let rtc_msg_json = self.ws_client.as_ref().unwrap()
            .rcv_ws()
            .map_err(|e| e.to_string())
            .and_then(|data| String::from_utf8(data).map_err(|e| e.to_string()))?;

        let msg: WebRTCMessage = serde_json::from_str(&rtc_msg_json)
                            .map_err(|e| e.to_string())?;
        let sessions = String::from_utf8(msg.payload.clone()).map_err(|e| e.to_string())?;

        // erase quotation marks from the string
        let sessions = sessions.replace("\"", "")
                            .replace("[", "")
                            .replace("]", "")
                            .split(",")
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>();

        Ok(sessions)
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: CLOSE_SESSION.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        let ws_client = self.ws_client.as_ref().unwrap();

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;

        ws_client.send_ws(Some("text"), msg.as_bytes().to_vec())
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn join_session(&self, session_id: &str) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: JOIN_SESSION.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        let ws_client = self.ws_client.as_ref().unwrap();

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;

        ws_client.send_ws(Some("text"), msg.as_bytes().to_vec())
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn leave_session(&self, session_id: &str) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: LEAVE_SESSION.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        let ws_client = self.ws_client.as_ref().unwrap();

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;

        ws_client.send_ws(Some("text"), msg.as_bytes().to_vec())
                .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn list_participants(&self, session_id: &str) -> Result<Vec<String>, String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        let msg = WebRTCMessage {
            client_id: self.client_id.clone().unwrap(),
            message_type: LIST_PARTICIPANTS.to_string(),
            payload: session_id.as_bytes().to_vec(),
            sdp: None,
            error: None,
        };

        let ws_client = self.ws_client.as_ref().unwrap();

        // serialize msg into json
        let msg = serde_json::to_string(&msg).map_err(|e| e.to_string())?;

        ws_client.send_ws(Some("text"), msg.as_bytes().to_vec())
                .map_err(|e| e.to_string())?;

        // wait for response
        let rtc_msg_json = self.ws_client.as_ref().unwrap()
            .rcv_ws()
            .map_err(|e| e.to_string())
            .and_then(|data| String::from_utf8(data).map_err(|e| e.to_string()))?;

        let msg: WebRTCMessage = serde_json::from_str(&rtc_msg_json)
            .map_err(|e| e.to_string())?;

        let participants = String::from_utf8(msg.payload.clone()).map_err(|e| e.to_string())?;

        // erase quotation marks from the string
        let participants = participants.replace("\"", "")
                            .replace("[", "")
                            .replace("]", "")
                            .split(",")
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>();

        Ok(participants)
    }
    /**
     * This is to deliver an offer SDP to the server, so that the server can deliver it to the subscriber
     * 
     */
    pub fn offer(&self, session_id: &str) -> Result<(), String> {
        if self.client_id.is_none() {
            return Err("Client ID is not set".to_string());
        }

        


        Ok(())
    }
}