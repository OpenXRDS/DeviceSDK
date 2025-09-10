mod tests {
    use crate::client::ClientBuilder;
    use crate::common::enums::{PROTOCOLS, FtpCommands};
    use crate::common::data_structure::{FtpPayload, WebRTCMessage, 
        CREATE_SESSION, LIST_SESSIONS, JOIN_SESSION, LEAVE_SESSION, 
        CLOSE_SESSION, LIST_PARTICIPANTS, OFFER, ANSWER, WELCOME, ICE_CANDIDATE, ICE_CANDIDATE_ACK};
    use crate::common::{append_to_path, payload_str_to_vector_str};
    use crate::server::XRNetServer;
    use tokio::time::{sleep, Duration};
    use tokio::time::timeout;
    use crate::client::WebRTCClient;
    use rustls::crypto::{CryptoProvider, ring};
    use ring::default_provider;
    use once_cell::sync::OnceCell;
    use serial_test::serial;
    use std::time::Instant;
    use std::sync::Mutex;

    static HTTP_ECHO_SERVER_URL: &str = "https://echo.free.beeceptor.com";
    static CRYPTO_INIT: OnceCell<()> = OnceCell::new();
    static LAST_HTTP3_TEST: Mutex<Option<Instant>> = Mutex::new(None);

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

    /*
        Disabled due to hardness of testing the IP address
     */
    // #[test]
    // fn test_http_request_host_ip() {
    //     let client_builder = ClientBuilder::new();
    //     let client = client_builder.set_protocol(PROTOCOLS::HTTP)
    //         .build();

    //     let response = client.set_url("192.168.10.240")
    //                             .set_follow_redirect(true).request();
    //     println!("error: {}", response.error.as_ref().unwrap());
    //     assert!(response.error.is_none());  // successful request

    //     assert!(response.headers.len() > 0);
    //     assert!(response.body.len() > 0);
    //     assert_eq!(response.status_code, 200);
    // }

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

    #[test]
    fn test_mqtt_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let response = client.set_url("test.mosquitto.org:1883")
            .connect();
        assert_eq!(response.is_ok(), true);
    }

    #[test]
    fn test_mqtt_subscribe() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let client = client.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let result = client.mqtt_subscribe("hello/rumqtt");
        assert_eq!(result.is_ok(), true);
    }

    #[test]
    fn test_mqtt_publish() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let response = client.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let data: Vec<u8> = Vec::from("Hello, MQTT".as_bytes());
        let response = response.send(data, Some("hello/rumqtt"));
        assert_eq!(response.is_ok(), true);
    }

    #[test]
    fn test_mqtt_sub_pub_rcv() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT)
            .build();

        let client = client.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let response = client.mqtt_subscribe("hello/rumqtt");
        assert_eq!(response.is_ok(), true);

        let data: Vec<u8> = Vec::from("Hello, MQTT".as_bytes());
        let response = response.unwrap().send(data, Some("hello/rumqtt"));
        assert_eq!(response.is_ok(), true);

        let mut count = 0;
        loop {
            count += 1;
            let rcv_result = response.clone().unwrap().rcv();
            if rcv_result.is_ok() {
                let rcv_data = rcv_result.unwrap();
                let rcv_str = String::from_utf8(rcv_data).unwrap();
                
                if !rcv_str.is_empty() {
                    println!("rcv: {}", rcv_str);
                    assert_eq!(rcv_str.as_str(), "Hello, MQTT");
                    break;
                }
            } else {
                if count > 10 {
                    break;
                }
            }
        }
    }

    #[test]
    fn test_quic_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::QUIC)
            .build();

        let result = client.set_url("https://quic.nginx.org:443")
            .connect();

        let result = result.map_err(|e| e.to_string());
        assert_eq!(result.is_ok(), true);
    }

    #[test]
    fn test_quic_send() {
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
    fn test_http3_request() {
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
    fn test_http3_request_custom_header() {
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

    #[tokio::test]
    async fn test_client_exchange_ice_candidate() {
        init_crypto();

        let port = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;

        let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

        let mut publisher = WebRTCClient::new();
        publisher.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (_, publisher) = wait_for_message(publisher, WELCOME, 2).await;
        
        let publisher = publisher.create_session().await.expect("Failed to create session");
        let (msg, publisher) = wait_for_message(publisher, CREATE_SESSION, 5).await;

        let session_id = msg.session_id;
        println!("Test: session_id created: {}", session_id);
        
        let mut publisher = publisher;
        publisher.publish(&session_id).await.expect("Failed to publish");
        let (_, publisher) = wait_for_message(publisher, OFFER, 5).await;
        // println!("Test: publish_result received: {:?}", publish_result.sdp); // sdp is supposed to be None for this test

        // subscriber joins the session
        let mut subscriber = WebRTCClient::new();
        subscriber.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (msg, subscriber) = wait_for_message(subscriber, WELCOME, 2).await;
        let _client_id = msg.client_id;
        // println!("Test: client_id received: {}", client_id);
        
        let subscriber = subscriber.join_session(&session_id).await.expect("Failed to join session");
        let (join_result, subscriber) = wait_for_message(subscriber, JOIN_SESSION, 5).await;
        // println!("Test: join_result received: {:?}", join_result.sdp); // sdp is supposed to be None for this test
        
        let mut subscriber = subscriber;
        subscriber.handle_offer(join_result.sdp.unwrap()).await.expect("Failed to handle offer");
        
        // println!("Test: answer_result received: {:?}", answer_result.sdp); // sdp is supposed to be None for this test

        let (offer_result, mut publisher) = wait_for_message(publisher, ANSWER, 5).await;
        publisher.handle_answer(offer_result).await.expect("Failed to handle answer");

        publisher.send_ice_candidates(false).await.expect("Failed to send ICE candidates");

        let (msg, mut subscriber) = wait_for_message(subscriber, ICE_CANDIDATE, 5).await;
        println!("Test: ICE candidate received: {:?}", msg.ice_candidates);
        subscriber.handle_ice_candidate(msg).await.expect("Failed to handle ICE candidate");

        subscriber.send_ice_candidates(true).await.expect("Failed to send ICE candidates");
        
        let (msg, mut publisher) = wait_for_message(publisher, ICE_CANDIDATE_ACK, 5).await;
        println!("Test: ICE candidate ACK received: {:?}", msg.ice_candidates);
        publisher.handle_ice_candidate(msg).await.expect("Failed to handle ICE candidate ACK");


        server_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_client_webrtc_send_video_file() {
        init_crypto();

        let port = line!() + 8000;
        let server_handle = run_server(PROTOCOLS::WEBRTC, port);
        sleep(Duration::from_secs(2)).await;

        let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

        let mut publisher = WebRTCClient::new();
        publisher.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (_msg, publisher) = wait_for_message(publisher, WELCOME, 2).await;
        
        let publisher = publisher.create_session().await.expect("Failed to create session");
        let (msg, publisher) = wait_for_message(publisher, CREATE_SESSION, 5).await;

        let session_id = msg.session_id;
        println!("Test: session_id created: {}", session_id);
        
        let mut publisher = publisher;
        publisher.publish(&session_id).await.expect("Failed to publish");   // includes creating offer
        let (_publish_result, publisher) = wait_for_message(publisher, OFFER, 5).await;
        println!("Test: publish_result received: {:?}", _publish_result.sdp); // sdp is supposed to be None for this test

        // subscriber joins the session
        let mut subscriber = WebRTCClient::new();
        subscriber.set_debug_file_path("test_output").await.expect("Failed to set debug file path");
        subscriber.connect(addr_str.as_str()).await.expect("Failed to connect");

        let (msg, subscriber) = wait_for_message(subscriber, WELCOME, 2).await;
        let _client_id = msg.client_id;
        // println!("Test: client_id received: {}", client_id);
        
        let subscriber = subscriber.join_session(&session_id).await.expect("Failed to join session");
        let (join_result, subscriber) = wait_for_message(subscriber, JOIN_SESSION, 5).await;
        // println!("Test: join_result received: {:?}", join_result.sdp); // sdp is supposed to be None for this test
        
        let mut subscriber = subscriber;
        subscriber.handle_offer(join_result.sdp.unwrap()).await.expect("Failed to handle offer");

        let (_answer_result, subscriber) = wait_for_message(subscriber, ANSWER, 5).await;
        // println!("Test: answer_result received: {:?}", answer_result.sdp); // sdp is supposed to be None for this test

        let (offer_result, mut publisher) = wait_for_message(publisher, ANSWER, 5).await;
        publisher.handle_answer(offer_result).await.expect("Failed to handle answer");

        publisher.send_ice_candidates(false).await.expect("Failed to send ICE candidates");

        let (msg, mut subscriber) = wait_for_message(subscriber, ICE_CANDIDATE, 10).await;
        println!("Test: ICE candidate received: {:?}", msg.ice_candidates);
        subscriber.handle_ice_candidate(msg).await.expect("Failed to handle ICE candidate");

        subscriber.send_ice_candidates(true).await.expect("Failed to send ICE candidates");
        
        let (msg, mut publisher) = wait_for_message(publisher, ICE_CANDIDATE_ACK, 10).await;
        println!("Test: ICE candidate ACK received: {:?}", msg.ice_candidates);
        publisher.handle_ice_candidate(msg).await.expect("Failed to handle ICE candidate ACK");

        // let sample_file_path = "samples/tsm_1080p.mp4";
        let sample_file_path = "samples/sample_video.h264";
        // try open the file
        let file = std::fs::read(sample_file_path).expect("Failed to open sample video file");
        let _ = publisher.start_streaming(Some(sample_file_path)).await.expect("Failed to start streaming");

        // wait till the video file is sent
        sleep(Duration::from_secs(120)).await;

        let received_file = std::fs::read("test_output/received.h264").expect("Failed to open received file");
        server_handle.abort();

        assert!(is_valid_h264(&file), "Sent file is not a valid H264 file");
        assert!(is_valid_h264(&received_file), "Received file is not a valid H264 file");

        let size_ratio = (file.len() as f64) / (received_file.len() as f64);
        assert!(size_ratio > 0.9 && size_ratio < 1.1, "Sent file size {} is different from received file size {}", file.len(), received_file.len());
        println!("Sent file size: {}, Received file size: {}, Size ratio: {}", file.len(), received_file.len(), size_ratio);

        // Size difference is normal due to network overhead and possible keyframe differences
    }
 }

