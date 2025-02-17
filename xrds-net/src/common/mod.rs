pub mod enums;
pub mod data_structure;

use std::path;
use std::path::PathBuf;

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

    Ok(Url {   // temporal return value
        scheme: scheme.to_string(),
        host: host.to_string(),
        port: port,
        path: path.to_string(),
        
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

#[cfg(test)]
mod tests;