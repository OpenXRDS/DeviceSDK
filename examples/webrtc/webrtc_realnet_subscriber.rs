// Standalone WebRTC subscriber for the real-network verification procedure
// in docs/xrds-net-release-readiness.md Phase 3 — see
// docs/done/xrds-net-webrtc-realnet-binaries.md.
//
// Like webrtc_realnet_publisher.rs, this uses the default production
// STUN/TURN config (no `set_ice_servers()` override) — see that file's
// module docs for the TURN-credentials env vars/flags. The subscriber
// doesn't strictly need TURN credentials to be *correct* (the publisher
// having them is what matters for the relay to exist), but if this
// subscriber is behind a restrictive NAT too, it needs its own relay
// candidates, so pass them here as well when in doubt.
//
// Run with (on Machine B, after the publisher has printed its session id):
//   cargo run --example webrtc_realnet_subscriber -- \
//     --signaling-addr ws://<signaling-server-ip>:<port>/ \
//     --session-id <id printed by the publisher> \
//     --turn-username <user> --turn-password <pass>
//
// Flags:
//   --signaling-addr <ws://host:port/>  Required.
//   --session-id <id>                   Required — from the publisher's
//                                        stdout banner.
//   --output-dir <path>                 Default: test_output.
//   --turn-username <user>              Optional — same effect as setting
//   --turn-password <pass>              XRDS_TURN_USERNAME/PASSWORD in the
//                                        environment. Provide both or
//                                        neither; one alone falls back to
//                                        STUN-only, same as setting none.

// `realnet_common.rs` is shared across all three `webrtc_realnet_*`
// binaries via this `#[path]` inclusion; each binary only uses part of its
// API, hence `allow(dead_code)` here rather than in the shared file.
#[path = "realnet_common.rs"]
#[allow(dead_code)]
mod realnet_common;

use std::time::Duration;

use xrds_net::common::ensure_rustls_crypto_provider;
use xrds_net::WebRTCClient;

#[tokio::main]
async fn main() {
    realnet_common::init_logger();

    ensure_rustls_crypto_provider();

    let args = realnet_common::Args::parse();
    realnet_common::apply_turn_credentials_from_args(&args);
    let signaling_addr = args.required("signaling-addr");
    let session_id = args.required("session-id");
    let output_dir = args.get_or("output-dir", realnet_common::DEFAULT_OUTPUT_DIR);

    std::fs::create_dir_all(&output_dir)
        .unwrap_or_else(|e| panic!("failed to create output dir {output_dir}: {e}"));

    // No `set_ice_servers()` override here either — see module docs above.
    let mut subscriber = WebRTCClient::new();
    subscriber
        .connect_to_signaling_server(&signaling_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to reach signaling server at {signaling_addr}: {e}"));

    subscriber
        .set_debug_dir_path(&output_dir)
        .await
        .expect("failed to set debug output dir");

    println!("Joining session {session_id}...");
    subscriber
        .join_session(&session_id)
        .await
        .expect("failed to join session — is the session id correct?");

    println!("Joined — exchanging ICE candidates...");
    subscriber
        .exchange_ice_candidates(true)
        .await
        .expect("ICE candidate exchange failed");

    let ice_connected =
        realnet_common::log_ice_summary("subscriber", &subscriber, Duration::from_secs(60)).await;
    if !ice_connected {
        println!("[subscriber] Not waiting for a stream — ICE never connected (see the message above).");
        let _ = subscriber.close_peer_connection().await;
        return;
    }

    println!("Waiting to receive the stream...");
    let video_path = subscriber
        .get_debug_video_file_path()
        .cloned()
        .expect("subscriber should have a debug video path set up after joining");

    // The publisher drives how long the stream runs; give it generous time
    // to arrive, then report whatever showed up. There's no explicit
    // "stream complete" signal to await here (see webrtc_file_stream.rs /
    // tests/webrtc_integration.rs for the file-size-polling pattern used
    // when both sides are in the same process — not directly reusable
    // across two independent processes without a shared clock).
    tokio::time::sleep(Duration::from_secs(45)).await;

    let _ = subscriber.close_peer_connection().await;

    match std::fs::metadata(&video_path) {
        Ok(meta) => {
            println!("Received stream saved to {video_path} ({} bytes).", meta.len());
            println!("To visually verify: ffplay \"{video_path}\"");
        }
        Err(e) => println!("No file at {video_path}: {e} — did the stream actually arrive?"),
    }
}
