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
    use crate::common::data_structure::{FtpPayload, FtpResponse};

    fn connect_client(port: u32) -> Client {
        let client = ClientBuilder::new()
            .set_protocol(PROTOCOLS::FTP)
            .set_user("admin")
            .set_password("admin")
            .build();

        println!("Connecting to server");
        let addr = ["127.0.0.1", port.to_string().as_str()].join(":");
        
        let ftp = client.set_url(addr.as_str()).connect();
        if ftp.is_err() {
            println!("{}", ftp.err().unwrap());
            panic!("Connection failed");
        } else {
            println!("Connection successful");
            ftp.unwrap()
        }
    }

    fn run_server(port: u32) -> tokio::task::JoinHandle<()> {
        let protocols = vec![PROTOCOLS::FTP];
        let ports = vec![port];

        let server = XRNetServer::new(protocols, ports);
        let server_handle = tokio::spawn(async move {
            println!("Starting server");
            server.start().await;
        });
        server_handle
    }

    #[tokio::test]
    async fn test_server_connection() {
        let current_line = line!();
        let server_handle = run_server(current_line);
        sleep(Duration::from_secs(2)).await;

        let client_handle = tokio::task::spawn_blocking(move || {
            let _ = connect_client(current_line);
        });
        
        client_handle.await.unwrap();
        server_handle.abort();
    }

    /**
     * TODO: Need more tests on the server side for different root directory setting
     */
    #[tokio::test]
    async fn test_server_list() {
        let current_line = line!();
        let server_handle = run_server(current_line);
        sleep(Duration::from_secs(2)).await;
        
        let client_handle = tokio::task::spawn_blocking(move || {
            let client = connect_client(current_line);

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
}