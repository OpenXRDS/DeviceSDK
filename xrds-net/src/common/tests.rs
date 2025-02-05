mod tests {
    use crate::common::parse_url;

    #[test]
    fn url_validation_test1() {
        let http_url_1 = "http://www.rust-lang.org";
        let parsed_url_1 = parse_url(http_url_1);
        
        assert_eq!(parsed_url_1.is_ok(), true);
        
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
        assert_eq!(parsed_url_2.is_ok(), true);
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
        assert_eq!(parsed_url_3.is_ok(), true);
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
        assert_eq!(parsed_url_4.is_ok(), true);
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
        assert_eq!(parsed_url_5.is_ok(), true);
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
        assert_eq!(parsed_url_6.is_ok(), true);
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

        assert_eq!(parsed_url_7.is_err(), true);
    }

    #[test]
    fn url_validation_test8() { // port range check
        let http_url_9 = "http://www.rust-lang.org:65536";
        let parsed_url_9 = parse_url(http_url_9);
        // let parsed_url_9 = parsed_url_9.unwrap();
        // println!("parsed_url_9.port: {}", parsed_url_9.port);
        assert_eq!(parsed_url_9.is_err(), true);
    }

    #[test]
    fn url_validation_test_valid_query_params1() { // GET query parsing
        let http_url_8 = "http://echo.free.beeceptor.com?name=John&age=30";
        let parsed_url_8 = parse_url(http_url_8);

        assert_eq!(parsed_url_8.is_ok(), true);

        let parsed_url_8 = parsed_url_8.unwrap();

        assert_eq!(parsed_url_8.scheme, "http");
        assert_eq!(parsed_url_8.host, "echo.free.beeceptor.com");
        assert_eq!(parsed_url_8.port, 80);
        assert_eq!(parsed_url_8.path, "/");
        assert_eq!(parsed_url_8.query.unwrap(), "name=John&age=30");
    }

    #[test]
    fn url_validation_test_valid_query_params2() {
        let http_url_10 = "http://echo.free.beeceptor.com:80/?name=John&age=30";
        let parsed_url_10 = parse_url(http_url_10);
        
        assert_eq!(parsed_url_10.is_ok(), true);
        
        let parsed_url_10 = parsed_url_10.unwrap();

        assert_eq!(parsed_url_10.scheme, "http");
        assert_eq!(parsed_url_10.host, "echo.free.beeceptor.com");
        assert_eq!(parsed_url_10.port, 80);
        assert_eq!(parsed_url_10.path, "/");
        assert_eq!(parsed_url_10.query.unwrap(), "name=John&age=30");
    }

    #[test]
    fn url_validation_test_invalid_query_params1() {
        let http_url_11 = "http://www.rust-lang.org/??";
        let parsed_url_11 = parse_url(http_url_11);
        assert_eq!(parsed_url_11.is_err(), true);
    }
}