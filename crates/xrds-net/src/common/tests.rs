// This file is already `common::tests` (via `mod tests;` in common/mod.rs);
// the inner `mod tests` below nests it one level further rather than
// actually renaming anything — harmless, not worth re-indenting the whole
// file to flatten.
#[allow(clippy::module_inception)]
mod tests {
    use crate::common::parse_url;

    #[test]
    fn url_validation_test1() {
        let http_url_1 = "http://www.rust-lang.org";
        let parsed_url_1 = parse_url(http_url_1);

        assert!(parsed_url_1.is_ok());

        let parsed_url_1 = parsed_url_1.unwrap();

        assert_eq!(parsed_url_1.scheme, "http");
        assert_eq!(parsed_url_1.host, "www.rust-lang.org");
        assert_eq!(parsed_url_1.port, 80);
        assert_eq!(parsed_url_1.path, "/");
    }

    #[test]
    fn url_validation_test2() {
        let http_url_2 = "http://www.rust-lang.org:80";
        let parsed_url_2 = parse_url(http_url_2);
        assert!(parsed_url_2.is_ok());
        let parsed_url_2 = parsed_url_2.unwrap();

        assert_eq!(parsed_url_2.scheme, "http");
        assert_eq!(parsed_url_2.host, "www.rust-lang.org");
        assert_eq!(parsed_url_2.port, 80);
        assert_eq!(parsed_url_2.path, "/");
    }

    #[test]
    fn url_validation_test3() {
        let http_url_3 = "www.rust-lang.org";
        let parsed_url_3 = parse_url(http_url_3);
        assert!(parsed_url_3.is_ok());
        let parsed_url_3 = parsed_url_3.unwrap();

        assert_eq!(parsed_url_3.scheme, "");
        assert_eq!(parsed_url_3.host, "www.rust-lang.org");
        assert_eq!(parsed_url_3.port, 80);
        assert_eq!(parsed_url_3.path, "/");
    }

    #[test]
    fn url_validation_test4() {
        let http_url_4 = "http://www.rust-lang.org:80/path/to/resource";
        let parsed_url_4 = parse_url(http_url_4);
        assert!(parsed_url_4.is_ok());
        let parsed_url_4 = parsed_url_4.unwrap();

        assert_eq!(parsed_url_4.scheme, "http");
        assert_eq!(parsed_url_4.host, "www.rust-lang.org");
        assert_eq!(parsed_url_4.port, 80);
        assert_eq!(parsed_url_4.path, "/path/to/resource");
    }

    #[test]
    fn url_validation_test5() {
        let http_url_5 = "naver.com";
        let parsed_url_5 = parse_url(http_url_5);
        assert!(parsed_url_5.is_ok());
        let parsed_url_5 = parsed_url_5.unwrap();

        assert_eq!(parsed_url_5.scheme, "");
        assert_eq!(parsed_url_5.host, "naver.com");
        assert_eq!(parsed_url_5.port, 80);
        assert_eq!(parsed_url_5.path, "/");
    }

    #[test]
    fn url_validation_test6() {
        let http_url_6 = "http://www.rust-lang.org/";
        let parsed_url_6 = parse_url(http_url_6);
        assert!(parsed_url_6.is_ok());
        let parsed_url_6 = parsed_url_6.unwrap();

        assert_eq!(parsed_url_6.scheme, "http");
        assert_eq!(parsed_url_6.host, "www.rust-lang.org");
        assert_eq!(parsed_url_6.port, 80);
        assert_eq!(parsed_url_6.path, "/");
        assert_eq!(parsed_url_6.query, None);
    }

    #[test]
    fn url_validation_test7() {
        let http_url_7 = "://www.rust-lang.org";
        let parsed_url_7 = parse_url(http_url_7);

        assert!(parsed_url_7.is_err());
    }

    #[test]
    fn url_validation_test8() {
        // port range check
        let http_url_9 = "http://www.rust-lang.org:65536";
        let parsed_url_9 = parse_url(http_url_9);
        // let parsed_url_9 = parsed_url_9.unwrap();
        // println!("parsed_url_9.port: {}", parsed_url_9.port);
        assert!(parsed_url_9.is_err());
    }

    #[test]
    fn default_port_per_scheme_when_omitted() {
        let cases = [
            ("http://example.com", 80),
            ("https://example.com", 443),
            ("ws://example.com", 80),
            ("wss://example.com", 443),
            ("ftp://example.com", 21),
            ("sftp://example.com", 22),
            ("mqtt://example.com", 1883),
            ("coap://example.com", 5683),
            ("quic://example.com", 443),
            ("example.com", 80), // scheme-less falls back to 80
        ];
        for (url, expected) in cases {
            let parsed = parse_url(url).expect("should parse");
            assert_eq!(parsed.port, expected, "url: {url}");
        }
    }

    #[test]
    fn explicit_port_overrides_scheme_default() {
        let parsed = parse_url("mqtt://example.com:9999/topic").expect("should parse");
        assert_eq!(parsed.port, 9999);
        // and the path/userinfo still parse alongside the explicit port
        assert_eq!(parsed.path, "/topic");
    }
}
