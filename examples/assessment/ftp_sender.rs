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

//! Assessment sender — **FTP** node.
//!
//! Uploads the whole content suite (one file per supported XR content type), so
//! a single run covers the `ftp` column of the content × protocol matrix.
//!
//! Pure protocol-agnostic API: `XrdsNet::transfer(url, TransferOp::Upload(..))`.
//! Credentials travel as URL userinfo (`user:pass@host`) — there is no
//! FTP-specific builder step at this layer, which is the point.
//!
//! For each item the payload goes up first and its manifest sidecar last, so
//! the receiver can treat the sidecar as "the payload beside me is complete".
//! After each upload this binary reads the file back with
//! `TransferOp::Download` and compares hashes, so the sender proves the round
//! trip independently of the receiver's report.
//!
//! Run the receiver first:
//! ```text
//! cargo run --example assess_ftp_receiver -- --port 2121
//! cargo run --example assess_ftp_sender   -- --url ftp://xrds:assessment@127.0.0.1:2121/
//! ```

use std::time::Instant;

use xrds_net::{TransferOp, TransferResult, XrdsNet};

#[path = "assess_common.rs"]
mod common;

use common::{
    new_run_id, print_send_banner, resolve_items, sha256_hex, Args, Manifest, MANIFEST_SUFFIX,
};

const PROTOCOL: &str = "ftp";
const API: &str = "XrdsNet::transfer (Upload / Download / List)";
const DEFAULT_URL: &str = "ftp://xrds:assessment@127.0.0.1:2121/";
const USAGE: &str = "cargo run --example assess_ftp_sender -- --url ftp://user:pass@host:2121/ [--files a,b --types t1,t2 | --file <path> --content-type <type>]";

fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let base = args.get_or("url", DEFAULT_URL).trim_end_matches('/').to_string();
    let items = resolve_items(&args);
    let run_id = new_run_id();
    // Whole-file transport: one "chunk" per item by definition, so no chunk
    // size to report.
    print_send_banner(PROTOCOL, API, &base, &run_id, &items, 0);

    let suite_size = items.len() as u32;
    let started = Instant::now();

    for (index, item) in items.iter().enumerate() {
        let payload_url = format!("{base}/{}", item.file_name);
        let manifest_url = format!("{base}/{}{MANIFEST_SUFFIX}", item.file_name);
        let manifest = Manifest::for_item(
            &run_id,
            PROTOCOL,
            &payload_url,
            item,
            item.data.len() as u32,
            index as u32,
            suite_size,
        );
        let at = Instant::now();

        match XrdsNet::transfer(&payload_url, TransferOp::Upload(item.data.clone())) {
            Ok(TransferResult::Uploaded) => {}
            Ok(other) => fail(&format!("unexpected upload result: {other:?}")),
            Err(e) => fail(&format!("upload of {} failed: {e}", item.file_name)),
        }

        // Read it back through the same agnostic verb: the sender's own proof
        // that what landed on the far node is what it sent.
        match XrdsNet::transfer(&payload_url, TransferOp::Download) {
            Ok(TransferResult::Downloaded(echoed)) => {
                if sha256_hex(&echoed) != manifest.sha256 {
                    fail(&format!(
                        "{} read back from the server differs from what was sent",
                        item.file_name
                    ));
                }
            }
            Ok(other) => fail(&format!("unexpected download result: {other:?}")),
            Err(e) => fail(&format!("read-back of {} failed: {e}", item.file_name)),
        }

        // Sidecar last — its arrival is the receiver's completion signal for
        // this item.
        match XrdsNet::transfer(&manifest_url, TransferOp::Upload(manifest.to_json())) {
            Ok(TransferResult::Uploaded) => {}
            Ok(other) => fail(&format!("unexpected manifest upload result: {other:?}")),
            Err(e) => fail(&format!("manifest upload for {} failed: {e}", item.file_name)),
        }

        println!(
            "  [{}/{}] {:<15} {:<30} {:>10} B  {:>9}  uploaded + read back",
            index + 1,
            suite_size,
            item.content_type,
            item.file_name,
            item.data.len(),
            format!("{:.3} s", at.elapsed().as_secs_f64())
        );
    }

    if let Ok(TransferResult::Listed(entries)) =
        XrdsNet::transfer(&format!("{base}/"), TransferOp::List)
    {
        println!("  server directory now holds {} entr(y/ies)", entries.len());
    }

    let total: usize = items.iter().map(|i| i.data.len()).sum();
    println!();
    println!(
        "  transferred {} item(s), {total} bytes in {:.3} s",
        items.len(),
        started.elapsed().as_secs_f64()
    );
    println!("  the receiver prints the suite report");
}

fn fail(message: &str) -> ! {
    eprintln!("FAIL: {message}");
    std::process::exit(1);
}
