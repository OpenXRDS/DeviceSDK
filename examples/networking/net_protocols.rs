// Focused example: a tour of the capability matrix (MANUAL.md §12) — each
// protocol reached through the same four intent verbs, with the URL scheme
// selecting the protocol. Shows which verb each protocol actually supports.
use std::time::Duration;

use xrds_net::{RequestOptions, TransferOp, TransferResult, XrdsNet};

fn main() {
    println!("xrds-net: capability tour (one supported verb per protocol)\n");

    // --- request: HTTP(S) / FILE / CoAP (and HTTP3 via the expert API) ------
    match XrdsNet::request("http://www.rust-lang.org/", RequestOptions::get()) {
        Ok(r) => println!("[HTTP] request  -> status {}, {} bytes", r.status_code, r.body.len()),
        Err(e) => println!("[HTTP] request  failed: {e}"),
    }
    match XrdsNet::request("coap://coap.me:5683/hello", RequestOptions::get()) {
        Ok(r) => println!("[CoAP] request  -> status {}, {} bytes", r.status_code, r.body.len()),
        Err(e) => println!("[CoAP] request  failed: {e}"),
    }

    // --- dispatch: WS / QUIC / MQTT (send, fire-and-forget) -----------------
    // WS is a single bidirectional connection: dispatch connects + sends. To
    // read a reply on the SAME socket (echo / request-response), use the
    // expert Client API (examples/net.rs), not a separate listen().
    match XrdsNet::dispatch("wss://echo.websocket.org/", b"ping".to_vec()) {
        Ok(()) => println!("[WS]   dispatch -> sent"),
        Err(e) => println!("[WS]   dispatch failed: {e}"),
    }

    // --- dispatch + listen round-trip: MQTT (broker routes pub/sub) ---------
    // Unlike WS, an MQTT broker routes across connections, so a separate
    // listen() receives what a separate dispatch() publishes.
    let topic = "mqtt://test.mosquitto.org:1883/xrds-net/examples/tour";
    match XrdsNet::listen(topic) {
        Ok(stream) => {
            let _ = XrdsNet::dispatch(topic, b"telemetry".to_vec());
            match stream.recv_timeout(Duration::from_secs(5)) {
                Ok(ev) => println!(
                    "[MQTT] listen   -> got {:?}",
                    String::from_utf8_lossy(&ev.payload)
                ),
                Err(e) => println!("[MQTT] listen   -> {e}"),
            }
            stream.close();
        }
        Err(e) => println!("[MQTT] listen   failed: {e}"),
    }

    // --- transfer: FTP / SFTP (credentials as URL userinfo; ftp -> :21) -----
    match XrdsNet::transfer("ftp://demo:password@test.rebex.net/", TransferOp::List) {
        Ok(TransferResult::Listed(entries)) => println!("[FTP]  transfer -> {} entries", entries.len()),
        Ok(other) => println!("[FTP]  transfer -> {other:?}"),
        Err(e) => println!("[FTP]  transfer failed: {e}"),
    }
}
