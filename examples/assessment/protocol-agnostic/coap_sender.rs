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

//! Assessment sender — **CoAP** node.
//!
//! CoAP is request-shaped in xrds-net (`CoapHandler` is GET-only), so as with
//! HTTP the sending node is the one that *serves*, using the same resource
//! layout so the two receivers differ only in the URL scheme:
//!
//! - `GET /count` → how many items the suite holds
//! - `GET /item/<i>/manifest` → that item's manifest
//! - `GET /item/<i>/chunk/<n>` → that item's bytes, base64, `--chunk-size` each
//!
//! Two CoAP-specific constraints shape this:
//!
//! - **Chunked payloads.** CoAP runs over UDP, so a response larger than a
//!   datagram needs block-wise transfer (RFC 7959). That *does* work through
//!   this path — the `coap` crate implements it on both ends, and a 64 KiB
//!   chunk (87 KiB of base64) round-trips fine on loopback and over a LAN — so
//!   the chunking here is about bounding memory and retry cost, not about
//!   fitting a datagram. Lower `--chunk-size` on a lossy link, where a smaller
//!   block costs less to lose.
//! - **Text-only bodies.** xrds-net's CoAP client runs the response payload
//!   through `String::from_utf8` and reports an error for anything else
//!   (`protocols/coap.rs`), so raw asset bytes cannot survive the trip. Each
//!   chunk is therefore base64-encoded here and decoded by the receiver — 33%
//!   overhead, a library limitation rather than a property of CoAP. See the
//!   findings in README.md.
//!
//! xrds-net has no CoAP *server*, so this side uses the `coap` crate directly;
//! it exists only to give the xrds-net client a peer. The assessed code is all
//! on the receiver.
//!
//! Run this first:
//! ```text
//! cargo run --example assess_coap_sender   -- --port 5683
//! cargo run --example assess_coap_receiver -- --url coap://127.0.0.1:5683
//! ```

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use coap::Server;
use coap_lite::{CoapRequest, RequestType, ResponseType};

#[path = "assess_common.rs"]
mod common;

use common::{
    choose_port, local_ip, new_run_id, print_peer_command, print_send_banner, resolve_items, Args,
    Manifest, PortKind,
};

const PROTOCOL: &str = "coap";
const API: &str = "peer node for XrdsNet::request (coap crate server)";
/// Block-wise transfer carries whatever this is set to, so it is a
/// memory/retry trade-off rather than a datagram limit: 16 KiB raw (≈22 KiB of
/// base64) keeps the request count low without making a single lost block
/// expensive. Verified up to 64 KiB on loopback and over a LAN; drop to 512 on
/// a lossy link.
const DEFAULT_CHUNK: &str = "16384";
const USAGE: &str = "cargo run --example assess_coap_sender -- --port 5683 [--files a,b --types t1,t2 | --file <path> --content-type <type>] [--chunk-size 16384] [--keep]";

/// One suite item, prepared for serving.
struct Served {
    manifest_json: Vec<u8>,
    /// Base64 of each raw chunk, ready to hand out.
    chunks: Vec<String>,
}

struct Content {
    items: Vec<Served>,
    served_last: AtomicBool,
    served_count: AtomicUsize,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let port = choose_port(
        PortKind::Udp,
        "0.0.0.0",
        args.get_or("port", "5683")
            .parse()
            .expect("--port must be a number"),
        args.flag("strict-port"),
    );
    let chunk_size: usize = args
        .get_or("chunk-size", DEFAULT_CHUNK)
        .parse::<usize>()
        .expect("--chunk-size must be a number")
        .max(1);
    let keep = args.flag("keep");

    let items = resolve_items(&args);
    let run_id = new_run_id();
    // The address actually bound, not a loopback guess: this string goes
    // into the evidence as where the content was served from.
    let url = format!("coap://0.0.0.0:{port}/");
    print_send_banner(PROTOCOL, API, &url, &run_id, &items, chunk_size);

    let suite_size = items.len() as u32;
    let served: Vec<Served> = items
        .iter()
        .enumerate()
        .map(|(index, item)| Served {
            manifest_json: Manifest::for_item(
                &run_id,
                PROTOCOL,
                &url,
                item,
                chunk_size as u32,
                index as u32,
                suite_size,
            )
            .to_json(),
            chunks: item.data.chunks(chunk_size).map(|c| BASE64.encode(c)).collect(),
        })
        .collect();

    let total_chunks: usize = served.iter().map(|s| s.chunks.len()).sum();
    println!("  serving  GET /count             -> {suite_size}");
    println!("  serving  GET /item/<i>/manifest -> item manifest");
    println!("  serving  GET /item/<i>/chunk/<n> -> {total_chunks} base64 chunk(s) in total");
    print_peer_command(
        "receiving",
        &format!(
            "assess_coap_receiver.exe --url coap://{}:{port}",
            local_ip()
        ),
    );
    println!("  waiting for the receiver ...");

    let content = Arc::new(Content {
        items: served,
        served_last: AtomicBool::new(false),
        served_count: AtomicUsize::new(0),
    });

    // The server owns the socket for its whole life, so the "we're done"
    // signal has to come from the handler — watched here and used to end the
    // process once the suite's last chunk has gone out.
    let watched = Arc::clone(&content);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if watched.served_last.load(Ordering::Relaxed) && !keep {
                println!();
                println!(
                    "  whole suite served in {} request(s); exiting (pass --keep to stay up)",
                    watched.served_count.load(Ordering::Relaxed)
                );
                std::process::exit(0);
            }
        }
    });

    let server = Server::new_udp(("0.0.0.0", port)).unwrap_or_else(|e| {
        eprintln!("cannot bind CoAP port {port}: {e}");
        std::process::exit(1);
    });

    let result = server
        .run(move |mut request: Box<CoapRequest<SocketAddr>>| {
            let content = Arc::clone(&content);
            async move {
                serve(&mut request, &content);
                request
            }
        })
        .await;

    if let Err(e) = result {
        eprintln!("CoAP server stopped: {e}");
        std::process::exit(1);
    }
}

fn serve(request: &mut CoapRequest<SocketAddr>, content: &Content) {
    let path = request.get_path();
    let is_get = *request.get_method() == RequestType::Get;
    content.served_count.fetch_add(1, Ordering::Relaxed);

    let Some(response) = request.response.as_mut() else {
        return;
    };

    if !is_get {
        response.set_status(ResponseType::MethodNotAllowed);
        response.message.payload = b"only GET is served".to_vec();
        return;
    }

    if path == "count" {
        response.message.payload = content.items.len().to_string().into_bytes();
        return;
    }

    match parse_item_path(&path) {
        Some((index, Resource::Manifest)) => match content.items.get(index) {
            Some(item) => {
                println!("  <- GET /item/{index}/manifest");
                response.message.payload = item.manifest_json.clone();
            }
            None => not_found(response),
        },
        Some((index, Resource::Chunk(chunk))) => {
            match content.items.get(index).and_then(|i| i.chunks.get(chunk)) {
                Some(encoded) => {
                    response.message.payload = encoded.as_bytes().to_vec();
                    let last_item = index + 1 == content.items.len();
                    let last_chunk = chunk + 1 == content.items[index].chunks.len();
                    if last_item && last_chunk {
                        content.served_last.store(true, Ordering::Relaxed);
                    }
                }
                None => not_found(response),
            }
        }
        None => not_found(response),
    }
}

enum Resource {
    Manifest,
    Chunk(usize),
}

/// `item/<i>/manifest` or `item/<i>/chunk/<n>`.
fn parse_item_path(path: &str) -> Option<(usize, Resource)> {
    let rest = path.strip_prefix("item/")?;
    let (index, what) = rest.split_once('/')?;
    let index = index.parse().ok()?;
    if what == "manifest" {
        return Some((index, Resource::Manifest));
    }
    let chunk = what.strip_prefix("chunk/")?.parse().ok()?;
    Some((index, Resource::Chunk(chunk)))
}

fn not_found(response: &mut coap_lite::CoapResponse) {
    response.set_status(ResponseType::NotFound);
    response.message.payload = b"not found".to_vec();
}
