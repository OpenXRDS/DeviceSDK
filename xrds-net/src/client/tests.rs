mod tests {
    use crate::client::{ClientBuilder};
    use crate::client::xrds_webrtc::webrtc_client::{WebRTCClient, StreamSource};
    use crate::common::enums::{PROTOCOLS, FtpCommands};
    use crate::common::data_structure::{FtpPayload, WebRTCMessage, 
        CREATE_SESSION, JOIN_SESSION, OFFER, ANSWER, WELCOME, ICE_CANDIDATE, ICE_CANDIDATE_ACK};
    use crate::common::{append_to_path};
    use crate::server::XRNetServer;
    use crate::client::media::VideoTrackHandler;
    use tokio::time::{sleep, Duration};
    
    use rustls::crypto::{CryptoProvider, ring};
    use ring::default_provider;
    use once_cell::sync::OnceCell;
    use serial_test::serial;
    use std::time::Instant;
    use std::sync::{Arc,Mutex};
    use webrtc::track::track_remote::TrackRemote;
    use std::future::Future;
    use std::pin::Pin;

    
    static HTTP_ECHO_SERVER_URL: &str = "https://echo.free.beeceptor.com";
    static CRYPTO_INIT: OnceCell<()> = OnceCell::new();
    static LAST_HTTP3_TEST: Mutex<Option<Instant>> = Mutex::new(None);
    static DEFAULT_DEBUG_FILE_PATH: &str = "test_output";
    pub struct CustomVideoProcessor {
    }

    impl VideoTrackHandler for CustomVideoProcessor {
        fn handle_video_track<'a>(
        &'a self,
        track: Arc<TrackRemote>,
        ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
            Box::pin(async move {
                println!("🎬 Custom video processor started for track: {}", track.id());
                println!("🎬 Track SSRC: {}", track.ssrc());
                println!("🎬 Track codec: {:?}", track.codec());
                
                let mut packet_count = 0;
                let mut total_bytes = 0;
                let mut min_size = usize::MAX;
                let mut max_size = 0;
                
                // Read RTP packets and print their sizes
                while let Ok((rtp_packet, _attributes)) = track.read_rtp().await {
                    packet_count += 1;
                    let payload_size = rtp_packet.payload.len();
                    let total_packet_size = payload_size + 12; // RTP header is typically 12 bytes
                    
                    total_bytes += payload_size;
                    min_size = min_size.min(payload_size);
                    max_size = max_size.max(payload_size);
                    
                    // Print packet info every 30 packets (roughly 1 second at 30fps)
                    if packet_count % 30 == 0 {
                        println!("📦 Packet #{}: payload={}B, total={}B, seq={}, timestamp={}", 
                            packet_count, 
                            payload_size, 
                            total_packet_size, 
                            rtp_packet.header.sequence_number,
                            rtp_packet.header.timestamp
                        );
                        
                        let avg_size = if packet_count > 0 { total_bytes / packet_count } else { 0 };
                        println!("📊 Stats: avg={}B, min={}B, max={}B, total_packets={}", 
                            avg_size, min_size, max_size, packet_count);
                    }
                    
                    // Print detailed info for first few packets
                    if packet_count <= 5 {
                        println!("🔍 Packet #{} details:", packet_count);
                        println!("   - Payload size: {} bytes", payload_size);
                        println!("   - RTP header size: ~12 bytes");
                        println!("   - Total packet size: ~{} bytes", total_packet_size);
                        println!("   - Sequence number: {}", rtp_packet.header.sequence_number);
                        println!("   - Timestamp: {}", rtp_packet.header.timestamp);
                        println!("   - SSRC: {}", rtp_packet.header.ssrc);
                        
                        // Check if it's a keyframe (for H.264)
                        if payload_size > 0 {
                            let nal_type = rtp_packet.payload[0] & 0x1F;
                            match nal_type {
                                7 => println!("   - NAL type: SPS (Sequence Parameter Set)"),
                                8 => println!("   - NAL type: PPS (Picture Parameter Set)"),
                                5 => println!("   - NAL type: IDR frame (keyframe)"),
                                1 => println!("   - NAL type: Non-IDR frame"),
                                _ => println!("   - NAL type: {} (other)", nal_type),
                            }
                        }
                    }
                }
                
                println!("🔚 Video processing ended:");
                println!("   - Total packets: {}", packet_count);
                println!("   - Total payload bytes: {}", total_bytes);
                println!("   - Average packet size: {}B", if packet_count > 0 { total_bytes / packet_count } else { 0 });
                println!("   - Min packet size: {}B", if min_size == usize::MAX { 0 } else { min_size });
                println!("   - Max packet size: {}B", max_size);
                
                Ok(())
            })
        }
    }

    async fn custom_audio_handler(track: Arc<TrackRemote>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🎵 Custom audio processing started");
        
        let mut packet_count = 0;
        while let Ok((rtp_packet, _)) = track.read_rtp().await {
            packet_count += 1;
            
            if packet_count % 50 == 0 {
                println!("📦 Audio Packet #{}: payload_size={}B, seq={}, timestamp={}", 
                    packet_count, 
                    rtp_packet.payload.len(), 
                    rtp_packet.header.sequence_number,
                    rtp_packet.header.timestamp
                );
            }
        }
        
        println!("🎵 Custom audio processing ended: {} packets", packet_count);
        Ok(())
    }

    //Helper function to setup signaling connection and wait for WELCOME message
    async fn setup_signaling_connection(addr: &str) -> Result<WebRTCClient, String> {
        let mut client = WebRTCClient::new();
        client.connect(addr).await.map_err(|e| e.to_string())?;
        
        // WELCOME 메시지 대기
        let msg = wait_for_message_type(&mut client, WELCOME, 5).await?;
        println!("✅ Client ID received: {}", msg.client_id);
        
        Ok(client)
    }

    // Helper function to setup publisher signaling (create_session + publish(OFFER))
    pub async fn setup_publisher_signaling(client: &mut WebRTCClient) -> Result<String, String> {
        client.create_session().await?;
        let msg = wait_for_message_type(client, CREATE_SESSION, 5).await?;
        let session_id = msg.session_id.clone();
        
        client.publish(&session_id).await?;
        let _msg = wait_for_message_type(client, OFFER, 5).await?;
        
        println!("✅ Publisher signaling completed: {}", session_id);
        Ok(session_id)
    }

    /// Helper function to setup subscriber signaling (join_session + handle_offer)
    pub async fn setup_subscriber_signaling(
        client: &mut WebRTCClient, 
        session_id: &str,
        debug_path: Option<&str>
    ) -> Result<(), String> {
        if let Some(path) = debug_path {
            client.set_debug_file_path(path).await?;
        }

        client.join_session(session_id).await.map_err(|e| e.to_string())?;
        let join_result = wait_for_message_type(client, JOIN_SESSION, 5).await?;
        
        client.handle_offer(join_result.sdp.unwrap()).await?;
        let _msg = wait_for_message_type(client, ANSWER, 5).await?;
        
        println!("✅ Subscriber signaling completed for session: {}", session_id);
        Ok(())
    }

    /// 테스트용 헬퍼: P2P 핸드셰이크 완료 (Publisher 측)
    pub async fn complete_publisher_handshake(client: &mut WebRTCClient) -> Result<(), String> {
        client.send_ice_candidates(false).await?;
        let ice_msg = wait_for_message_type(client, ICE_CANDIDATE_ACK, 15).await?;
        client.handle_ice_candidate(ice_msg).await?;
        
        println!("✅ Publisher P2P handshake completed");
        Ok(())
    }

    /// 테스트용 헬퍼: P2P 핸드셰이크 완료 (Subscriber 측)
    pub async fn complete_subscriber_handshake(client: &mut WebRTCClient) -> Result<(), String> {
        let ice_msg = wait_for_message_type(client, ICE_CANDIDATE, 15).await?;
        client.handle_ice_candidate(ice_msg).await?;
        
        client.send_ice_candidates(true).await?;
        
        println!("✅ Subscriber P2P handshake completed");
        Ok(())
    }

    // Helper function to wait for a specific message type with timeout
    async fn wait_for_message_type(
        client: &mut WebRTCClient, 
        msg_type: &str, 
        timeout_secs: u64
    ) -> Result<WebRTCMessage, String> {
        let start = std::time::Instant::now();
        
        while start.elapsed().as_secs() < timeout_secs {
            if let Some(msg) = client.receive_message().await {
                if msg.message_type == msg_type {
                    return Ok(msg);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Err(format!("Timeout waiting for {}", msg_type))
    }
    
    /// 테스트용 헬퍼: 완전한 WebRTC 연결 설정 (시그널링 + P2P)
    pub async fn establish_complete_webrtc_connection(
        port: u32
    ) -> Result<(WebRTCClient, WebRTCClient, String, tokio::task::JoinHandle<()>), String> {
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;
        
        let addr_str = format!("ws://127.0.0.1:{}/", port);
        
        // Publisher 설정
        let mut publisher = setup_signaling_connection(&addr_str).await?;
        let session_id = setup_publisher_signaling(&mut publisher).await?;
        
        // Subscriber 설정
        let mut subscriber = setup_signaling_connection(&addr_str).await?;
        setup_subscriber_signaling(&mut subscriber, &session_id, Some(DEFAULT_DEBUG_FILE_PATH)).await?;
        
        // 3. Publisher가 answer 처리 (필수!)
        let answer_msg = wait_for_message_type(&mut publisher, ANSWER, 10).await?;
        publisher.handle_answer(answer_msg).await?;

        // P2P 핸드셰이크 (병렬 실행)
        tokio::try_join!(
            complete_publisher_handshake(&mut publisher),
            complete_subscriber_handshake(&mut subscriber)
        )?;
        
        Ok((publisher, subscriber, session_id, server_handle))
    }

    /// 테스트용 헬퍼: 커스텀 Subscriber 설정으로 연결
    pub async fn establish_webrtc_with_custom_subscriber<F>(
        port: u32,
        subscriber_setup: F
    ) -> Result<(WebRTCClient, WebRTCClient, String, tokio::task::JoinHandle<()>), String>
    where
        F: FnOnce(&mut WebRTCClient),
    {
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;
        
        let addr_str = format!("ws://127.0.0.1:{}/", port);
        
        // Publisher 설정
        let mut publisher = setup_signaling_connection(&addr_str).await?;
        let session_id = setup_publisher_signaling(&mut publisher).await?;
        
        // Subscriber 설정 (커스텀 핸들러 적용)
        let mut subscriber = setup_signaling_connection(&addr_str).await?;
        subscriber_setup(&mut subscriber); // 커스텀 설정 적용
        setup_subscriber_signaling(&mut subscriber, &session_id, Some(DEFAULT_DEBUG_FILE_PATH)).await?;
        
        // 3. Publisher가 answer 처리 (필수!)
        let answer_msg = wait_for_message_type(&mut publisher, ANSWER, 10).await?;
        publisher.handle_answer(answer_msg).await?;

        // P2P 핸드셰이크
        tokio::try_join!(
            complete_publisher_handshake(&mut publisher),
            complete_subscriber_handshake(&mut subscriber)
        )?;
        
        Ok((publisher, subscriber, session_id, server_handle))
    }

    fn run_http3_test_with_retry(url: &str, max_attempts: usize) -> (u32, String, Option<String>) {
        for attempt in 1..=max_attempts {
            println!("HTTP/3 test attempt {}/{} for {}", attempt, max_attempts, url);
            
            let client_builder = ClientBuilder::new();
            let client = client_builder.set_protocol(PROTOCOLS::HTTP3).build();
            
            let result = client.set_url(url).request();
            
            if result.status_code == 200 {
                let res_body = String::from_utf8(result.body).unwrap_or_default();
                return (result.status_code, res_body, result.error);
            }
            
            if let Some(ref error) = result.error {
                println!("Attempt {} failed: {}", attempt, error);
                
                // Don't retry on certain permanent errors
                if error.contains("DNS") || error.contains("host") || error.contains("certificate") {
                    return (result.status_code, String::new(), result.error);
                }
            }
            
            if attempt < max_attempts {
                println!("Retrying in 2 seconds...");
                std::thread::sleep(Duration::from_secs(2));
            }
        }
        
        (0, String::new(), Some("All attempts failed".to_string()))
    }

    fn ensure_http3_test_spacing() {
        let mut last_test = LAST_HTTP3_TEST.lock().unwrap();
        if let Some(last_time) = *last_test {
            let elapsed = last_time.elapsed();
            if elapsed < Duration::from_secs(5) { // 5 second spacing
                std::thread::sleep(Duration::from_secs(5) - elapsed);
            }
        }
        *last_test = Some(Instant::now());
    }

    fn is_valid_h264(data: &[u8]) -> bool {
        // Check for H264 NAL unit start codes
        for window in data.windows(4) {
            if window == [0x00, 0x00, 0x00, 0x01] {
                return true;
            }
        }
        for window in data.windows(3) {
            if window == [0x00, 0x00, 0x01] {
                return true;
            }
        }
        false
    }

    fn init_crypto() {
        CRYPTO_INIT.get_or_init(|| {
        let result = CryptoProvider::install_default(default_provider());
        assert!(result.is_ok(), "Failed to initialize crypto: {:?}", result);
    });
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

    #[test]
    fn test_build_client() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        /* Assertions */
        assert_eq!(client.protocol, PROTOCOLS::HTTP);
    }

    /* start of HTTP 1.1 tests */
    #[test]
    fn test_http_request_wrong_host_name1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("ww.w.clear.com").request();
        
        /* Assertions */
        assert!(response.error.is_some());  // wrong host name
    }

    #[test]
    fn test_http_request_wrong_host_name2() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("3.112.22.222.11").request();
                
        /* Assertions */
        assert!(response.error.is_some());  // wrong host name
    }

    #[test]
    fn test_http_request_get_with_redirection() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_follow_redirect(true)
            .set_url("http://www.rust-lang.org:80/")
            .request();
        
        /* Assertions */
        assert!(response.error.is_none());  // successful request
        assert!(response.headers.len() > 0);
        assert!(response.body.len() > 0);
        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn test_http_request_get_without_redirection() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("http://www.rust-lang.org:80/").request();
        
        /* Assertions */
        assert!(response.error.is_none());  // successful request
        assert!(response.headers.len() > 0);
        assert!(response.body.len() > 0);
        assert_ne!(response.status_code, 200);  // redirection status code
    }

    #[test]
    fn test_http_request_post_no_post_allowed() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url("http://www.rust-lang.org:80/")
                        .set_method("POST")
                        .set_follow_redirect(true)
                        .request();
        
        /* Assertions */
        assert!(response.error.is_none());  // successful request
        assert!(response.headers.len() > 0);
        assert!(response.body.len() > 0);

        // println!("return code: {}", response.status_code);
        assert_ne!(response.status_code, 200);  // no post allowed for the server
    }

    #[test]
    fn test_http_request_post_headers_only() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url(HTTP_ECHO_SERVER_URL)
                .set_req_headers(vec![("Content-Type", "application/json")])
                .set_method("POST")
                .request();
        
        /* Assertions */
        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn test_http_request_post_1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        let response = client.set_url(HTTP_ECHO_SERVER_URL)
                .set_req_headers(vec![("Content-Type", "application/json"), ("Authorization", "Bearer 123456")])
                .set_method("POST")
                .set_req_body("{}")
                .request();
        
        /* Assertions */
        assert_eq!(response.status_code, 200);
    }
    /* end of HTTP 1.1 tests */

    /* start of HTTPS tests */
    #[test]
    fn test_https_request_get() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTPS)
            .build();

        let response = client.set_url("https://github.com")
            .set_follow_redirect(true)
            .request();

        /* Assertions */
        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn test_file_download1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::FILE)
            .build();

        // private test file
        let response = client.set_url("https://files.keti-xr.duckdns.org/api/public/dl/afeLp4YK/Box.glb")
                                            .request();
        // public test file
        // let response = client.set_url("https://www.rust-lang.org/logos/rust-logo-512x512.png")
        //                                 .request();

        /* Assertions */
        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn test_coap_request_get() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::COAP)
            .build();

        let response = client.set_url("coap://coap.me:5683/test")
            .request();

        /* Assertions */
        assert_eq!(response.status_code, 69);
    }

    #[test]
    fn test_coap_request_post() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::COAP)
            .build();

        let response = client.set_url("coap://coap.me:5683/.well-known/core/test")
            .set_method("POST")
            .set_req_body("Hello, CoAP!")
            .request();

        /* Assertions */
        assert_eq!(response.status_code, 69);
    }

    #[test]
    fn test_coap_unknown_host() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::COAP)
            .build();

        let response = client.set_url("coap://coap.unknown:5683/test").request();

        /* Assertions */
        assert!(response.error.is_some());
    }

    #[test]
    fn test_ws_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::WS)
            .build();

        let response = client.set_url("https://echo.websocket.org/").connect();
        assert!(response.is_ok());
    }
    
    #[test]
    fn test_ws_send() {
        let msg = "Hello, WS";
        let data = Vec::from(msg.as_bytes());

        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::WS)
            .build();

        let connect_result = client.set_url("wss://echo.websocket.org/").connect();
        
        let send_result = connect_result.unwrap().send(data, None);
        
        assert_eq!(send_result.is_ok(), true);
        
    }

    #[test]
    fn test_ws_rcv() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::WS)
            .build();

        let connect_result = client.set_url("wss://echo.websocket.org/").connect();
        let send_result = connect_result.unwrap().send(Vec::from("Hello, WS".as_bytes()), None);
        
        let client = send_result.unwrap();
        let response = client.rcv();
        
        let response_str = String::from_utf8(response.clone().unwrap()).unwrap();
        println!("response: {}", response_str);
        assert_eq!(response.is_ok(), true);
    }

    #[test]
    fn test_ftp_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let response = client.set_url("test.rebex.net:21").connect();
        assert_eq!(response.is_ok(), true);
    }

    #[test]
    fn test_ftp_quit() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let response = client.set_url("test.rebex.net:21").connect();
        let ftp_payload = FtpPayload {
            command: FtpCommands::QUIT,
            payload_name: "".to_string(),
            payload: None,
        };

        let response = response.unwrap().run_ftp_command(ftp_payload);
        assert_eq!(response.error.is_none(), true);
    }

    #[test]
    fn test_ftp_cwd() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let response = client.set_url("test.rebex.net:21")
            .connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::CWD,
            payload_name: "pub/example".to_string(),
            payload: None,
        };
        let response = response.run_ftp_command(ftp_payload);
        
        assert_eq!(response.error.is_none(), true);
    }

    #[test]
    fn test_ftp_list() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let response = client.set_url("test.rebex.net:21")
            .connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::LIST,
            payload_name: "".to_string(),
            payload: None,
        };
        let response = response.run_ftp_command(ftp_payload);
        
        assert_eq!(response.error.is_none(), true);
    }

    #[test]
    fn test_ftp_download() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let client = client.set_url("test.rebex.net:21")
            .connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::RETR,
            payload_name: "readme.txt".to_string(),
            payload: None,
        };
        let response = client.run_ftp_command(ftp_payload);
        
        assert_eq!(response.error.is_none(), true);
        let payload_str = String::from_utf8(response.payload.clone().unwrap()).unwrap();
        println!("payload: {}", payload_str);
        assert!(response.payload.is_some());
    }

    /************************** MQTT Tests **************************/
    #[test]
    fn test_client_mqtt_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let response = client.set_url("test.mosquitto.org:1883")
            .connect();
        assert_eq!(response.is_ok(), true);
    }

    #[test]
    fn test_client_mqtt_subscribe() {
        let client_builder = ClientBuilder::new();
        let subscriber = client_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let subscriber = subscriber.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let subscriber = subscriber.mqtt_subscribe("hello/keti");
        assert_eq!(subscriber.is_ok(), true);
    }

    #[test]
    fn test_client_mqtt_publish() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let response = client.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let data: Vec<u8> = Vec::from("Hello, MQTT".as_bytes());
        let response = response.send(data, Some("hello/keti"));
        assert_eq!(response.is_ok(), true);
    }

    #[test]
    fn test_client_mqtt_sub_pub_rcv() {
        let publisher_builder = ClientBuilder::new();
        let publisher = publisher_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let subscriber_builder = ClientBuilder::new();
        let subscriber = subscriber_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let publisher = publisher.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let subscriber = subscriber.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let subscriber = subscriber.mqtt_subscribe("hello/keti");
        assert_eq!(subscriber.is_ok(), true);

        let data: Vec<u8> = Vec::from("Hello, MQTT".as_bytes());
        // publishes data to topic "hello/rumqtt"
        let publisher = publisher.send(data, Some("hello/keti"));
        assert_eq!(publisher.is_ok(), true);
        

        let mut count = 0;
        let recv_str;
        loop {
            count += 1;
            let rcv_result = subscriber.clone().unwrap().rcv();
            if rcv_result.is_ok() {
                let rcv_data = rcv_result.unwrap();
                let rcv_str = String::from_utf8(rcv_data);
                if rcv_str.is_ok() {
                    let rcv_str_unwrapped = rcv_str.unwrap();
                    println!("Received data (attempt {}): {}", count, rcv_str_unwrapped);
                    recv_str = rcv_str_unwrapped.as_str();
                    println!("Received data: {}", recv_str);
                    assert_eq!(recv_str, "Hello, MQTT");
                    break;
                } else {
                    println!("Failed to convert received data to string");
                }
            } else {
                println!("No message received yet, attempt {}", count);
                continue;
            }
        }

    }

    #[test]
    fn test_client_quic_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::QUIC)
            .build();

        let result = client.set_url("https://quic.nginx.org:443")
            .connect();

        let result = result.map_err(|e| e.to_string());
        assert_eq!(result.is_ok(), true);
    }

    #[test]
    fn test_client_quic_send() {
        let client_builder = ClientBuilder::new();
        let client: crate::client::Client = client_builder.set_protocol(PROTOCOLS::QUIC)
            .build();

        let result = client.set_url("https://quic.nginx.org:443")
            .connect().map_err(|e| e.to_string());

        let client = result.unwrap();

        let send_result = client.send(Vec::from("Hello, QUIC".as_bytes()), None);
        assert_eq!(send_result.is_ok(), true);

        assert!(true);
    }

    #[test]
    fn test_quic_rcv() {

    }

    #[test]
    #[serial]
    fn test_client_http3_request() {
        ensure_http3_test_spacing();
        
        // let (status_code, res_body, error) = run_http3_test_with_retry("https://www.litespeedtech.com/products/litespeed-web-server", 3);
        let (status_code, res_body, error) = run_http3_test_with_retry("https://turn.keti.xrds.kr", 3);

        
        println!("response body length: {}", res_body.len());
        println!("status code: {}", status_code);
        println!("error: {:?}", error);
        
        assert_eq!(status_code, 200, "HTTP/3 request failed after retries. Error: {:?}", error);
    }

    #[test]
    #[serial]
    fn test_client_http3_request_custom_header() {
        ensure_http3_test_spacing();

        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP3)
            .build();

        // These 5 fields MUST appear or it won't work
        let header = vec![
            (":method", "GET"),                             // mandatory pseudo field
            (":scheme", "https"),                           // mandatory pseudo field
            (":authority", "www.litespeedtech.com"),        // mandatory pseudo field
            (":path", "/products/litespeed-web-server"),    // mandatory pseudo field
            ("user-Agent", "PostmanRuntime/7.43.0"),   // Some http3 sites require this field
            ("accept", "*/*"),                      // custom fields
            ("accept-language", "en-US,en;q=0.9"),  // custom fields
        ];

        let result = client.set_url("https://www.litespeedtech.com/products/litespeed-web-server")
            .set_req_headers(header)
            .request();
        let res_body = String::from_utf8(result.body).unwrap();
        println!("response body length: {}", res_body.len());
        println!("status code: {}", result.status_code);
        println!("error: {:?}", result.error);
        assert_eq!(result.status_code, 200);
    }

    #[test]
    #[serial]
    fn test_http3_request_without_agent() {
        ensure_http3_test_spacing();

        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP3)
            .build();

        let header = vec![
            (":method", "GET"),
            (":scheme", "https"),
            (":authority", "cloudflare-quic.com"),
            (":path", "/"),
        ];

        let result = client.set_url("https://cloudflare-quic.com")
            .set_req_headers(header)
            .request();
        let res_body = String::from_utf8(result.body).unwrap();
        println!("response body length: {}", res_body.len());
        println!("status code: {}", result.status_code);
        println!("error: {:?}", result.error);
        assert_eq!(result.status_code, 200);
    }

    /************************** start of WebRTC tests **************************/
    #[tokio::test]
    async fn test_client_webrtc_exchange_ice_candidate() {
        init_crypto();

        let port = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;
        let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

        // 시그널링까지 완료
        // publisher는 offer 생성 후 대기
        // subscriber는 offer 수신 후 answer 전달
        let mut publisher = setup_signaling_connection(&addr_str).await.unwrap();
        let session_id = setup_publisher_signaling(&mut publisher).await.unwrap();
        let mut subscriber = setup_signaling_connection(&addr_str).await.unwrap();
        setup_subscriber_signaling(&mut subscriber, &session_id, Some(DEFAULT_DEBUG_FILE_PATH)).await.unwrap();

        // Publisher가 answer를 처리해야 함 (remote description 설정)
        let answer_msg = wait_for_message_type(&mut publisher, ANSWER, 10).await.unwrap();
        publisher.handle_answer(answer_msg).await.expect("Failed to handle answer");

        // 이제 ICE 후보 교환 가능 (양쪽 모두 remote description이 설정됨)
        publisher.send_ice_candidates(false).await.expect("Failed to send ICE candidates");
        let ice_msg = wait_for_message_type(&mut subscriber, ICE_CANDIDATE, 10).await.unwrap();
        println!("ICE candidates received: {:?}", ice_msg.ice_candidates);
        
        subscriber.handle_ice_candidate(ice_msg).await.expect("Failed to handle ICE candidate");
        subscriber.send_ice_candidates(true).await.expect("Failed to send ICE ACK");
        
        let ice_ack_msg = wait_for_message_type(&mut publisher, ICE_CANDIDATE_ACK, 10).await.unwrap();
        publisher.handle_ice_candidate(ice_ack_msg).await.expect("Failed to handle ICE ACK");

        println!("✅ ICE candidate exchange completed successfully");
        server_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_client_webrtc_send_video_file() {
        init_crypto();

        let port = line!() + 8000;
        let (mut publisher, subscriber, _session_id, server_handle) = 
            establish_complete_webrtc_connection(port).await
            .expect("Failed to establish connection");

        let sample_file_path = "samples/sample_video.h264";
        let file = std::fs::read(sample_file_path).expect("Failed to read sample file");
        
        let _ = publisher.start_streaming(Some(StreamSource::File(sample_file_path.to_string()))).await
            .expect("Failed to start file streaming");

        sleep(Duration::from_secs(120)).await;
        let video_debug_file_path = subscriber.get_debug_video_file_path().unwrap();
        let received_file = std::fs::read(video_debug_file_path)
            .expect("Failed to read received file");
        server_handle.abort();

        // Assertions
        assert!(is_valid_h264(&file), "Sent file is not valid H264");
        assert!(is_valid_h264(&received_file), "Received file is not valid H264");

        let size_ratio = (file.len() as f64) / (received_file.len() as f64);
        assert!(size_ratio > 0.9 && size_ratio < 1.1, 
            "File size mismatch: sent={}, received={}", file.len(), received_file.len());

        // Size difference is normal due to network overhead and possible keyframe differences
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_client_webrtc_webcam_video_stream() {
        std::env::set_var("RUST_LOG", "info");
        pretty_env_logger::init();
        init_crypto();

        let port = line!() + 8000;
        let (mut publisher, _subscriber, _session_id, server_handle) = 
            establish_complete_webrtc_connection(port).await
            .expect("Failed to establish WebRTC connection");

        // 연결이 완료된 상태에서 스트리밍 시작
        let _ = publisher.start_streaming(Some(StreamSource::Webcam(0))).await
            .expect("Failed to start streaming");

        sleep(Duration::from_secs(10)).await;
        publisher.stop_stream().await.expect("Failed to stop streaming");
        server_handle.abort();
    }
    
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_client_webrtc_datachannel() {
        std::env::set_var("RUST_LOG", "info");
        pretty_env_logger::init();
        init_crypto();

        let port = line!() + 8000;
        let (publisher, _subscriber, _session_id, server_handle) = 
            establish_complete_webrtc_connection(port).await
            .expect("Failed to establish WebRTC connection");

        publisher.send_data_channel_message("hello webrtc").await
            .expect("Failed to send data channel message");

        sleep(Duration::from_secs(10)).await;
        server_handle.abort();
    }
 
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_client_webrtc_custom_handler() {
        std::env::set_var("RUST_LOG", "info");
        pretty_env_logger::init();
        init_crypto();

        let port = line!() + 8000;
        let (mut publisher, _subscriber, _session_id, server_handle) = 
            establish_webrtc_with_custom_subscriber(port, |subscriber| {
                let video_processor = Arc::new(CustomVideoProcessor {});
                subscriber.register_video_handler(video_processor);
            }).await.expect("Failed to establish connection with custom handler");

        let _ = publisher.start_streaming(Some(StreamSource::Webcam(0))).await
            .expect("Failed to start streaming");

        sleep(Duration::from_secs(10)).await;
        publisher.stop_stream().await.expect("Failed to stop streaming");
        server_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_client_webrtc_custom_callback_fn() {
        std::env::set_var("RUST_LOG", "info");
        pretty_env_logger::init();
        init_crypto();

        let port = line!() + 8000;
        let (mut publisher, _subscriber, _session_id, server_handle) = 
            establish_webrtc_with_custom_subscriber(port, |subscriber| {
                subscriber.on_audio_track(|track| {
                    Box::pin(custom_audio_handler(track))
                });
            }).await.expect("Failed to establish connection");

        let _ = publisher.start_streaming(Some(StreamSource::Webcam(0))).await
            .expect("Failed to start streaming");

        sleep(Duration::from_secs(10)).await;
        publisher.stop_stream().await.expect("Failed to stop streaming");
        server_handle.abort();
    }
 }
