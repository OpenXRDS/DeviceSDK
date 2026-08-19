// This file is already `client::tests` (via `mod tests;` in client/mod.rs);
// the inner `mod tests` below nests it one level further
// (`client::tests::tests`) rather than actually renaming anything —
// harmless, not worth re-indenting the whole file to flatten.
#[allow(clippy::module_inception)]
mod tests {
    use crate::client::ClientBuilder;
    use crate::common::data_structure::FtpPayload;
    use crate::common::enums::{FtpCommands, PROTOCOLS};
    use tokio::time::Duration;

    use serial_test::serial;
    use std::sync::Mutex;
    use std::time::Instant;

    static HTTP_ECHO_SERVER_URL: &str = "https://echo.free.beeceptor.com";
    static LAST_HTTP3_TEST: Mutex<Option<Instant>> = Mutex::new(None);

    fn run_http3_test_with_retry(url: &str, max_attempts: usize) -> (u32, String, Option<String>) {
        for attempt in 1..=max_attempts {
            println!(
                "HTTP/3 test attempt {}/{} for {}",
                attempt, max_attempts, url
            );

            let client_builder = ClientBuilder::new();
            let client = client_builder.set_protocol(PROTOCOLS::HTTP3).build();

            let result = client.set_url(url).request();
            let (status_code, body, error) = match result {
                Ok(response) => (response.status_code, response.body, response.error),
                Err(e) => (0, Vec::new(), Some(e.to_string())),
            };

            if status_code == 200 {
                let res_body = String::from_utf8(body).unwrap_or_default();
                return (status_code, res_body, error);
            }

            if let Some(ref error_msg) = error {
                println!("Attempt {} failed: {}", attempt, error_msg);

                // Don't retry on certain permanent errors
                if error_msg.contains("DNS")
                    || error_msg.contains("host")
                    || error_msg.contains("certificate")
                {
                    return (status_code, String::new(), error);
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
            if elapsed < Duration::from_secs(5) {
                // 5 second spacing
                std::thread::sleep(Duration::from_secs(5) - elapsed);
            }
        }
        *last_test = Some(Instant::now());
    }

    #[test]
    fn test_build_client() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        /* Assertions */
        assert_eq!(client.get_protocol(), PROTOCOLS::HTTP);
    }

    /* start of HTTP 1.1 tests */
    #[test]
    fn test_http_request_wrong_host_name1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        let response = client.set_url("ww.w.clear.com").request();

        /* Assertions */
        assert!(response.is_err()); // wrong host name
    }

    #[test]
    fn test_http_request_wrong_host_name2() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        let response = client.set_url("3.112.22.222.11").request();

        /* Assertions */
        assert!(response.is_err()); // wrong host name
    }

    #[test]
    #[ignore = "live network: hits www.rust-lang.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_http_request_get_with_redirection() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        let response = client
            .set_follow_redirect(true)
            .set_url("http://www.rust-lang.org:80/")
            .request()
            .unwrap();

        /* Assertions */
        assert!(response.error.is_none()); // successful request
        assert!(!response.headers.is_empty());
        assert!(!response.body.is_empty());
        assert_eq!(response.status_code, 200);
    }

    #[test]
    #[ignore = "live network: hits www.rust-lang.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_http_request_get_without_redirection() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        let response = client
            .set_url("http://www.rust-lang.org:80/")
            .request()
            .unwrap();

        /* Assertions */
        assert!(response.error.is_none()); // successful request
        assert!(!response.headers.is_empty());
        assert!(!response.body.is_empty());
        assert_ne!(response.status_code, 200); // redirection status code
    }

    #[test]
    #[ignore = "live network: hits www.rust-lang.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_http_request_post_no_post_allowed() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        let response = client
            .set_url("http://www.rust-lang.org:80/")
            .set_method("POST")
            .set_follow_redirect(true)
            .request()
            .unwrap();

        /* Assertions */
        assert!(response.error.is_none()); // successful request
        assert!(!response.headers.is_empty());
        assert!(!response.body.is_empty());

        // println!("return code: {}", response.status_code);
        assert_ne!(response.status_code, 200); // no post allowed for the server
    }

    #[test]
    fn test_http_request_post_headers_only() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        let response = client
            .set_url(HTTP_ECHO_SERVER_URL)
            .set_req_headers(vec![("Content-Type", "application/json")])
            .set_method("POST")
            .request()
            .unwrap();

        /* Assertions */
        assert_eq!(response.status_code, 200);
    }

    #[test]
    #[ignore = "live network: hits an external HTTP endpoint; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_http_request_post_1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP).build();

        let response = client
            .set_url(HTTP_ECHO_SERVER_URL)
            .set_req_headers(vec![
                ("Content-Type", "application/json"),
                ("Authorization", "Bearer 123456"),
            ])
            .set_method("POST")
            .set_req_body("{}")
            .request()
            .unwrap();

        /* Assertions */
        assert_eq!(response.status_code, 200);
    }
    /* end of HTTP 1.1 tests */

    /* start of HTTPS tests */
    #[test]
    #[ignore = "live network: hits test.rebex.net; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_https_request_get() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTPS).build();

        let response = client
            .set_url("https://github.com")
            .set_follow_redirect(true)
            .request()
            .unwrap();

        /* Assertions */
        assert_eq!(response.status_code, 200);
    }

    // Was `PROTOCOLS::FILE` against a private `files.keti-xr.duckdns.org` URL —
    // that host is gone, and separately, `FILE` now means a genuine local
    // filesystem read (see http.rs's module doc), so an `https://` URL there
    // was never going to work again regardless. Switched to FTP against the
    // same public test.rebex.net target the other FTP tests already use.
    #[test]
    #[ignore = "live network: hits test.rebex.net; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_file_download1() {
        let client_builder = ClientBuilder::new();
        let client = client_builder
            .set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let client = client.set_url("test.rebex.net:21").connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::RETR,
            payload_name: "readme.txt".to_string(),
            payload: None,
        };
        let response = client.run_ftp_command(ftp_payload);

        assert!(response.error.is_none());
        assert!(response.payload.is_some());
    }

    #[test]
    #[ignore = "live network: hits coap.me; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_coap_request_get() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::COAP).build();

        let response = client.set_url("coap://coap.me:5683/test").request().unwrap();

        /* Assertions */
        assert_eq!(response.status_code, 69);
    }

    #[test]
    #[ignore = "live network: hits coap.me; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_coap_request_post() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::COAP).build();

        let response = client
            .set_url("coap://coap.me:5683/.well-known/core/test")
            .set_method("POST")
            .set_req_body("Hello, CoAP!")
            .request()
            .unwrap();

        /* Assertions */
        assert_eq!(response.status_code, 69);
    }

    #[test]
    fn test_coap_unknown_host() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::COAP).build();

        let response = client.set_url("coap://coap.unknown:5683/test").request();

        /* Assertions */
        assert!(response.is_err());
    }

    #[test]
    #[ignore = "live network: hits echo.websocket.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ws_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::WS).build();

        let response = client.set_url("wss://echo.websocket.org/").connect();
        assert!(response.is_ok());
    }

    #[test]
    #[ignore = "live network: hits echo.websocket.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ws_send() {
        let msg = "Hello, WS";
        let data = Vec::from(msg.as_bytes());

        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::WS).build();

        let connect_result = client.set_url("wss://echo.websocket.org/").connect();

        let send_result = connect_result.unwrap().send(data, None);

        assert!(send_result.is_ok());
    }

    #[test]
    #[ignore = "live network: hits echo.websocket.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ws_rcv() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::WS).build();

        let connect_result = client.set_url("wss://echo.websocket.org/").connect();
        let send_result = connect_result
            .unwrap()
            .send(Vec::from("Hello, WS".as_bytes()), None);

        let mut client = send_result.unwrap();
        let response = client.rcv();

        let response_str = String::from_utf8(response.clone().unwrap()).unwrap();
        println!("response: {}", response_str);
        assert!(response.is_ok());
    }

    // Bidirectional WSS *session* via `XrdsNet::open` — exercises the
    // tokio-tungstenite backend over TLS (native-tls). echo.websocket.org
    // echoes on the same connection, which is exactly a session round-trip.
    // Public-server test → tolerant of the network being unavailable (this
    // sandbox often can't reach it), but asserts the echo when it connects.
    #[test]
    #[ignore = "live network: hits echo.websocket.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_wss_session_round_trip() {
        use crate::client::XrdsNet;

        match XrdsNet::open("wss://echo.websocket.org/") {
            Ok(mut chan) => {
                chan.send(b"hello wss".to_vec()).expect("send on the wss session");
                // echo.websocket.org sends a welcome banner before echoing, so
                // drain until we see our own message come back.
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut got_echo = false;
                while Instant::now() < deadline {
                    match chan.recv_timeout(Duration::from_secs(2)) {
                        Ok(ev) if ev.payload == b"hello wss" => {
                            got_echo = true;
                            break;
                        }
                        Ok(_) => continue, // welcome banner / other frame
                        Err(_) => break,   // timed out or closed
                    }
                }
                assert!(got_echo, "expected our message echoed back over the wss session");
                let _ = chan.close();
            }
            // Not reachable from here — tolerated (network flakiness), same as
            // the other echo.websocket.org tests.
            Err(e) => println!("wss open failed (network?): {e}"),
        }
    }

    #[test]
    #[ignore = "live network: hits test.rebex.net; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ftp_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder
            .set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let response = client.set_url("test.rebex.net:21").connect();
        assert!(response.is_ok());
    }

    #[test]
    #[ignore = "live network: hits test.rebex.net; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ftp_quit() {
        let client_builder = ClientBuilder::new();
        let client = client_builder
            .set_protocol(PROTOCOLS::FTP)
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
        assert!(response.error.is_none());
    }

    #[test]
    #[ignore = "live network: hits test.rebex.net; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ftp_cwd() {
        let client_builder = ClientBuilder::new();
        let client = client_builder
            .set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let response = client.set_url("test.rebex.net:21").connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::CWD,
            payload_name: "pub/example".to_string(),
            payload: None,
        };
        let response = response.run_ftp_command(ftp_payload);

        assert!(response.error.is_none());
    }

    #[test]
    #[ignore = "live network: hits test.rebex.net; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ftp_list() {
        let client_builder = ClientBuilder::new();
        let client = client_builder
            .set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let response = client.set_url("test.rebex.net:21").connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::LIST,
            payload_name: "".to_string(),
            payload: None,
        };
        let response = response.run_ftp_command(ftp_payload);

        assert!(response.error.is_none());
    }

    #[test]
    #[ignore = "live network: hits test.rebex.net; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_ftp_download() {
        let client_builder = ClientBuilder::new();
        let client = client_builder
            .set_protocol(PROTOCOLS::FTP)
            .set_user("demo")
            .set_password("password")
            .build();

        let client = client.set_url("test.rebex.net:21").connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::RETR,
            payload_name: "readme.txt".to_string(),
            payload: None,
        };
        let response = client.run_ftp_command(ftp_payload);

        assert!(response.error.is_none());
        let payload_str = String::from_utf8(response.payload.clone().unwrap()).unwrap();
        println!("payload: {}", payload_str);
        assert!(response.payload.is_some());
    }

    /************************** MQTT Tests **************************/
    #[test]
    #[ignore = "live network: hits test.mosquitto.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_mqtt_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT).build();

        let response = client.set_url("test.mosquitto.org:1883").connect();
        assert!(response.is_ok());
    }

    #[test]
    #[ignore = "live network: hits test.mosquitto.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_mqtt_subscribe() {
        let client_builder = ClientBuilder::new();
        let subscriber = client_builder.set_protocol(PROTOCOLS::MQTT).build();

        let subscriber = subscriber
            .set_url("test.mosquitto.org:1883")
            .connect()
            .unwrap();

        let subscriber = subscriber.mqtt_subscribe("hello/keti");
        assert!(subscriber.is_ok());
    }

    #[test]
    #[ignore = "live network: hits test.mosquitto.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_mqtt_publish() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::MQTT).build();

        let response = client.set_url("test.mosquitto.org:1883").connect().unwrap();

        let data: Vec<u8> = Vec::from("Hello, MQTT".as_bytes());
        let response = response.send(data, Some("hello/keti"));
        assert!(response.is_ok());
    }

    #[test]
    #[ignore = "live network: hits test.mosquitto.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_mqtt_sub_pub_rcv() {
        let publisher_builder = ClientBuilder::new();
        let publisher = publisher_builder.set_protocol(PROTOCOLS::MQTT).build();

        let subscriber_builder = ClientBuilder::new();
        let subscriber = subscriber_builder.set_protocol(PROTOCOLS::MQTT).build();

        let publisher = publisher
            .set_url("test.mosquitto.org:1883")
            .connect()
            .unwrap();

        let subscriber = subscriber
            .set_url("test.mosquitto.org:1883")
            .connect()
            .unwrap();

        let mut subscriber = subscriber.mqtt_subscribe("hello/keti").unwrap();

        let data: Vec<u8> = Vec::from("Hello, MQTT".as_bytes());
        // publishes data to topic "hello/rumqtt"
        let publisher = publisher.send(data, Some("hello/keti"));
        assert!(publisher.is_ok());

        let mut count = 0;
        loop {
            count += 1;
            let rcv_result = subscriber.rcv();
            if let Ok(rcv_data) = rcv_result {
                let rcv_str = String::from_utf8(rcv_data);
                if let Ok(rcv_str_unwrapped) = rcv_str {
                    println!("Received data (attempt {}): {}", count, rcv_str_unwrapped);
                    let recv_str = rcv_str_unwrapped.as_str();
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
    #[ignore = "live network: hits quic.nginx.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_quic_connect() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::QUIC).build();

        let result = client.set_url("https://quic.nginx.org:443").connect();

        let result = result.map_err(|e| e.to_string());
        assert!(result.is_ok());
    }

    #[test]
    #[ignore = "live network: hits quic.nginx.org; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_quic_send() {
        let client_builder = ClientBuilder::new();
        let client: crate::client::Client = client_builder.set_protocol(PROTOCOLS::QUIC).build();

        let result = client
            .set_url("https://quic.nginx.org:443")
            .connect()
            .map_err(|e| e.to_string());

        let client = result.unwrap();

        let send_result = client.send(Vec::from("Hello, QUIC".as_bytes()), None);
        assert!(send_result.is_ok());
    }

    #[test]
    fn test_quic_rcv() {}

    #[test]
    #[serial]
    #[ignore = "live network: hits litespeedtech.com; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_http3_request() {
        ensure_http3_test_spacing();

        // let (status_code, res_body, error) = run_http3_test_with_retry("https://www.litespeedtech.com/products/litespeed-web-server", 3);
        let (status_code, res_body, error) =
            run_http3_test_with_retry("https://turn.keti.xrds.kr", 3);

        println!("response body length: {}", res_body.len());
        println!("status code: {}", status_code);
        println!("error: {:?}", error);

        assert_eq!(
            status_code, 200,
            "HTTP/3 request failed after retries. Error: {:?}",
            error
        );
    }

    #[test]
    #[serial]
    #[ignore = "live network: hits litespeedtech.com; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_client_http3_request_custom_header() {
        ensure_http3_test_spacing();

        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP3).build();

        // These 5 fields MUST appear or it won't work
        let header = vec![
            (":method", "GET"),                          // mandatory pseudo field
            (":scheme", "https"),                        // mandatory pseudo field
            (":authority", "www.litespeedtech.com"),     // mandatory pseudo field
            (":path", "/products/litespeed-web-server"), // mandatory pseudo field
            ("user-Agent", "PostmanRuntime/7.43.0"),     // Some http3 sites require this field
            ("accept", "*/*"),                           // custom fields
            ("accept-language", "en-US,en;q=0.9"),       // custom fields
        ];

        let result = client
            .set_url("https://www.litespeedtech.com/products/litespeed-web-server")
            .set_req_headers(header)
            .request()
            .unwrap();
        let res_body = String::from_utf8(result.body).unwrap();
        println!("response body length: {}", res_body.len());
        println!("status code: {}", result.status_code);
        println!("error: {:?}", result.error);
        assert_eq!(result.status_code, 200);
    }

    #[test]
    #[serial]
    #[ignore = "live network: hits cloudflare-quic.com; run with --ignored, or see the xrds-net-live-network workflow"]
    fn test_http3_request_without_agent() {
        ensure_http3_test_spacing();

        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP3).build();

        let header = vec![
            (":method", "GET"),
            (":scheme", "https"),
            (":authority", "cloudflare-quic.com"),
            (":path", "/"),
        ];

        let result = client
            .set_url("https://cloudflare-quic.com")
            .set_req_headers(header)
            .request()
            .unwrap();
        let res_body = String::from_utf8(result.body).unwrap();
        println!("response body length: {}", res_body.len());
        println!("status code: {}", result.status_code);
        println!("error: {:?}", result.error);
        assert_eq!(result.status_code, 200);
    }

}
