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

//! Assessment sender — **MQTT** node.
//!
//! Publishes the whole content suite (one file per supported XR content type)
//! to one topic, so a single run covers the `mqtt` column of the content ×
//! protocol matrix.
//!
//! Pure protocol-agnostic API: `XrdsNet::dispatch(url, bytes)` publishes to the
//! topic in the URL path. Identical call shape to the WebSocket sender's
//! `open`/`send` — the scheme is the only thing that differs.
//!
//! Note the cost this makes visible: `dispatch` is a one-shot verb (connect,
//! publish, drop), so every chunk opens its own broker connection. On a LAN
//! broker that is ~3 ms per chunk (the whole suite publishes in 0.1 s); against
//! the public `test.mosquitto.org` it is ~0.8 s per chunk and 35 s for the
//! suite. Use a broker you control unless measuring the internet is the point.
//!
//! Run the receiver first:
//! ```text
//! cargo run --example assess_mqtt_receiver -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1
//! cargo run --example assess_mqtt_sender   -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1
//! ```

use std::time::Instant;

use xrds_net::XrdsNet;

#[path = "assess_common.rs"]
mod common;

use common::{
    encode_frame, new_run_id, print_send_banner, resolve_items, Args, Manifest, KIND_CHUNK,
    KIND_END, KIND_MANIFEST,
};

const PROTOCOL: &str = "mqtt";
const API: &str = "XrdsNet::dispatch (publish; URL path = topic)";
const DEFAULT_URL: &str = "mqtt://172.16.1.55:1883/xrds/assessment/demo";
/// rumqttc's default max packet size is 10 KiB in each direction, so a frame
/// must stay comfortably under that — header included.
const DEFAULT_CHUNK: &str = "4096";
const USAGE: &str = "cargo run --example assess_mqtt_sender -- --url mqtt://<broker>:1883/<topic> [--files a,b --types t1,t2 | --file <path> --content-type <type>] [--chunk-size 4096]";

fn main() {
    let args = Args::parse();
    if args.flag("help") {
        println!("{USAGE}");
        return;
    }

    let url = args.get_or("url", DEFAULT_URL).to_string();
    let chunk_size: usize = args
        .get_or("chunk-size", DEFAULT_CHUNK)
        .parse::<usize>()
        .expect("--chunk-size must be a number")
        .max(1);

    let items = resolve_items(&args);
    let run_id = new_run_id();
    print_send_banner(PROTOCOL, API, &url, &run_id, &items, chunk_size);

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

        dispatch(&url, encode_frame(KIND_MANIFEST, 0, &manifest.to_json()), "manifest");
        for (seq, chunk) in item.data.chunks(chunk_size).enumerate() {
            dispatch(
                &url,
                encode_frame(KIND_CHUNK, seq as u32, chunk),
                &format!("chunk {seq}"),
            );
        }
        dispatch(&url, encode_frame(KIND_END, manifest.chunk_count, b""), "end");

        println!(
            "  [{}/{}] {:<15} {:<30} {:>10} B  {:>4} chunk(s)  {:.3} s",
            index + 1,
            suite_size,
            item.content_type,
            item.file_name,
            item.data.len(),
            manifest.chunk_count,
            at.elapsed().as_secs_f64()
        );
    }

    let total: usize = items.iter().map(|i| i.data.len()).sum();
    println!();
    println!(
        "  published {} item(s), {total} bytes in {:.3} s",
        items.len(),
        started.elapsed().as_secs_f64()
    );
    println!("  the receiver prints the suite report");
}

fn dispatch(url: &str, payload: Vec<u8>, what: &str) {
    if let Err(e) = XrdsNet::dispatch(url, payload) {
        eprintln!("FAIL: dispatch of the {what} failed: {e}");
        std::process::exit(1);
    }
}
