use std::path::PathBuf;
use std::sync::Arc;

use random_string::generate;

use crate::common::enums::PROTOCOLS;
use crate::common::{validate_path, validate_path_write_permission};

use unftp_sbe_fs::ServerExt;

// QUIC / HTTP3
use quiche::{Connection, RecvInfo, SendInfo};

const MAX_DATAGRAM_SIZE: usize = 1350;

const RANDOM_STRING_CHARSET: &str = "1234567890abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[derive(Debug, Clone)]
pub struct XRNetServer {
    pub protocol: Vec<PROTOCOLS>,
    pub port: Vec<u32>,

    // Optional fields
    pub greeting: Option<String>,
    pub root_dir: Option<String>,
}

impl XRNetServer {
    pub fn new(protocol: Vec<PROTOCOLS>, port: Vec<u32>) -> XRNetServer {
        XRNetServer {
            protocol,
            port,
            greeting: None,
            root_dir: Some("".to_string()),
        }
    }

    pub fn set_greeting(mut self, greeting: String) -> Self {
        self.greeting = Some(greeting);
        self
    }

    pub fn set_root_dir(mut self, root_dir: &str) -> Self {
        self.root_dir = Some(root_dir.to_string());
        self
    }

    pub async fn start(&self) {
        let server = Arc::new(self.clone());

        // Protocol Check
        if self.protocol.len() == 0 {
            panic!("No protocol is specified");
        }

        if self.port.len() == 0 {
            panic!("No port is specified");
        }

        if self.protocol.len() != self.port.len() {
            panic!("Protocol and Port size mismatch");
        }

        if validate_path(self.root_dir.clone().unwrap().as_str()).is_err() {
            panic!("Invalid root directory");
        }

        if validate_path_write_permission(self.root_dir.clone().unwrap().as_str()).is_err() {
            panic!("No write permission to the root directory");
        }   

        for i in 0..self.protocol.len() {
            match self.protocol[i] {
                PROTOCOLS::FTP | PROTOCOLS::SFTP => {
                    let server = Arc::clone(&server);
                    let port = self.port[i];
                    tokio::spawn(async move {
                        server.run_ftp_server(port).await;
                    });
                }
                PROTOCOLS::HTTP | PROTOCOLS::HTTPS => {
                    // self.run_http_server();
                }
                PROTOCOLS::MQTT => {
                    // self.run_mqtt_server();
                }
                PROTOCOLS::COAP => {
                    // self.run_coap_server();
                }
                PROTOCOLS::FILE => {
                    // self.run_file_server();
                }
                PROTOCOLS::WS | PROTOCOLS::WSS => {
                    // self.run_ws_server();
                }
                PROTOCOLS::WEBRTC => {
                    // self.run_webrtc_server();
                }
                PROTOCOLS::HTTP3 => {
                    // self.run_http3_server();
                }
                PROTOCOLS::QUIC => {
                    let server = Arc::clone(&server);
                    let port = self.port[i];
                    tokio::spawn(async move {
                        server.run_quic_server(port).await;
                    });
                }
            }
        }
    }

    async fn run_ftp_server(&self, port: u32) {
        println!("FTP server started");
        let ftp_home: PathBuf;
        // set root directory as designated dir if the given directory is invalid or not provided
        let root_dir_val_result = validate_path(self.root_dir.as_ref().unwrap());
        if (self.root_dir.is_none()) || (root_dir_val_result.is_err()) {
            println!("Given root directory is invalid. Setting to default test directory");
            ftp_home = std::env::temp_dir();
            
        } else {
            let target_dir = self.root_dir.as_ref().unwrap();
            
            ftp_home = PathBuf::from(target_dir.as_str());
        }
        println!("ftp server home: {:?}", ftp_home);

        let server = libunftp::Server::with_fs(ftp_home)
        .build()
        .unwrap();

        let host_addr = ["127.0.0.1", port.to_string().as_str()].join(":");
        let listen_result = server.listen(host_addr.as_str()).await;

        if let Err(e) = listen_result {
            println!("Error starting FTP server: {}", e);
        } else {
            println!("FTP server started");
        }
    }

    async fn run_quic_server(&self, port: u32) {
        println!("QUIC server started");

        // quic server starts from udp socket
        // server requires certificate and private key
        
    }

    fn create_quic_config(&self) -> quiche::Config {
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
        config.verify_peer(false);

        config.set_application_protos(quiche::h3::APPLICATION_PROTOCOL).unwrap();
        config.set_max_idle_timeout(5000);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_stream_data_uni(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);
        config.set_disable_active_migration(true);

        config
    }
}

