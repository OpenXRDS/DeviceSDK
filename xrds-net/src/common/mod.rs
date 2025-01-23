pub mod enums;
pub mod data_structure;

use url::Url;

pub fn validate_url(raw_url: &str) -> Result<Url, String> {
    // do scheme test based on url string
    let tokenized_url = raw_url.split("://").collect::<Vec<&str>>();
    if tokenized_url.len() != 2 {
        return Err("Missing scheme".to_string());
    }
    
    let parsed_url = Url::parse(raw_url);
    if parsed_url.is_err() {
        return Err(parsed_url.err().unwrap().to_string());  // return the error message to force scheme in the url
    } else {
        return Ok(parsed_url.unwrap());
    }
}