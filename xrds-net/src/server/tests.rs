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
    use suppaftp::list;
    use tokio::time::{sleep, Duration};
    use crate::server::XRNetServer;
    use crate::client::{ClientBuilder, Client, WebRTCClient};
    use crate::common::enums::{PROTOCOLS, FtpCommands};
    use crate::common::data_structure::FtpPayload;

    use crate::common::append_to_path;
    use std::sync::Arc;
    use std::thread::sleep as thread_sleep;

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
        let client_handle = tokio::task::spawn_blocking(move || {
            let connect_result = webrtc_client.connect(addr_str.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let close_result = webrtc_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_create() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let client_handle = tokio::task::spawn_blocking(move || {
            let connect_result = webrtc_client.connect(addr_str.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            let close_result = webrtc_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_list() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let client_handle = tokio::task::spawn_blocking(move || {
            let connect_result = webrtc_client.connect(addr_str.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            let list_session_result = webrtc_client.list_sessions();
            assert_eq!(list_session_result.is_ok(), true);

            list_session_result.unwrap().iter().for_each(|session| {
                println!("{:?}", session);
            });

            let close_result = webrtc_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_multiple() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let client_handle = tokio::task::spawn_blocking(move || {
            let connect_result = webrtc_client.connect(addr_str.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            let list_session_result = webrtc_client.list_sessions();
            assert_eq!(list_session_result.is_ok(), true);

            list_session_result.unwrap().iter().for_each(|session| {
                println!("{:?}", session);
            });

            let close_result = webrtc_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_close() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let client_handle = tokio::task::spawn_blocking(move || {
            let connect_result = webrtc_client.connect(addr_str.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            let first_session_id = create_session_result.unwrap();
            println!("First session id: {}", first_session_id.clone());

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            let list_session_result = webrtc_client.list_sessions();
            assert_eq!(list_session_result.is_ok(), true);

            list_session_result.unwrap().iter().for_each(|session| {
                println!("{}", session);
            });

            let close_session_result = webrtc_client.close_session(first_session_id.clone().as_str());
            assert_eq!(close_session_result.is_ok(), true);

            let remaining_sessions = webrtc_client.list_sessions().unwrap();
            assert_eq!(remaining_sessions.len(), 1);

            let close_result = webrtc_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_join() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let client_handle = tokio::task::spawn_blocking(move || {
            let connect_result = webrtc_client.connect(addr_str.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            println!("Session id: {}", create_session_result.clone().unwrap());

            thread_sleep(Duration::from_secs(15));

            let close_result = webrtc_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        let mut participant_client = WebRTCClient::new();
        let addr_str2 = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let participant = tokio::task::spawn_blocking(move || {
            // lazy start for the second client
            thread_sleep(Duration::from_secs(5));

            let connect_result = participant_client.connect(addr_str2.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let list_result = participant_client.list_sessions();
            let sessions = list_result.unwrap();

            let room_id = sessions[0].clone();
            println!("Joining Room id: {}", room_id.clone());

            let join_session_result = participant_client.join_session(room_id.as_str());
            assert_eq!(join_session_result.is_ok(), true);

            let close_result = participant_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        participant.await.unwrap();
        client_handle.await.unwrap();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_webrtc_session_leave() {
        let current_line = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, current_line);

        sleep(Duration::from_secs(2)).await;

        let mut webrtc_client = WebRTCClient::new();
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let client_handle = tokio::task::spawn_blocking(move || {
            let connect_result = webrtc_client.connect(addr_str.as_str());
            assert_eq!(connect_result.is_ok(), true);

            let create_session_result = webrtc_client.create_session();
            assert_eq!(create_session_result.is_ok(), true);

            let session_id = create_session_result.clone().unwrap();
            println!("Session id: {}", session_id.clone());

            thread_sleep(Duration::from_secs(10));

            let check_participants_result = webrtc_client.list_participants(session_id.as_str());
            let participants = check_participants_result.unwrap();
            println!("Remaining Participants: {:?}", participants);
            
            assert_eq!(participants.len(), 1);

            let close_result = webrtc_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        let mut participants_client = WebRTCClient::new();
        let addr_str2 = "ws://127.0.0.1".to_owned() + ":" + &current_line.to_string() + "/";
        let client_handle2 = tokio::task::spawn_blocking(move || {
            thread_sleep(Duration::from_secs(5));
            
            let _ = participants_client.connect(addr_str2.as_str());

            let session_list = participants_client.list_sessions().unwrap();
            let session_id = session_list[0].clone();

            println!("Joining Room id: {}", session_id.clone());
            let join_result = participants_client.join_session(session_id.as_str());
            assert_eq!(join_result.is_ok(), true);

            let leave_result = participants_client.leave_session(session_id.as_str());
            assert_eq!(leave_result.is_ok(), true);

            let close_result = participants_client.close_connection();
            assert_eq!(close_result.is_ok(), true);
        });

        client_handle2.await.unwrap();
        client_handle.await.unwrap();
        server_handle.abort();
    }

}