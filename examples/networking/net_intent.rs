// The protocol-agnostic path: XrdsNet's four intent verbs (request,
// dispatch, listen, transfer). No protocol enum, no builder chain, no
// connect()/send() choice to make — the URL scheme picks the mechanism.
// See docs/xrds-net-protocol-handler.md for the full design.
//
// Compare with net.rs, which shows the same operations through the expert
// ClientBuilder/Client session API this is built on top of.
use xrds_net::{RequestOptions, TransferOp, TransferResult, XrdsNet};

fn example_request() {
    println!("=== XrdsNet::request (HTTP) ===");

    match XrdsNet::request("http://www.rust-lang.org:80/", RequestOptions::get()) {
        Ok(response) => println!(
            "status: {}, body bytes: {}",
            response.status_code,
            response.body.len()
        ),
        Err(e) => println!("request failed: {e}"),
    }
}

fn example_dispatch_and_listen() {
    println!("=== XrdsNet::dispatch + listen (MQTT) ===");

    let topic_url = "mqtt://test.mosquitto.org:1883/xrds-net/examples/net_intent";

    // listen() connects AND subscribes (confirmed via SUBACK) before
    // returning, so dispatching right after is safe — no manual ordering or
    // race to manage.
    let stream = match XrdsNet::listen(topic_url) {
        Ok(stream) => stream,
        Err(e) => {
            println!("listen failed: {e}");
            return;
        }
    };

    if let Err(e) = XrdsNet::dispatch(topic_url, b"hello from XrdsNet".to_vec()) {
        println!("dispatch failed: {e}");
        return;
    }

    match stream.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(event) => println!("received: {}", String::from_utf8_lossy(&event.payload)),
        Err(e) => println!("listen timed out: {e}"),
    }
    stream.close();
}

fn example_transfer() {
    println!("=== XrdsNet::transfer (FTP) ===");

    // Credentials travel as URL userinfo (user:pass@host) — there's no
    // FTP-specific builder step at this layer, by design.
    let result = XrdsNet::transfer(
        "ftp://demo:password@test.rebex.net:21/readme.txt",
        TransferOp::Download,
    );

    match result {
        Ok(TransferResult::Downloaded(bytes)) => println!("downloaded {} bytes", bytes.len()),
        Ok(other) => println!("unexpected result: {other:?}"),
        Err(e) => println!("transfer failed: {e}"),
    }
}

pub fn main() {
    println!("xrds-net: protocol-agnostic XrdsNet intent-verb example\n");

    example_request();
    example_dispatch_and_listen();
    example_transfer();
}
