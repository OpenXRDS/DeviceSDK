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

//! Assessment sender — **HTTP** node.
//!
//! HTTP is request-shaped, so the sending node is the one that *serves*: it
//! publishes the whole content suite and the receiver pulls it with
//! `XrdsNet::request`. That is the real XR content path (an app fetching a GLB
//! from a content server), so it is the direction worth assessing.
//!
//! Resource layout, shared with the CoAP pair so the two receivers differ only
//! in the URL scheme:
//!
//! - `GET /count` → how many items the suite holds
//! - `GET /item/<i>/manifest` → that item's manifest
//! - `GET /item/<i>/payload` → that item's bytes
//!
//! xrds-net has no HTTP *server*, so this side is a deliberately minimal
//! HTTP/1.1 responder built on `std::net` — it exists only to give the xrds-net
//! client a peer. The assessed code is all on the receiver.
//!
//! Run this first:
//! ```text
//! cargo run --example assess_http_sender   -- --port 9302
//! cargo run --example assess_http_receiver -- --url http://127.0.0.1:9302
//! ```

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

#[path = "assess_common.rs"]
mod common;

use common::{
    choose_port, local_ip, new_run_id, print_peer_command, print_send_banner, resolve_items, Args,
    Manifest, PortKind,
};

const PROTOCOL: &str = "http";
const API: &str = "peer node for XrdsNet::request (minimal std::net HTTP/1.1 server)";
const USAGE: &str = "cargo run --example assess_http_sender -- --port 9302 [--files a,b --types t1,t2 | --file <path> --content-type <type>] [--keep]";

fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let port = choose_port(
        PortKind::Tcp,
        "0.0.0.0",
        args.get_or("port", "9302")
            .parse()
            .expect("--port must be a number"),
        args.flag("strict-port"),
    );
    let keep = args.flag("keep");

    let items = resolve_items(&args);
    let run_id = new_run_id();
    // The address actually bound, not a loopback guess: this string goes
    // into the evidence as where the content was served from.
    let url = format!("http://0.0.0.0:{port}/");
    // Whole-file transport: one "chunk" per item by definition.
    print_send_banner(PROTOCOL, API, &url, &run_id, &items, 0);

    let suite_size = items.len() as u32;
    let manifests: Vec<Vec<u8>> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            Manifest::for_item(
                &run_id,
                PROTOCOL,
                &url,
                item,
                item.data.len() as u32,
                index as u32,
                suite_size,
            )
            .to_json()
        })
        .collect();

    let listener = TcpListener::bind(("0.0.0.0", port)).unwrap_or_else(|e| {
        eprintln!("cannot bind port {port}: {e}");
        std::process::exit(1);
    });

    println!("  serving  GET /count             -> {suite_size}");
    println!("  serving  GET /item/<i>/manifest -> item manifest");
    println!("  serving  GET /item/<i>/payload  -> item bytes");
    print_peer_command(
        "receiving",
        &format!(
            "assess_http_receiver.exe --url http://{}:{port}",
            local_ip()
        ),
    );
    println!("  waiting for the receiver ...");

    let mut served_last = false;

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("  ! accept failed: {e}");
                continue;
            }
        };

        let Some(path) = read_request_path(&mut stream) else {
            continue;
        };
        println!("  <- GET {path}");

        let target = path.trim_start_matches('/');
        if target == "count" {
            respond(&mut stream, 200, "text/plain", suite_size.to_string().as_bytes());
        } else if let Some((index, what)) = parse_item_path(target) {
            match (items.get(index), what) {
                (Some(_), "manifest") => {
                    respond(&mut stream, 200, "application/json", &manifests[index])
                }
                (Some(item), "payload") => {
                    respond(&mut stream, 200, "application/octet-stream", &item.data);
                    served_last = index + 1 == items.len();
                }
                _ => respond(&mut stream, 404, "text/plain", b"no such item\n"),
            }
        } else {
            respond(&mut stream, 404, "text/plain", b"not found\n");
        }

        if served_last && !keep {
            println!();
            println!("  whole suite served; exiting (pass --keep to stay up for more runs)");
            return;
        }
    }
}

/// `item/<i>/<what>` → `(i, what)`.
fn parse_item_path(target: &str) -> Option<(usize, &str)> {
    let rest = target.strip_prefix("item/")?;
    let (index, what) = rest.split_once('/')?;
    Some((index.parse().ok()?, what))
}

/// Reads one HTTP/1.1 request and returns its path. Requests here are bodyless
/// GETs, so the head is all there is to parse.
fn read_request_path(stream: &mut TcpStream) -> Option<String> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;

    // Drain the remaining headers so the client isn't left mid-write.
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 || line.trim().is_empty() {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let _method = parts.next()?;
    Some(parts.next()?.to_string())
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let reason = if status == 200 { "OK" } else { "Not Found" };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}
