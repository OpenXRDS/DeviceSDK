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

//! Assessment receiver — **WebSocket** node.
//!
//! Runs xrds-net's own `XRNetServer` (WS) and registers one binary-message
//! handler that reassembles the XRDS frames the sender pushes, verifies each
//! item against its manifest, and writes it out. Every frame is acknowledged,
//! so the sender can report per-item progress rather than hoping.
//!
//! The sender walks the whole content suite over one session; the manifest's
//! `suite_size` tells this binary how many items to expect, so it prints the
//! suite report and exits on its own.
//!
//! Run:
//! ```text
//! cargo run --example assess_ws_receiver -- --port 9301
//! cargo run --example assess_ws_sender   -- --url ws://127.0.0.1:9301/
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use xrds_net::common::enums::PROTOCOLS;
use xrds_net::server::XRNetServer;

#[path = "assess_common.rs"]
mod common;

use common::{
    choose_port, decode_frame, default_out_dir, encode_frame, kind_name, local_ip,
    print_peer_command, Args, Manifest, PortKind, Reassembly, SuiteReport, KIND_ACK, KIND_CHUNK,
    KIND_END, KIND_MANIFEST, KIND_NACK,
};

const PROTOCOL: &str = "ws";
const API: &str = "XRNetServer (WS) + registered binary handler";

struct State {
    asm: Reassembly,
    report: Option<SuiteReport>,
    expected: Option<u32>,
    done: bool,
    exit_code: i32,
}

fn handle_frame(bytes: Vec<u8>, state: &Arc<Mutex<State>>) -> Option<Vec<u8>> {
    let frame = match decode_frame(&bytes) {
        Ok(frame) => frame,
        Err(e) => {
            eprintln!("  ! dropped a message: {e}");
            return Some(encode_frame(KIND_NACK, 0, e.as_bytes()));
        }
    };

    let mut state = state.lock().unwrap();

    match frame.kind {
        KIND_MANIFEST => match Manifest::from_json(&frame.payload) {
            Ok(manifest) => {
                state.expected = Some(manifest.suite_size);
                state.asm.start_item(manifest);
                Some(encode_frame(KIND_ACK, frame.seq, b"manifest"))
            }
            Err(e) => {
                eprintln!("  ! bad manifest: {e}");
                Some(encode_frame(KIND_NACK, frame.seq, e.as_bytes()))
            }
        },

        KIND_CHUNK => {
            state.asm.accept_chunk(frame.seq, frame.payload);
            Some(encode_frame(KIND_ACK, frame.seq, b"chunk"))
        }

        KIND_END => {
            let Some(manifest) = state.asm.manifest.clone() else {
                let msg = "end frame before any manifest";
                eprintln!("  ! {msg}");
                return Some(encode_frame(KIND_NACK, frame.seq, msg.as_bytes()));
            };
            let elapsed = state.asm.elapsed();
            let Some(data) = state.asm.assembled() else {
                let msg = format!(
                    "incomplete: {} of {} chunks arrived",
                    state.asm.received_chunks(),
                    manifest.chunk_count
                );
                eprintln!("  ! {msg}");
                state.done = true;
                state.exit_code = 1;
                return Some(encode_frame(KIND_NACK, frame.seq, msg.as_bytes()));
            };

            let report = state.report.as_mut().expect("report present until finished");
            let pass = report.record(manifest, &data, elapsed);
            let completed = report.completed() as u32;

            if Some(completed) == state.expected {
                let report = state.report.take().expect("report present");
                state.exit_code = report.finish();
                state.done = true;
            }

            Some(encode_frame(
                KIND_ACK,
                frame.seq,
                if pass { b"PASS" } else { b"FAIL" },
            ))
        }

        other => {
            eprintln!("  ! unexpected frame kind: {}", kind_name(other));
            Some(encode_frame(KIND_NACK, frame.seq, b"unexpected frame"))
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let usage = "cargo run --example assess_ws_receiver -- --port 9301 [--strict-port] [--out-dir examples/assessment/received]";
    if args.flag("help") {
        println!("{usage}");
        return;
    }

    // Checked up front: XRNetServer binds inside a spawned task and swallows
    // the error, so without this a taken port means an endless "waiting ...".
    let port = choose_port(
        PortKind::Tcp,
        "0.0.0.0",
        args.get_or("port", "9301")
            .parse()
            .expect("--port must be a number"),
        args.flag("strict-port"),
    );
    let port = port as u32;
    let out_dir = args
        .get("out-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_out_dir);

    // Recorded in the evidence as the endpoint this node received on; the
    // sender's own address travels in the manifest.
    let listen = format!("ws://0.0.0.0:{port}/");

    let state = Arc::new(Mutex::new(State {
        asm: Reassembly::new(),
        report: Some(SuiteReport::new(PROTOCOL, API, listen.clone(), out_dir)),
        expected: None,
        done: false,
        exit_code: 1,
    }));

    let mut server = XRNetServer::new(vec![PROTOCOLS::WS], vec![port])
        // XRNetServer validates its root dir even when the protocol has no
        // use for one, so point it at a directory that certainly exists.
        .set_root_dir(".");

    let handler_state = Arc::clone(&state);
    server.register_handler("binary", move |data: Vec<u8>| {
        let ack = handle_frame(data, &handler_state);
        std::future::ready(ack)
    });

    println!("XRDS assessment receiver (WebSocket) listening on ws://0.0.0.0:{port}/");
    print_peer_command(
        "sending",
        &format!("assess_ws_sender.exe --url ws://{}:{port}/", local_ip()),
    );
    println!("waiting for a sender ...");
    server.start().await;

    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let state = state.lock().unwrap();
        if state.done {
            // Give the final ack a moment to reach the sender before the
            // process (and with it the socket) goes away.
            let code = state.exit_code;
            drop(state);
            tokio::time::sleep(Duration::from_millis(300)).await;
            std::process::exit(code);
        }
    }
}
