/*
 Copyright 2025 KETI

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

      https://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License.
 */

/**
 * Server testings by running integration test with client
 * No mocking is done here
 */

mod tests {
    use rumqttc::Subscribe;
    use tokio::time::{sleep, Duration};
    use webrtc::sdp::description::session;
    use crate::server::XRNetServer;
    use crate::client::{ClientBuilder, Client, WebRTCClient};
    use crate::common::enums::{PROTOCOLS, FtpCommands};
    use crate::common::data_structure::{FtpPayload, WebRTCMessage, 
        CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, LEAVE_SESSION, CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER, WELCOME};
    use crate::common::{append_to_path, payload_str_to_vector_str};
    use tokio::time::timeout;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use std::thread::sleep as thread_sleep;
    use tokio::runtime::Runtime;

    async fn wait_for_message(mut client: WebRTCClient, msg_type: &str, timeout_secs: u64) -> (WebRTCMessage, WebRTCClient) {
        let msg = timeout(Duration::from_secs(timeout_secs), async {
            loop {
                if let Some(msg) = client.receive_message().await {
                    if msg.message_type == msg_type {
                        return msg;
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect(&format!("Timed out waiting for {}", msg_type));
        (msg, client)
    }

    async fn echo_handler(msg: Vec<u8>) -> Option<Vec<u8>> {
        let msg_str = String::from_utf8(msg.clone()).unwrap();
        println!("This is custom handler: {:?}", msg_str);
        Some(msg)
    }

    /**
     * Since this function is blocking, must be called with tokio::task::spawn_blocking
     */
    fn connect_ws_client(port: u32) -> Client {
        let client = ClientBuilder::new()
            .set_protocol(PROTOCOLS::WS)
            .build();

        let addr = "ws://127.0.0.1".to_string() + ":" + &port.to_string() + "/";
        println!("Connecting to {}", addr.clone());

        let ws = client.set_url(addr.as_str()).connect();
        if ws.is_err() {
            println!("{}", ws.err().unwrap());
            panic!("Connection failed");
        } else {
            ws.unwrap()
        }
    }

    fn connect_ftp_client(port: u32) -> Client {
        let client = ClientBuilder::new()
            .set_protocol(PROTOCOLS::FTP)
            .set_user("admin")
            .set_password("admin")
            .build();

        let addr = ["127.0.0.1", port.to_string().as_str()].join(":");
        
        let ftp = client.set_url(addr.as_str()).connect();
        if ftp.is_err() {
            println!("{}", ftp.err().unwrap());
            panic!("Connection failed");
        } else {
            ftp.unwrap()
        }
    }

    fn run_ftp_server(port: u32) -> tokio::task::JoinHandle<()> {
        let protocols = vec![PROTOCOLS::FTP];
        let ports = vec![port];
        let crnt_dir = std::env::current_dir().unwrap();
        let target_dir = append_to_path(crnt_dir, "/test_root_dir"); 
        let root_dir = Some(target_dir.as_path().to_str().unwrap().to_string());

        let server = XRNetServer::new(protocols, ports);
        let server_handle = tokio::spawn(async move {
            println!("Starting server");
            server.set_root_dir(root_dir.unwrap().as_str()).start().await;
        });
        server_handle
    }

    fn run_server(protocol: PROTOCOLS, port: u32) -> tokio::task::JoinHandle<()> {
        let crnt_dir = std::env::current_dir().unwrap();
        let target_dir = append_to_path(crnt_dir, "/test_root_dir"); 
        let root_dir = Some(target_dir.as_path().to_str().unwrap().to_string());

        let server = XRNetServer::new(vec![protocol], vec![port]);
        let server_handle = tokio::spawn(async move {
            server.set_root_dir(root_dir.unwrap().as_str()).start().await;
        });
        server_handle
    }

    #[tokio::test]
    async fn test_server_run_multiple() {
        let current_line = line!();
        let protocol_vec: Vec<PROTOCOLS> = vec![PROTOCOLS::FTP, PROTOCOLS::QUIC];
        let ports: Vec<u32> = vec![current_line, current_line + 1];
        let crnt_dir = std::env::current_dir().unwrap();
        let target_dir = append_to_path(crnt_dir, "/test_root_dir");
        
        let server = XRNetServer::new(protocol_vec, ports);
        let server_handle = tokio::spawn(async move {
            server.set_root_dir(target_dir.as_path().to_str().unwrap()).start().await;
        });

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_ftp_connection() {
        let current_line = line!();
        let server_handle = run_ftp_server(current_line);
        sleep(Duration::from_secs(2)).await;

        let client_handle = tokio::task::spawn_blocking(move || {
            let _ = connect_ftp_client(current_line);
        });
        
        client_handle.await.unwrap();
        server_handle.abort();

        assert!(true);
    }

    #[tokio::test]
    async fn test_server_ftp_list() {
        let current_line = line!(); // To avoid duplicate port number for each test
        let server_handle = run_ftp_server(current_line);
        sleep(Duration::from_secs(2)).await;
        
        let client_handle = tokio::task::spawn_blocking(move || {
            let client = connect_ftp_client(current_line);

            let ftp_payload = FtpPayload {
                command: FtpCommands::LIST,
                payload_name: "".to_string(),
                payload: None,
            };

            let ftp_response = client.run_ftp_command(ftp_payload);
            if ftp_response.error.is_some() {
                println!("{}", ftp_response.error.unwrap());
                assert!(false);
            } else {
                let res_body = ftp_response.payload.clone().unwrap();

                let res_str = String::from_utf8(res_body).unwrap();
                println!("{}", res_str);
                assert!(res_str.len() > 0);
            }
        });

        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_ftp_noop() {
        let current_line = line!(); // To avoid duplicate port number for each test
        let server_handle = run_ftp_server(current_line);
        sleep(Duration::from_secs(2)).await;
        
        let client_handle = tokio::task::spawn_blocking(move || {
            let client = connect_ftp_client(current_line);

            let ftp_payload = FtpPayload {
                command: FtpCommands::NOOP,
                payload_name: "".to_string(),
                payload: None,
            };

            let ftp_response = client.run_ftp_command(ftp_payload);
            ftp_response
        });

        let ftp_response = client_handle.await.unwrap();
        server_handle.abort();

        assert!(ftp_response.payload.is_none());
    }

    /**
     * This test includes these commands:
     * - MKD
     * - CWD
     * - PWD
     * - APPE: libunftp library does not support APPE command
     * - STOR
     * - RETR
     * - DELE
     * - CDUP
     * - RMD: can remove empty directory only
     */
    #[tokio::test]
    async fn test_server_ftp_crud() {
        let current_line = line!(); // To avoid duplicate port number for each test
        let server_handle = run_ftp_server(current_line);
        sleep(Duration::from_secs(2)).await;
        
        let ftp_payload_mkd = FtpPayload { command: FtpCommands::MKD, payload_name: "test_dir".to_string(), payload: None,};
        let ftp_payload_cwd = FtpPayload {command: FtpCommands::CWD, payload_name: "test_dir".to_string(), payload: None, };
        let ftp_payload_pwd = FtpPayload {command: FtpCommands::PWD, payload_name: "".to_string(), payload: None, };
        let ftp_payload_stor1 = FtpPayload {command: FtpCommands::STOR, payload_name: "test_file1.txt".to_string(), payload: Some("test1".as_bytes().to_vec()), };
        let ftp_payload_stor2 = FtpPayload {command: FtpCommands::STOR, payload_name: "test_file2.txt".to_string(), payload: Some("test2".as_bytes().to_vec()), };
        // let ftp_payload_appe = FtpPayload {command: FtpCommands::APPE, payload_name: "test_file1.txt".to_string(), payload: Some("appended".as_bytes().to_vec()), };
        let ftp_payload_retr = FtpPayload {command: FtpCommands::RETR, payload_name: "test_file1.txt".to_string(), payload: None, };
        let ftp_payload_dele1 = FtpPayload {command: FtpCommands::DELE, payload_name: "test_file1.txt".to_string(), payload: None, };
        let ftp_payload_dele2 = FtpPayload {command: FtpCommands::DELE, payload_name: "test_file2.txt".to_string(), payload: None, };
        let ftp_payload_list = FtpPayload {command: FtpCommands::LIST, payload_name: "".to_string(), payload: None, };
        let ftp_payload_cdup = FtpPayload {command: FtpCommands::CDUP, payload_name: "".to_string(), payload: None, };
        let ftp_payload_rmd = FtpPayload {command: FtpCommands::RMD, payload_name: "./test_dir".to_string(), payload: None, };

        let client_handle = tokio::task::spawn_blocking(move || {
            let client = connect_ftp_client(current_line);

            let ftp_response_mkd = client.run_ftp_command(ftp_payload_mkd);
            assert!(ftp_response_mkd.error.is_none());

            let ftp_response_cwd = client.run_ftp_command(ftp_payload_cwd);
            println!("{:?}", ftp_response_cwd.payload);
            assert!(ftp_response_cwd.error.is_none());

            let ftp_response_pwd = client.run_ftp_command(ftp_payload_pwd);
            let res_body_str = String::from_utf8(ftp_response_pwd.payload.clone().unwrap()).unwrap();
            println!("pwd: {:?}", res_body_str);

            let ftp_response_stor1 = client.run_ftp_command(ftp_payload_stor1);
            assert!(ftp_response_stor1.error.is_none());

            let ftp_response_stor2 = client.run_ftp_command(ftp_payload_stor2);
            assert!(ftp_response_stor2.error.is_none());

            let ftp_response_list = client.run_ftp_command(ftp_payload_list.clone());
            let res_body = ftp_response_list.payload.clone().unwrap();
            let res_str = String::from_utf8(res_body).unwrap();
            println!("list: {:?}", res_str);

            let ftp_response_retr = client.run_ftp_command(ftp_payload_retr);
            let res_body = ftp_response_retr.payload.clone().unwrap();
            let res_str = String::from_utf8(res_body).unwrap();
            println!("retr: {:?}", res_str);

            let ftp_response_dele1 = client.run_ftp_command(ftp_payload_dele1);
            assert!(ftp_response_dele1.error.is_none());

            let ftp_response_dele2 = client.run_ftp_command(ftp_payload_dele2);
            assert!(ftp_response_dele2.error.is_none());

            let ftp_response_cdup = client.run_ftp_command(ftp_payload_cdup);
            assert!(ftp_response_cdup.error.is_none());

            let ftp_response_rmd = client.run_ftp_command(ftp_payload_rmd);
            if ftp_response_rmd.error.is_some() {
                println!("{}", ftp_response_rmd.clone().error.unwrap());
            }
            assert!(ftp_response_rmd.error.is_none());

            let ftp_response_list = client.run_ftp_command(ftp_payload_list);
            let res_body = ftp_response_list.payload.clone().unwrap();
            let res_str = String::from_utf8(res_body).unwrap();
            println!("list: {:?}", res_str);
        });

        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_websocket_register_handler() {
        let mut server = XRNetServer::new(vec![PROTOCOLS::WS], vec![line!()]);

        server.register_handler("test", |msg| Box::pin(echo_handler(msg)));
        
        assert!(true);
    }

    #[tokio::test]
    async fn test_server_websocket_run() {
        let current_line = line!();
        let server_handle = run_server(PROTOCOLS::WS, current_line);

        sleep(Duration::from_secs(2)).await;
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_websocket_connection() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WS, current_line);

        sleep(Duration::from_secs(2)).await;

        let ws_client_handle = tokio::task::spawn_blocking(move || {
                let _ = connect_ws_client(current_line);
            }
        );

        ws_client_handle.await.unwrap();
        server_handle.abort();
        // ws_server_handle.await.unwrap();
        assert!(true);
    }

    #[tokio::test]
    async fn test_server_websocket_rcv() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WS, current_line);

        sleep(Duration::from_secs(2)).await;

        let ws_client_handle = tokio::task::spawn_blocking(move || {
                let client = connect_ws_client(current_line);

                let msg = "test".as_bytes().to_vec();
                let result = client.send(msg, None);
                println!("client send result: {:?}", result.clone());
                assert_eq!(result.is_ok(), true);

                let close_result = result.unwrap().close();
                println!("client close result {:?}", close_result.clone());
                assert_eq!(close_result.is_ok(), true);
            }
        );

        ws_client_handle.await.unwrap();
        // ws_server_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_websocket_custome_handler() {
        let current_line = line!() + 8000;
        let crnt_dir = std::env::current_dir().unwrap();
        let target_dir = append_to_path(crnt_dir, "/test_root_dir"); 
        let root_dir = Some(target_dir.as_path().to_str().unwrap().to_string());

        let mut server = XRNetServer::new(vec![PROTOCOLS::WS], vec![current_line]);
        server.register_handler("text", |msg| Box::pin(echo_handler(msg)));
        let server_handle = tokio::spawn(async move {
            server.set_root_dir(root_dir.unwrap().as_str()).start().await;
        });

        sleep(Duration::from_secs(2)).await;

        let ws_client_handle = tokio::task::spawn_blocking(move || {
                let client = connect_ws_client(current_line);

                let msg = "hello world".as_bytes().to_vec();
                let mut result = client.send(msg, Some("text"));

                assert_eq!(result.is_ok(), true);

                let rcv_result = result.as_mut().unwrap().rcv();
                println!("client received {:?}", rcv_result.clone());

                let close_result = result.unwrap().close();
                assert_eq!(close_result.is_ok(), true);
            }
        );

        ws_client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_run() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(15)).await;

        assert!(true);
        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_webrtc_connect_signal() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";

        webrtc_client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let client_id = timeout(Duration::from_secs(2), async {
            loop {
                if let Some(msg) = webrtc_client.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");

        println!("Test: client_id received: {}", client_id);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_multiuser() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        webrtc_client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let client_id = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = webrtc_client.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");

        sleep(Duration::from_secs(2)).await;
        println!("Starting second client");
        let mut webrtc_client2 = WebRTCClient::new();
        webrtc_client2.connect(addr_str.as_str()).await.expect("Failed to connect");

        let client_id2 = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = webrtc_client2.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for client_id");

        println!("Test: client_id received: {}", client_id);
        println!("Test: client_id2 received: {}", client_id2);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_create() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";

        webrtc_client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let client_id = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = webrtc_client.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");

        println!("Test: client_id received: {}", client_id);

        let mut result = webrtc_client.create_session().await.expect("Failed to create session");
        let session_id = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = result.receive_message().await {
                    if msg.message_type == CREATE_SESSION {
                        let session_id = String::from_utf8_lossy(&msg.payload).to_string();
                        return session_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for session_id");

        println!("Test: session_id received: {}", session_id);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_list() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";

        webrtc_client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let client_id = timeout(Duration::from_secs(2), async {
            loop {
                if let Some(msg) = webrtc_client.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");
        println!("Test: client_id received: {}", client_id);

        let mut client = webrtc_client.create_session().await.expect("Failed to create session");
        
        client = client.list_sessions().await.expect("Failed to list sessions");

        let session_list = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = client.receive_message().await {
                    if msg.message_type == LIST_SESSIONS {
                        let session_list = String::from_utf8_lossy(&msg.payload).to_string();
                        return session_list;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for session list");

        println!("Test: session_list received: {}", session_list);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_multiple() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";

        webrtc_client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let client_id = timeout(Duration::from_secs(2), async {
            loop {
                if let Some(msg) = webrtc_client.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");

        println!("Test: client_id received: {}", client_id);
        
        let mut client = webrtc_client.create_session().await.expect("Failed to create session");
        client = client.create_session().await.expect("Failed to create session");
        client = client.list_sessions().await.expect("Failed to list sessions");

        let session_list = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = client.receive_message().await {
                    if msg.message_type == LIST_SESSIONS {
                        let session_list = String::from_utf8_lossy(&msg.payload).to_string();
                        return session_list;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for session list");

        println!("Test: session_list received: {}", session_list);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_close() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        
        webrtc_client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let client_id = timeout(Duration::from_secs(2), async {
            loop {
                if let Some(msg) = webrtc_client.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");

        let mut client = webrtc_client.create_session().await.expect("Failed to create session");
        client = client.list_sessions().await.expect("Failed to list sessions");

        let session_list = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = client.receive_message().await {
                    if msg.message_type == LIST_SESSIONS {
                        let session_list = String::from_utf8_lossy(&msg.payload).to_string();
                        return session_list;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for session list");

        let session_list = payload_str_to_vector_str(session_list.as_str());

        println!("Test: session_list received: {:?}", session_list);

        let session_id = session_list[0].clone();
        println!("Test: session_id received: {}", session_id);

        client = client.close_session(session_id.as_str()).await.expect("Failed to close session");
        let close_result = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = client.receive_message().await {
                    if msg.message_type == CLOSE_SESSION {
                        let close_result = String::from_utf8_lossy(&msg.payload).to_string();
                        return close_result;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for close session result");

        println!("Test: close_result received: {}", close_result);

        client = client.list_sessions().await.expect("Failed to list sessions");
        let session_list = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(msg) = client.receive_message().await {
                    if msg.message_type == LIST_SESSIONS {
                        let session_list = String::from_utf8_lossy(&msg.payload).to_string();
                        return session_list;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for session list");

        let session_list = payload_str_to_vector_str(session_list.as_str());
        println!("Test: session_list received: {:?}", session_list);
        
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_join() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_publisher = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        
        let mut webrtc_subscriber = WebRTCClient::new();

        webrtc_publisher.connect(addr_str.as_str()).await.expect("Failed to connect");
        let client_id = timeout(Duration::from_secs(2), async {
            loop {
                if let Some(msg) = webrtc_publisher.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");

        let mut webrtc_publisher = webrtc_publisher.create_session().await.expect("Failed to create session");        

        webrtc_subscriber.connect(addr_str.as_str()).await.expect("Failed to connect");
        let client_id2 = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(msg) = webrtc_subscriber.receive_message().await {
                    if msg.message_type == "WELCOME" {
                        return msg.client_id;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Timed out waiting for client_id");

        webrtc_subscriber = webrtc_subscriber.list_sessions().await.expect("Failed to list sessions");
        let session_list = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(msg) = webrtc_subscriber.receive_message().await {
                    if msg.message_type == LIST_SESSIONS {
                        let session_list = String::from_utf8_lossy(&msg.payload).to_string();
                        return session_list;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for session list");

        println!("Test: session_list received: {}", session_list);

        let session_id = payload_str_to_vector_str(session_list.as_str());
        let session_id = session_id[0].clone();
        println!("Test: session_id received: {}", session_id);

        webrtc_subscriber = webrtc_subscriber.join_session(session_id.as_str()).await.expect("Failed to join session");

        let join_result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(msg) = webrtc_subscriber.receive_message().await {
                    if msg.message_type == JOIN_SESSION {
                        let sdp = msg.sdp.clone();
                        return sdp;
                    }
                } else {
                    println!("No message received");
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Timed out waiting for join result");

        println!("Test: join_result received: {:?}", join_result);  // sdp is supposed to be None for this test 

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_list_participants() {
        let port = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;

        let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

        let mut client = WebRTCClient::new();
        client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (msg, client) = wait_for_message(client, "WELCOME", 2).await;
        let client_id = msg.client_id;
        println!("Test: client_id received: {}", client_id);

        let client = client.create_session().await.expect("Failed to create session");
        let (msg, client) = wait_for_message(client, CREATE_SESSION, 5).await;

        let client = client.list_sessions().await.expect("Failed to list sessions");
        let (session_list_msg, client) = wait_for_message(client, LIST_SESSIONS, 5).await;
        let session_list = payload_str_to_vector_str(&String::from_utf8_lossy(&session_list_msg.payload));
        println!("Test: session_list received: {:?}", session_list);

        let session_id = session_list[0].clone();
        println!("Test: session_id received: {}", session_id);

        let session_id = session_list[0].clone();
        let client = client.join_session(&session_id).await.expect("Failed to join session");
        let (join_sdp, client) = wait_for_message(client, JOIN_SESSION, 5).await;
        println!("Test: join_result received: {:?}", join_sdp.sdp); // sdp is supposed to be None for this test

        let client = client.list_participants(&session_id).await.expect("Failed to list participants");
        let (participants_msg, client) = wait_for_message(client, LIST_PARTICIPANTS, 5).await;
        let participants = payload_str_to_vector_str(&String::from_utf8_lossy(&participants_msg.payload));
        println!("Test: participants received: {:?}", participants);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_leave() {
        let port = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;

        let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

        let mut client = WebRTCClient::new();
        client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (msg, client) = wait_for_message(client, WELCOME, 2).await;
        let client_id = msg.client_id;
        println!("Test: client_id received: {}", client_id);

        let client = client.create_session().await.expect("Failed to create session");
        let (msg, client) = wait_for_message(client, CREATE_SESSION, 5).await;

        let client = client.list_sessions().await.expect("Failed to list sessions");
        let (session_list_msg, client) = wait_for_message(client, LIST_SESSIONS, 5).await;
        let session_list = payload_str_to_vector_str(&String::from_utf8_lossy(&session_list_msg.payload));
        println!("Test: session_list received: {:?}", session_list);

        let session_id = session_list[0].clone();
        println!("Test: session_id received: {}", session_id);

        let session_id = session_list[0].clone();
        let client = client.join_session(&session_id).await.expect("Failed to join session");
        let (join_sdp, client) = wait_for_message(client, JOIN_SESSION, 5).await;
        println!("Test: join_result received: {:?}", join_sdp.sdp); // sdp is supposed to be None for this test

        let client = client.list_participants(&session_id).await.expect("Failed to list participants");
        let (participants_msg, client) = wait_for_message(client, LIST_PARTICIPANTS, 5).await;
        let participants = payload_str_to_vector_str(&String::from_utf8_lossy(&participants_msg.payload));

        assert_eq!(participants.len(), 1);

        let client = client.leave_session(&session_id).await.expect("Failed to leave session");
        let (leave_result, client) = wait_for_message(client, LEAVE_SESSION, 5).await;

        let client = client.list_participants(&session_id).await.expect("Failed to list participants");
        let (participants_msg, client) = wait_for_message(client, LIST_PARTICIPANTS, 5).await;
        let participants = payload_str_to_vector_str(&String::from_utf8_lossy(&participants_msg.payload));
        println!("Test: participants received: {:?}", participants);

        assert_eq!(participants.len(), 0);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_create_offer() {
        let mut client = WebRTCClient::new();
        client.test_offer_creation().await.expect("Failed to create offer");
    }

    #[tokio::test]
    async fn test_server_webrtc_offer() {
        let port = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;

        let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

        let mut client = WebRTCClient::new();
        client.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (msg, client) = wait_for_message(client, WELCOME, 2).await;
        let client_id = msg.client_id;
        println!("Test: client_id received: {}", client_id);

        // create session
        let client = client.create_session().await.expect("Failed to create session");
        let (msg, client) = wait_for_message(client, CREATE_SESSION, 5).await;
        let session_id = String::from_utf8_lossy(&msg.payload).to_string();
        println!("Test: session_id created: {}", session_id);
        let mut client = client;
        client.publish(&session_id, None).await.expect("Failed to publish");
        let (publish_result, client) = wait_for_message(client, OFFER, 5).await;
        println!("Test: publish_result received: {:?}", publish_result.sdp); // sdp is supposed to be None for this test

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_answer() {
        let port = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;

        let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

        let mut publisher = WebRTCClient::new();
        publisher.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (msg, publisher) = wait_for_message(publisher, WELCOME, 2).await;
        
        let publisher = publisher.create_session().await.expect("Failed to create session");
        let (msg, publisher) = wait_for_message(publisher, CREATE_SESSION, 5).await;

        let session_id = String::from_utf8_lossy(&msg.payload).to_string();
        println!("Test: session_id created: {}", session_id);
        
        let mut publisher = publisher;
        publisher.publish(&session_id, None).await.expect("Failed to publish");
        let (publish_result, publisher) = wait_for_message(publisher, OFFER, 5).await;
        println!("Test: publish_result received: {:?}", publish_result.sdp); // sdp is supposed to be None for this test

        // subscriber joins the session
        let mut subscriber = WebRTCClient::new();
        subscriber.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (msg, subscriber) = wait_for_message(subscriber, WELCOME, 2).await;
        let client_id = msg.client_id;
        // println!("Test: client_id received: {}", client_id);
        
        let subscriber = subscriber.join_session(&session_id).await.expect("Failed to join session");
        let (join_result, subscriber) = wait_for_message(subscriber, JOIN_SESSION, 5).await;
        // println!("Test: join_result received: {:?}", join_result.sdp); // sdp is supposed to be None for this test
        
        let mut subscriber = subscriber;
        subscriber.handle_offer(join_result.sdp.unwrap()).await.expect("Failed to handle offer");

        let (answer_result, subscriber) = wait_for_message(subscriber, ANSWER, 5).await;
        // println!("Test: answer_result received: {:?}", answer_result.sdp); // sdp is supposed to be None for this test

        let (offer_result, publisher) = wait_for_message(publisher, ANSWER, 5).await;
        println!("Test: offer_result received: {:?}", offer_result.sdp); // This must be different with the offer

        assert_ne!(publish_result.sdp, offer_result.sdp);

        server_handle.abort();
    }
}