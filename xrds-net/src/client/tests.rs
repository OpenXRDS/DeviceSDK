mod tests {
    use crate::client::{Client, ClientBuilder};
    use crate::common::data_structure::NetResponse;
    use crate::common::enums::PROTOCOLS;

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

        let response = client.set_url("coap://coap.unknown:5683/test")
            .request();

        /* Assertions */
        assert!(response.error.is_some());
    }
 }