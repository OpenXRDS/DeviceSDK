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

//! Assessment receiver — **FTP** node.
//!
//! Hosts xrds-net's own `XRNetServer` (FTP, via libunftp) rooted at the output
//! directory, then waits for the sender's uploads to land. FTP has no framing
//! of its own to carry a manifest in, so each item arrives as the payload
//! followed by a `<name>.xrds-manifest.json` sidecar — the sidecar appearing is
//! what tells this binary the payload beside it is complete.
//!
//! The first sidecar's `suite_size` says how many items the run holds, so this
//! binary prints the suite report and exits on its own.
//!
//! Run:
//! ```text
//! cargo run --example assess_ftp_receiver -- --port 2121
//! cargo run --example assess_ftp_sender   -- --url ftp://xrds:assessment@127.0.0.1:2121/
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use xrds_net::common::enums::PROTOCOLS;
use xrds_net::server::XRNetServer;

#[path = "assess_common.rs"]
mod common;

use common::{
    choose_port, default_out_dir, local_ip, print_peer_command, Args, Manifest, PortKind,
    SuiteReport, MANIFEST_SUFFIX,
};

const PROTOCOL: &str = "ftp";
const API: &str = "XRNetServer (FTP/libunftp) <- XrdsNet::transfer(Upload)";
const USAGE: &str = "cargo run --example assess_ftp_receiver -- --port 2121 [--bind 0.0.0.0] [--strict-port] [--out-dir examples/assessment/protocol-agnostic/received] [--timeout 300]";

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    // All interfaces by default, like every other receiver in this harness.
    // `XRNetServer` itself still defaults to loopback (its FTP server runs with
    // libunftp's anonymous authenticator, so exposing it is the caller's
    // decision) — but a cross-machine transfer is the entire point here, and a
    // loopback default meant the printed sender command could not work. Pass
    // `--bind 127.0.0.1` for a local-only run.
    let bind = args.get_or("bind", "0.0.0.0");
    // libunftp also reports a refused bind from inside a spawned task, so the
    // port is checked here rather than discovered by the sender timing out.
    let port = choose_port(
        PortKind::Tcp,
        bind,
        args.get_or("port", "2121")
            .parse()
            .expect("--port must be a number"),
        args.flag("strict-port"),
    );
    let port = port as u32;
    let out_dir = args
        .get("out-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_out_dir);
    let idle_timeout = Duration::from_secs(
        args.get_or("timeout", "300")
            .parse()
            .expect("--timeout must be a number of seconds"),
    );

    // The FTP server is rooted in a staging directory, not the evidence
    // directory: uploads land here, and each verified item is then filed into
    // this run's evidence folder (`<out-dir>/ftp/<run-id>/`) alongside the
    // report — where an FTP client cannot reach it.
    let root = out_dir.join(PROTOCOL).join("incoming");
    std::fs::create_dir_all(&root).expect("create the FTP root directory");
    let root_str = root.to_string_lossy().to_string();

    // Clear stale sidecars so a previous run's manifests are not mistaken for
    // this one's. Payloads are left alone — they are received artifacts.
    clear_sidecars(&root);

    let server = XRNetServer::new(vec![PROTOCOLS::FTP], vec![port])
        .set_root_dir(&root_str)
        .set_bind_address(bind);

    println!("XRDS assessment receiver (FTP) serving {root_str} on ftp://{bind}:{port}/");

    // The address the sender must dial has to match what was actually bound —
    // printing a LAN address while listening on loopback only produces a
    // "connection refused" on the other machine, which looks like a network
    // fault rather than a flag.
    let reachable_at = if bind == "127.0.0.1" || bind == "localhost" {
        println!("  (loopback only — pass --bind 0.0.0.0 for a sender on another machine)");
        bind.to_string()
    } else if bind == "0.0.0.0" {
        local_ip()
    } else {
        bind.to_string()
    };
    print_peer_command(
        "sending",
        &format!("assess_ftp_sender.exe --url ftp://xrds:assessment@{reachable_at}:{port}/"),
    );
    println!("waiting for uploads ...");
    server.start().await;

    let mut report = SuiteReport::new(PROTOCOL, API, format!("ftp://{bind}:{port}/"), out_dir);
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut expected: Option<u32> = None;
    let mut last_progress = Instant::now();

    loop {
        if last_progress.elapsed() >= idle_timeout {
            eprintln!(
                "FAIL: nothing arrived for {} s ({} of {} item(s) complete)",
                idle_timeout.as_secs(),
                report.completed(),
                expected.unwrap_or(0)
            );
            let _ = report.finish();
            std::process::exit(1);
        }

        // Poll for sidecars; each is uploaded after its payload, so its
        // presence means the payload beside it is whole.
        for (manifest, sidecar) in new_manifests(&root, &seen) {
            seen.insert(sidecar.clone());
            expected = Some(manifest.suite_size);

            let payload_path = root.join(&manifest.file_name);
            let data = match std::fs::read(&payload_path) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!(
                        "FAIL: manifest {} arrived but {} is unreadable: {e}",
                        sidecar.display(),
                        payload_path.display()
                    );
                    let _ = report.finish();
                    std::process::exit(1);
                }
            };

            // No timing: this node is a passive upload target, so all it could
            // time is how long it sat waiting. The sender prints the transfer
            // figure.
            report.record(manifest, &data, None);
            last_progress = Instant::now();

            // The verified copy now lives in the run's evidence folder, so
            // clear the staging directory for the next run.
            let _ = std::fs::remove_file(&payload_path);
            let _ = std::fs::remove_file(&sidecar);

            if Some(report.completed() as u32) == expected {
                std::process::exit(report.finish());
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn clear_sidecars(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.to_string_lossy().ends_with(MANIFEST_SUFFIX) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Every readable sidecar in `root` not already processed. A half-written
/// sidecar simply fails to parse and is picked up on a later poll.
fn new_manifests(root: &Path, seen: &HashSet<PathBuf>) -> Vec<(Manifest, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.to_string_lossy().ends_with(MANIFEST_SUFFIX) || seen.contains(&path) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if let Ok(manifest) = Manifest::from_json(&bytes) {
            found.push((manifest, path));
        }
    }
    // Report in suite order even if the filesystem hands them back otherwise.
    found.sort_by_key(|(manifest, _)| manifest.suite_index);
    found
}
