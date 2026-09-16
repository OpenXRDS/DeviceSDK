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

//! Assessment receiver — **MQTT** node.
//!
//! Pure protocol-agnostic API: `XrdsNet::listen(url)` subscribes (the URL path
//! is the topic) and hands back an `EventStream` this binary drains until the
//! whole content suite has arrived. The broker is a third node — any MQTT
//! broker works; the default is the KETI broker on the lab network. Prefer a
//! broker you control: against the public `test.mosquitto.org` the same suite
//! takes 35 s instead of 0.13 s, which measures the internet rather than the
//! SDK.
//!
//! Run:
//! ```text
//! cargo run --example assess_mqtt_receiver -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1
//! cargo run --example assess_mqtt_sender   -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1
//! ```

use std::path::PathBuf;
use std::time::{Duration, Instant};

use xrds_net::XrdsNet;

#[path = "assess_common.rs"]
mod common;

use common::{
    decode_frame, default_out_dir, kind_name, print_peer_command, Args, Manifest, Reassembly,
    SuiteReport, KIND_CHUNK, KIND_END, KIND_MANIFEST,
};

const PROTOCOL: &str = "mqtt";
const API: &str = "XrdsNet::listen -> EventStream::recv_timeout";
const DEFAULT_URL: &str = "mqtt://172.16.1.55:1883/xrds/assessment/demo";
const USAGE: &str = "cargo run --example assess_mqtt_receiver -- --url mqtt://<broker>:1883/<topic> [--timeout 300]";

fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let url = args.get_or("url", DEFAULT_URL).to_string();
    let out_dir = args
        .get("out-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_out_dir);
    let idle_timeout = Duration::from_secs(
        args.get_or("timeout", "300")
            .parse()
            .expect("--timeout must be a number of seconds"),
    );

    println!("XRDS assessment receiver (MQTT)");
    println!("  subscribing to {url}");

    // listen() connects *and* confirms the subscription (SUBACK) before it
    // returns, so there is no race with a sender started right after.
    let stream = match XrdsNet::listen(&url) {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("listen({url}) failed: {e}");
            std::process::exit(1);
        }
    };
    print_peer_command("sending", &format!("assess_mqtt_sender.exe --url {url}"));
    println!("  subscribed; waiting for a sender ...");

    let mut asm = Reassembly::new();
    let mut report = SuiteReport::new(PROTOCOL, API, url.clone(), out_dir);
    let mut expected: Option<u32> = None;
    // Bounded by inactivity rather than total runtime: a big suite over MQTT is
    // slow but should not be mistaken for a hang.
    let mut last_progress = Instant::now();

    loop {
        if last_progress.elapsed() >= idle_timeout {
            eprintln!(
                "FAIL: nothing arrived for {} s ({} of {} item(s) complete)",
                idle_timeout.as_secs(),
                report.completed(),
                expected.unwrap_or(0)
            );
            stream.close();
            let _ = report.finish();
            std::process::exit(1);
        }

        let event = match stream.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => event,
            Err(_) => continue, // idle tick, not a failure
        };

        let frame = match decode_frame(&event.payload) {
            Ok(frame) => frame,
            Err(e) => {
                // Retained messages from an earlier run land here too; they are
                // noise, not failures.
                println!("  (ignored a message: {e})");
                continue;
            }
        };

        match frame.kind {
            KIND_MANIFEST => match Manifest::from_json(&frame.payload) {
                Ok(manifest) => {
                    expected = Some(manifest.suite_size);
                    asm.start_item(manifest);
                    last_progress = Instant::now();
                }
                Err(e) => println!("  (ignored a bad manifest: {e})"),
            },
            KIND_CHUNK => {
                if asm.manifest.is_none() {
                    println!("  (ignored chunk {} arriving before a manifest)", frame.seq);
                    continue;
                }
                asm.accept_chunk(frame.seq, frame.payload);
                last_progress = Instant::now();
            }
            KIND_END => {
                let Some(manifest) = asm.manifest.clone() else {
                    // Almost always the retained end-frame of a previous run.
                    println!("  (ignored a stale end frame)");
                    continue;
                };
                let elapsed = asm.elapsed();
                let Some(data) = asm.assembled() else {
                    eprintln!(
                        "FAIL: {} incomplete — {} of {} chunks arrived",
                        manifest.file_name,
                        asm.received_chunks(),
                        manifest.chunk_count
                    );
                    stream.close();
                    let _ = report.finish();
                    std::process::exit(1);
                };

                report.record(manifest, &data, elapsed);
                last_progress = Instant::now();
                asm = Reassembly::new();

                if Some(report.completed() as u32) == expected {
                    stream.close();
                    std::process::exit(report.finish());
                }
            }
            other => println!("  (ignored a {} frame)", kind_name(other)),
        }
    }
}
