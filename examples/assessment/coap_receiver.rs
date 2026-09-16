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

//! Assessment receiver — **CoAP** node.
//!
//! Pure protocol-agnostic API: `XrdsNet::request(url, RequestOptions::get())`,
//! for the suite size, then each item's manifest and chunks. These are the
//! *same calls* the HTTP receiver makes against the same resource layout — only
//! the URL scheme differs, which is the protocol-agnostic claim stated as
//! plainly as it can be.
//!
//! CoAP runs over UDP, so a lost request is a normal event rather than a
//! failure: each request is retried a few times before the run is given up, and
//! the retry count is reported.
//!
//! Chunks arrive base64-encoded because xrds-net's CoAP client rejects
//! non-UTF-8 response payloads — see the findings in README.md.
//!
//! Run the sender (the content node) first:
//! ```text
//! cargo run --example assess_coap_sender   -- --port 5683
//! cargo run --example assess_coap_receiver -- --url coap://127.0.0.1:5683
//! ```

use std::path::PathBuf;
use std::time::Instant;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use xrds_net::{RequestOptions, XrdsNet};

#[path = "assess_common.rs"]
mod common;

use common::{default_out_dir, Args, Manifest, SuiteReport};

const PROTOCOL: &str = "coap";
const API: &str = "XrdsNet::request (GET)";
/// CoAP 2.05 Content — `class * 32 + detail`, the encoding xrds-net reports in
/// `NetResponse::status_code` for CoAP (see `common::coap_code_to_decimal`).
const COAP_CONTENT: u32 = 69;
const USAGE: &str = "cargo run --example assess_coap_receiver -- --url coap://127.0.0.1:5683 [--retries 3]";

fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let base = args
        .get_or("url", "coap://127.0.0.1:5683")
        .trim_end_matches('/')
        .to_string();
    let out_dir = args
        .get("out-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_out_dir);
    let retries: u32 = args
        .get_or("retries", "3")
        .parse()
        .expect("--retries must be a number");

    println!("XRDS assessment receiver (CoAP)");
    println!("  pulling from {base}");

    let mut retried = 0u32;

    let count_body = fetch(&format!("{base}/count"), "suite size", retries, &mut retried);
    let count: u32 = String::from_utf8_lossy(&count_body)
        .trim()
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("FAIL: /count is not a number: {e}");
            std::process::exit(1);
        });
    println!("  suite holds {count} item(s)");

    let mut report = SuiteReport::new(PROTOCOL, API, base.clone(), out_dir);

    for index in 0..count {
        let at = Instant::now();
        let manifest_body = fetch(
            &format!("{base}/item/{index}/manifest"),
            &format!("item {index} manifest"),
            retries,
            &mut retried,
        );
        let manifest = match Manifest::from_json(&manifest_body) {
            Ok(manifest) => manifest,
            Err(e) => {
                eprintln!("FAIL: {e}");
                let _ = report.finish();
                std::process::exit(1);
            }
        };

        let mut data = Vec::with_capacity(manifest.size as usize);
        for chunk in 0..manifest.chunk_count {
            let encoded = fetch(
                &format!("{base}/item/{index}/chunk/{chunk}"),
                &format!("item {index} chunk {chunk}"),
                retries,
                &mut retried,
            );
            match BASE64.decode(encoded.as_slice()) {
                Ok(bytes) => data.extend_from_slice(&bytes),
                Err(e) => {
                    eprintln!("FAIL: item {index} chunk {chunk} is not valid base64: {e}");
                    let _ = report.finish();
                    std::process::exit(1);
                }
            }
        }

        report.record(manifest, &data, Some(at.elapsed()));
    }

    if retried > 0 {
        println!("  (UDP: {retried} request(s) had to be retried)");
    }

    std::process::exit(report.finish());
}

/// One `XrdsNet::request`, retried on the transport errors a UDP protocol is
/// entitled to produce. A CoAP response code other than 2.05 Content is a real
/// answer, so it ends the run rather than being retried.
fn fetch(url: &str, what: &str, retries: u32, retried: &mut u32) -> Vec<u8> {
    let mut attempt = 0;
    loop {
        match XrdsNet::request(url, RequestOptions::get()) {
            Ok(response) if response.status_code == COAP_CONTENT && response.error.is_none() => {
                return response.body;
            }
            Ok(response) => {
                eprintln!(
                    "FAIL: {what} returned CoAP status {} ({:?})",
                    response.status_code, response.error
                );
                std::process::exit(1);
            }
            Err(e) if attempt < retries => {
                attempt += 1;
                *retried += 1;
                println!("  (retrying {what} after: {e})");
            }
            Err(e) => {
                eprintln!("FAIL: {what} request to {url} failed after {attempt} retries: {e}");
                eprintln!("is the sender node running?  {USAGE}");
                std::process::exit(1);
            }
        }
    }
}
