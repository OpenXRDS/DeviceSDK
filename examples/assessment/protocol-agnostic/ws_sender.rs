/*
Copyright 2025 KETI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! Assessment sender — **WebSocket** node.
//!
//! Sends the whole content suite (one file per supported XR content type) over
//! a single session, so one run of this pair covers the `ws` column of the
//! content × protocol matrix.
//!
//! Uses the protocol-agnostic session verb: `XrdsNet::open(url)` returns a
//! [`NetChannel`] this binary both sends chunks on and reads acks from. Nothing
//! here mentions WebSocket except the URL scheme, and nothing branches on
//! content type.
//!
//! Run the receiver first:
//! ```text
//! cargo run --example assess_ws_receiver -- --port 9301
//! cargo run --example assess_ws_sender   -- --url ws://127.0.0.1:9301/
//! ```

use std::time::{Duration, Instant};

use xrds_net::XrdsNet;

#[path = "assess_common.rs"]
mod common;

use common::{
    decode_frame, encode_frame, new_run_id, print_send_banner, resolve_items, Args, Manifest,
    KIND_ACK, KIND_CHUNK, KIND_END, KIND_MANIFEST,
};

const PROTOCOL: &str = "ws";
const API: &str = "XrdsNet::open -> NetChannel::send / try_recv";
const USAGE: &str = "cargo run --example assess_ws_sender -- --url ws://127.0.0.1:9301/ [--files a,b --types t1,t2 | --file <path> --content-type <type>] [--chunk-size 65536]";

fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let url = args.get_or("url", "ws://127.0.0.1:9301/").to_string();
    let chunk_size: usize = args
        .get_or("chunk-size", "65536")
        .parse::<usize>()
        .expect("--chunk-size must be a number")
        .max(1);
    let ack_timeout = Duration::from_secs(
        args.get_or("ack-timeout", "15")
            .parse()
            .expect("--ack-timeout must be a number of seconds"),
    );

    let items = resolve_items(&args);
    let run_id = new_run_id();
    print_send_banner(PROTOCOL, API, &url, &run_id, &items, chunk_size);

    // One protocol-agnostic call opens the session; the scheme in the URL is
    // the only thing that decided it would be WebSocket.
    let mut channel = match XrdsNet::open(&url) {
        Ok(channel) => channel,
        Err(e) => {
            eprintln!("open({url}) failed: {e}");
            eprintln!("is the receiver running?  {USAGE}");
            std::process::exit(1);
        }
    };

    let suite_size = items.len() as u32;
    let started = Instant::now();

    for (index, item) in items.iter().enumerate() {
        let manifest = Manifest::for_item(
            &run_id,
            PROTOCOL,
            &url,
            item,
            chunk_size as u32,
            index as u32,
            suite_size,
        );
        let at = Instant::now();

        // Manifest first, so the receiver knows what is coming and can verify it.
        channel
            .send(encode_frame(KIND_MANIFEST, 0, &manifest.to_json()))
            .unwrap_or_else(|e| fail(&format!("send manifest: {e}")));
        expect_ack(&mut channel, 0, ack_timeout, "manifest");

        // Then the payload, one acknowledged chunk at a time.
        for (seq, chunk) in item.data.chunks(chunk_size).enumerate() {
            let seq = seq as u32;
            channel
                .send(encode_frame(KIND_CHUNK, seq, chunk))
                .unwrap_or_else(|e| fail(&format!("send chunk {seq}: {e}")));
            expect_ack(&mut channel, seq, ack_timeout, "chunk");
        }

        channel
            .send(encode_frame(KIND_END, manifest.chunk_count, b""))
            .unwrap_or_else(|e| fail(&format!("send end: {e}")));
        let verdict = expect_ack(&mut channel, manifest.chunk_count, ack_timeout, "end");

        println!(
            "  [{}/{}] {:<15} {:<30} {:>10} B  {:>9}  {verdict}",
            index + 1,
            suite_size,
            item.content_type,
            item.file_name,
            item.data.len(),
            format!("{:.3} s", at.elapsed().as_secs_f64()),
        );
        if verdict != "PASS" {
            fail(&format!("receiver rejected {}", item.file_name));
        }
    }

    let total: usize = items.iter().map(|i| i.data.len()).sum();
    println!();
    println!(
        "  sent {} item(s), {total} bytes in {:.3} s",
        items.len(),
        started.elapsed().as_secs_f64()
    );
    println!("  the receiver prints the suite report");
    let _ = channel.close();
}

/// Waits for the receiver's ack of `seq` and returns its text. A NACK (or a
/// timeout) ends the run — a silent partial transfer would be worse than a
/// failed one.
fn expect_ack(
    channel: &mut xrds_net::NetChannel,
    seq: u32,
    timeout: Duration,
    what: &str,
) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(event) = channel.try_recv() {
            match decode_frame(&event.payload) {
                Ok(frame) if frame.kind == KIND_ACK && frame.seq == seq => {
                    return String::from_utf8_lossy(&frame.payload).to_string();
                }
                Ok(frame) if frame.kind == KIND_ACK => {
                    // Out-of-order ack: keep waiting for ours.
                    println!("  (ack for {} while waiting for {seq})", frame.seq);
                }
                Ok(frame) => fail(&format!(
                    "receiver rejected the {what}: {}",
                    String::from_utf8_lossy(&frame.payload)
                )),
                Err(e) => fail(&format!("undecodable reply while waiting for {what}: {e}")),
            }
        }
        if let Some(e) = channel.take_error() {
            fail(&format!("session error while waiting for {what}: {e}"));
        }
        if Instant::now() >= deadline {
            fail(&format!("timed out waiting for the {what} ack (seq {seq})"));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn fail(message: &str) -> ! {
    eprintln!("FAIL: {message}");
    std::process::exit(1);
}
