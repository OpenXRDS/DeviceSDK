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

//! Assessment receiver — **HTTP** node.
//!
//! Pure protocol-agnostic API: `XrdsNet::request(url, RequestOptions::get())`,
//! for the suite size, then each item's manifest and payload. This is the verb
//! an XR app uses to pull a GLB or a texture from a content server, so it is
//! the HTTP shape worth assessing.
//!
//! The CoAP receiver makes the *same calls* against the same resource layout —
//! only the URL scheme differs.
//!
//! Run the sender (the content node) first:
//! ```text
//! cargo run --example assess_http_sender   -- --port 9302
//! cargo run --example assess_http_receiver -- --url http://127.0.0.1:9302
//! ```

use std::path::PathBuf;
use std::time::Instant;

use xrds_net::{RequestOptions, XrdsNet};

#[path = "assess_common.rs"]
mod common;

use common::{default_out_dir, Args, Manifest, SuiteReport};

const PROTOCOL: &str = "http";
const API: &str = "XrdsNet::request (GET)";
const USAGE: &str = "cargo run --example assess_http_receiver -- --url http://127.0.0.1:9302 [--out-dir examples/assessment/received]";

fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let base = args
        .get_or("url", "http://127.0.0.1:9302")
        .trim_end_matches('/')
        .to_string();
    let out_dir = args
        .get("out-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_out_dir);

    println!("XRDS assessment receiver (HTTP)");
    println!("  pulling from {base}");

    let count_body = fetch(&format!("{base}/count"), "suite size");
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
        );
        let manifest = match Manifest::from_json(&manifest_body) {
            Ok(manifest) => manifest,
            Err(e) => {
                eprintln!("FAIL: {e}");
                let _ = report.finish();
                std::process::exit(1);
            }
        };

        let data = fetch(
            &format!("{base}/item/{index}/payload"),
            &format!("item {index} payload"),
        );
        report.record(manifest, &data, Some(at.elapsed()));
    }

    std::process::exit(report.finish());
}

fn fetch(url: &str, what: &str) -> Vec<u8> {
    match XrdsNet::request(url, RequestOptions::get()) {
        Ok(response) if response.status_code == 200 => response.body,
        Ok(response) => {
            eprintln!(
                "FAIL: {what} request returned HTTP {} ({:?})",
                response.status_code, response.error
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("FAIL: {what} request to {url} failed: {e}");
            eprintln!("is the sender node running?  {USAGE}");
            std::process::exit(1);
        }
    }
}
