pub mod enums;
pub mod data_structure;

use crate::common::data_structure::Url;

pub fn parse_url(url: &str) -> Result<Url, String> {
    
    // 1. separate scheme if it exists
    let scheme_tokens = url.split("://").collect::<Vec<&str>>(); // ["https", "www.google.com/search"]
    if scheme_tokens.len() > 2 {
        return Err("Invalid scheme structure".to_string());
    }

    let scheme = if scheme_tokens.len() == 2 {
        if scheme_tokens[0].is_empty() {
            return Err("Invalid scheme".to_string());
        }
        scheme_tokens[0]
    } else {
        ""
    };
    // 2. separate host
    let mut host  = scheme_tokens[scheme_tokens.len() - 1];
    let mut host_tokens = host.split("/").collect::<Vec<&str>>();   // host / path
    
    host = host_tokens[0];
    let path = if host_tokens.len() > 1 {
        // path starts with "/"
        "/".to_string() + &host_tokens[1..].join("/")
    } else {
        "/".to_string()
    };
    
    // 3. separate port
    let mut port = 80;
    host_tokens = host.split(":").collect::<Vec<&str>>();
    if host_tokens.len() == 2 {
        host = host_tokens[0];
        port = host_tokens[1].parse::<u32>().unwrap();
    }
    
    //TODO: query, username, password

    Ok(Url {   // temporal return value
        shceme: scheme.to_string(),
        host: host.to_string(),
        port: port,
        path: path.to_string(),
        
        query: None,
        username: None,
        password: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::common::parse_url;

    #[test]
    fn url_validation_test() {
        let http_url_1 = "http://www.rust-lang.org";
        let http_url_2 = "http://www.rust-lang.org:80";
        let http_url_3 = "www.rust-lang.org";
        let http_url_4 = "http://www.rust-lang.org:80/path/to/resource";
        let http_url_5 = "naver.com";
        let http_url_6 = "http://www.rust-lang.org/";
        let http_url_7 = "://www.rust-lang.org";

        let parsed_url_1 = parse_url(http_url_1);
        let parsed_url_2 = parse_url(http_url_2);
        let parsed_url_3 = parse_url(http_url_3);
        let parsed_url_4 = parse_url(http_url_4);
        let parsed_url_5 = parse_url(http_url_5);
        let parsed_url_6 = parse_url(http_url_6);
        let parsed_url_7 = parse_url(http_url_7);

        assert_eq!(parsed_url_1.is_ok(), true);
        assert_eq!(parsed_url_2.is_ok(), true);
        assert_eq!(parsed_url_3.is_ok(), true);
        assert_eq!(parsed_url_4.is_ok(), true);
        assert_eq!(parsed_url_5.is_ok(), true);
        assert_eq!(parsed_url_6.is_ok(), true);
        assert_eq!(parsed_url_7.is_err(), true);

        let parsed_url_1 = parsed_url_1.unwrap();
        let parsed_url_2 = parsed_url_2.unwrap();
        let parsed_url_3 = parsed_url_3.unwrap();
        let parsed_url_4 = parsed_url_4.unwrap();
        let parsed_url_5 = parsed_url_5.unwrap();
        let parsed_url_6 = parsed_url_6.unwrap();

        assert_eq!(parsed_url_1.shceme, "http");
        assert_eq!(parsed_url_1.host, "www.rust-lang.org");
        assert_eq!(parsed_url_1.port, 80);
        assert_eq!(parsed_url_1.path, "/");

        assert_eq!(parsed_url_2.shceme, "http");
        assert_eq!(parsed_url_2.host, "www.rust-lang.org");
        assert_eq!(parsed_url_2.port, 80);
        assert_eq!(parsed_url_2.path, "/");

        assert_eq!(parsed_url_3.shceme, "");
        assert_eq!(parsed_url_3.host, "www.rust-lang.org");
        assert_eq!(parsed_url_3.port, 80);
        assert_eq!(parsed_url_3.path, "/");

        assert_eq!(parsed_url_4.shceme, "http");
        assert_eq!(parsed_url_4.host, "www.rust-lang.org");
        assert_eq!(parsed_url_4.port, 80);
        assert_eq!(parsed_url_4.path, "/path/to/resource");

        assert_eq!(parsed_url_5.shceme, "");
        assert_eq!(parsed_url_5.host, "naver.com");
        assert_eq!(parsed_url_5.port, 80);
        assert_eq!(parsed_url_5.path, "/");

        assert_eq!(parsed_url_6.shceme, "http");
        assert_eq!(parsed_url_6.host, "www.rust-lang.org");
        assert_eq!(parsed_url_6.port, 80);
        assert_eq!(parsed_url_6.path, "/");

    }
}