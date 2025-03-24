pub mod enums;
pub mod data_structure;

use std::path;
use std::path::PathBuf;
use std::io::Read;

use quiche::h3::NameValue;

use random_string::generate;

use crate::common::data_structure::XrUrl;

const RANDOM_STRING_CHARSET: &str = "1234567890abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

pub fn parse_url(url: &str) -> Result<XrUrl, String> {
    
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

    // 2. separate query param if exists
    let mut host  = scheme_tokens[scheme_tokens.len() - 1];  // host:port / path (?query)
    let host_tokens = host.split("?").collect::<Vec<&str>>();
    if host_tokens.len() > 2 {
        return Err("Invalid query structure".to_string());
    }

    host = host_tokens[0];
    let query_params: Option<String> = if host_tokens.len() == 2 {
        Some(host_tokens[1].to_string())
    } else {
        None
    };
    

    // 3. separate host
    let mut host_tokens = host.split("/").collect::<Vec<&str>>();   // host / path
    
    host = host_tokens[0];
    let path = if host_tokens.len() > 1 {
        // path starts with "/"
        "/".to_string() + &host_tokens[1..].join("/")
    } else {
        "/".to_string()
    };
    
    // 4. separate port if exists
    let mut port = 80;
    host_tokens = host.split(":").collect::<Vec<&str>>();
    if host_tokens.len() == 2 {
        host = host_tokens[0];
        let port_parse = host_tokens[1].parse::<u32>();
        match port_parse {
            Ok(p) => port = p,
            Err(_) => return Err("Invalid port".to_string()),
        }
    } else if host_tokens.len() > 2 {
        return Err("Invalid host structure".to_string());
    }

    // 5. port range check
    if port < 1 || port > 65535 {
        return Err("Invalid port range".to_string());
    }

    Ok(XrUrl {   // temporal return value
        scheme: scheme.to_string(),
        host: host.to_string(),
        port: port,
        path: path.to_string(),
        raw_url: url.to_string(),
        
        query: query_params,
        username: None,
        password: None,
    })
}

pub fn coap_code_to_decimal(coap_code: &str) -> u32 {
    let coap_code_token = coap_code.split(".").collect::<Vec<&str>>();

    let class = coap_code_token[0].parse::<u32>().unwrap();
    let detail = coap_code_token[1].parse::<u32>().unwrap();

    class * 32 + detail
}

pub fn validate_path(path: &str) -> Result<(), String> {
    let path = path::Path::new(path);
    if path.exists() {
        Ok(())
    } else {
        Err("Invalid path".to_string())
    }
}

pub fn validate_path_write_permission(path: &str) -> Result<(), String> {
    let p_path = path::Path::new(path);

    if p_path.metadata().unwrap().permissions().readonly() {
        Err("No write permission".to_string())
    } else {
        Ok(())
    }
}

pub fn append_to_path(p: PathBuf, s: &str) -> PathBuf {
    let mut p = p.into_os_string();
    p.push(s);
    p.into()
}

fn convert_header_to_h3_header(headers: Vec<(String, String)>) -> Vec<quiche::h3::Header> {
    let mut h3_headers: Vec<quiche::h3::Header> = Vec::new();
    for (key, value) in headers {
        let key = key.as_bytes();
        let value = value.as_bytes();

        h3_headers.push(quiche::h3::Header::new(key, value));
    }

    h3_headers
}

/**
 * This function is to satisfy RFC 9114 Section 4.3.1
 * https://datatracker.ietf.org/doc/html/rfc9114#section-4.3.1
 * Mandatory headers are:
 * - :method
 * - :scheme
 * - :authority
 * - :path
 * 
 * Some sites may block requests without user-agent header.
 * So, user-agent header is also added.
 * 
 * Default Method: GET
 * If method is not provided by either set_method or header, GET method is used.
 */
pub fn fill_mandatory_http_headers(url: XrUrl, headers: Option<Vec<(String, String)>>, method: Option<String>) -> Vec<quiche::h3::Header> {
    let mut h3_headers = match headers {
        Some(h) => convert_header_to_h3_header(h),
        None => Vec::new(),
    };

    let mut mandatory_headers = Vec::new();

    let mut has_method = false;
    let mut has_scheme = false;
    let mut has_authority = false;
    let mut has_path = false;
    let mut has_useragent = false;  // optional. some sites may block requests without user-agent

    for header in h3_headers.iter() {
        if header.name() == b":method" {
            has_method = true;
        }
        if header.name() == b":scheme" {
            has_scheme = true;
        }
        if header.name() == b":authority" {
            has_authority = true;
        }
        if header.name() == b":path" {
            has_path = true;
        }
        if header.name() == b"user-agent" {
            has_useragent = true;
        }
    }

    if !has_method {
        let mut method_str = "GET";
        if method.is_some() {
            let binding = method.as_ref().unwrap();
            method_str = binding.as_str();
        }
        mandatory_headers.push(quiche::h3::Header::new(b":method", method_str.as_bytes()));
    }
    if !has_scheme {
        mandatory_headers.push(quiche::h3::Header::new(b":scheme", url.scheme.as_bytes()));
    }
    if !has_authority {
        let authority = format!("{}:{}", url.host, url.port);
        mandatory_headers.push(quiche::h3::Header::new(b":authority", authority.as_bytes()));
    }
    if !has_path {
        mandatory_headers.push(quiche::h3::Header::new(b":path", url.path.as_bytes()));
    }
    if !has_useragent {
        mandatory_headers.push(quiche::h3::Header::new(b"user-agent", b"xrds/1.0"));
    }

    h3_headers.extend(mandatory_headers);
    h3_headers

}


pub fn read_file_from_disk(path: &str) -> Result<Vec<u8>, String> {
    let p = path::Path::new(path);
    if !p.exists() {
        return Err("File does not exist".to_string());
    }

    let mut file = match std::fs::File::open(p) {
        Ok(f) => f,
        Err(_) => return Err("File open error".to_string()),
    };

    let mut buffer = Vec::new();
    match file.read_to_end(&mut buffer) {
        Ok(_) => Ok(buffer),
        Err(_) => Err("File read error".to_string()),
    }
}

pub fn payload_str_to_vector_str(payload: &str) -> Vec<String> {
    let payload_tokens = payload.split(",").collect::<Vec<&str>>();
    let mut payload_vector = Vec::new();
    for token in payload_tokens {
        let token = token.trim().replace('\"', "")
            .replace('[', "")
            .replace(']',"");
        if !token.is_empty() {
            payload_vector.push(token.to_string());
        }
    }
    payload_vector
}

pub fn generate_random_string(length: usize) -> String {
    let charset_str = RANDOM_STRING_CHARSET;

    let random_str = generate(length, charset_str);
    random_str    
}

pub fn generate_uuid() -> String {
    let uuid = uuid::Uuid::new_v4();
    uuid.to_string()
}

#[cfg(test)]
mod tests;