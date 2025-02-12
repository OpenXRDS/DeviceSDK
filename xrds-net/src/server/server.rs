use std::env;
use std::path::PathBuf;

use crate::common::enums::PROTOCOLS;
use crate::common::{validate_path, validate_path_write_permission, append_to_path};

use unftp_sbe_fs::ServerExt;


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

        // check port availability


        //TODO: root_dir validation in case of ftp/http families

        for i in 0..self.protocol.len() {
            match self.protocol[i] {
                PROTOCOLS::FTP | PROTOCOLS::SFTP => {
                    // create a thread to start a different server for each port
                    self.run_ftp_server(self.port[i]).await;
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
                    // self.run_quic_server();
                }
            }
        }
    }

    async fn run_ftp_server(&self, port: u32) {
        let ftp_home: PathBuf;
        // set root directory as designated dir if the given directory is invalid or not provided
        let root_dir_val_result = validate_path(self.root_dir.as_ref().unwrap());
        if (self.root_dir.is_none()) || (root_dir_val_result.is_err()) {
            println!("Given root directory is invalid. Setting to default test directory");
            let crnt_dir = std::env::current_dir().unwrap();
            
            let target_dir = append_to_path(crnt_dir, "/test_root_dir");
            
            ftp_home = target_dir;
        } else {
            // create a PathBuf from String
            ftp_home = PathBuf::from(self.root_dir.as_ref().unwrap());
        }
        println!("server home: {:?}", ftp_home);

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
}

