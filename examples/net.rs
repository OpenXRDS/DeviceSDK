// External usage: Pure Rust API without XRDS SDK or Bevy
use xrds_net::{
    client::ClientBuilder,
    common::{data_structure::FtpPayload, enums::PROTOCOLS},
    FtpCommands,
};

/// Example of external usage: Pure Rust API
/// This shows how to use xrds-net as a standalone library
fn example_http() {
    println!("=== External Usage (Pure Rust) ===");

    let client = ClientBuilder::new().set_protocol(PROTOCOLS::HTTP).build();

    let result = client.set_url("https://httpbin.org").request();
    println!("HTTP request result: {}", result.status_code);
}

fn example_ftp() {
    let client = ClientBuilder::new()
        .set_protocol(PROTOCOLS::FTP)
        .set_user("demo")
        .set_password("password")
        .build();
    let response = client.set_url("test.rebex.net:21").connect();

    println!("FTP connection response: {}", response.is_ok());

    let ftp_payload = FtpPayload {
        command: FtpCommands::QUIT,
        payload_name: "".to_string(),
        payload: None,
    };

    let ftp_response = response.unwrap().run_ftp_command(ftp_payload);
    println!(
        "FTP QUIT command response: {}",
        ftp_response.error.is_none()
    );
}

fn example_ws() {
    let client = ClientBuilder::new().set_protocol(PROTOCOLS::WS).build();
    let response = client.set_url("wss://echo.websocket.org").connect();

    println!("WebSocket connection response: {}", response.is_ok());
}

fn example_file_download() {
    let client = ClientBuilder::new().set_protocol(PROTOCOLS::FILE).build();
    let response = client
        .set_url("https://files.keti.xrds.kr/s/enS7MQox7zk2FA4")
        .request();

    println!("File download response: {:?}", response.status_code);
}

pub fn main() {
    println!("xrds-net: Dual-mode networking library example\n");

    example_http();
    example_ftp();
    example_ws();
    example_file_download();
}
