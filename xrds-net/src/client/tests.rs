mod tests {
    use crate::client::ClientBuilder;
    use crate::common::enums::{PROTOCOLS, FtpCommands};
    use crate::common::data_structure::FtpPayload;

    static HTTP_ECHO_SERVER_URL: &str = "https://echo.free.beeceptor.com";

    #[test]
    fn test_build_client() {
        let client_builder = ClientBuilder::new();
        let client = client_builder.set_protocol(PROTOCOLS::HTTP)
            .build();

        /* Assertions */
        assert_eq!(client.get_protocol(), &PROTOCOLS::HTTP);
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
        
        let response = send_result.unwrap().rcv();
        
        let response_str = String::from_utf8(response.clone().unwrap()).unwrap();
        println!("respnse: {}", response_str);
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

        let response = client.set_url("test.rebex.net:21")
            .connect().unwrap();

        let ftp_payload = FtpPayload {
            command: FtpCommands::RETR,
            payload_name: "readme.txt".to_string(),
            payload: None,
        };
        let response = response.run_ftp_command(ftp_payload);
        
        assert_eq!(response.error.is_none(), true);
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

        let response = client.set_url("test.mosquitto.org:1883")
            .connect().unwrap();

        let response = response.mqtt_subscribe("hello/rumqtt");
        assert_eq!(response.is_ok(), true);
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
 }

