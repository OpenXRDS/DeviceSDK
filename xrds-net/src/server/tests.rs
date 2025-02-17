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
    use tokio::time::{sleep, Duration};
    use crate::server::XRNetServer;
    use crate::client::{ClientBuilder, Client};
    use crate::common::enums::{PROTOCOLS, FtpCommands};
    use crate::common::data_structure::FtpPayload;
    use crate::common::append_to_path;

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

    fn run_server(port: u32) -> tokio::task::JoinHandle<()> {
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

    #[tokio::test]
    async fn test_server_ftp_connection() {
        let current_line = line!();
        let server_handle = run_server(current_line);
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
        let server_handle = run_server(current_line);
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
        let server_handle = run_server(current_line);
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
        let server_handle = run_server(current_line);
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
}