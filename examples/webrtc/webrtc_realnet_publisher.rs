// Standalone WebRTC publisher for the real-network verification procedure
// in docs/xrds-net-release-readiness.md Phase 3 — see
// docs/done/xrds-net-webrtc-realnet-binaries.md.
//
// Unlike webrtc_file_stream.rs, this does NOT call
// `WebRTCClient::set_ice_servers(vec![])` — it uses the default,
// production STUN/TURN list (`build_ice_servers()`), so it actually
// exercises the real network path Phase 3 needs verified. TURN relay
// credentials are picked up from XRDS_TURN_USERNAME/XRDS_TURN_PASSWORD in
// the environment, or from --turn-username/--turn-password below (either
// works — the flags just set the same env vars for you). Without either,
// this still runs, just STUN-only (logged as a warning).
//
// Run with (on Machine A, alongside webrtc_realnet_signaling_server):
//   cargo run --example webrtc_realnet_publisher -- \
//     --signaling-addr ws://<signaling-server-ip>:<port>/ \
//     --turn-username <user> --turn-password <pass>
//
// Flags:
//   --signaling-addr <ws://host:port/>  Required.
//   --file <path>                       Default: none — streams the
//                                        crate's sample video, which is
//                                        embedded into this binary at
//                                        compile time (so it works even
//                                        if you copy the built binary to
//                                        another machine). Pass a real
//                                        path here to stream something
//                                        else instead.
//   --stream-seconds <u64>              Default: 30.
//   --turn-username <user>              Optional — same effect as setting
//   --turn-password <pass>              XRDS_TURN_USERNAME/PASSWORD in the
//                                        environment. Provide both or
//                                        neither; one alone falls back to
//                                        STUN-only, same as setting none.
//
// Prints the session id in a hard-to-miss banner — copy it to the
// subscriber's --session-id. Also prints the final ICE connection state
// and the winning candidate pair type (host/srflx/relay) once connected —
// a `relay` result is the strongest evidence the TURN path specifically
// worked, not just direct/STUN.

// `realnet_common.rs` is shared across all three `webrtc_realnet_*`
// binaries via this `#[path]` inclusion; each binary only uses part of its
// API, hence `allow(dead_code)` here rather than in the shared file.
#[path = "realnet_common.rs"]
#[allow(dead_code)]
mod realnet_common;

use std::time::Duration;

use xrds_net::common::ensure_rustls_crypto_provider;
use xrds_net::{VideoSource, WebRTCClient};

#[tokio::main]
async fn main() {
    realnet_common::init_logger();

    ensure_rustls_crypto_provider();

    let args = realnet_common::Args::parse();
    realnet_common::apply_turn_credentials_from_args(&args);
    let signaling_addr = args.required("signaling-addr");
    let file_path = args.get("file").map(str::to_string);
    let stream_seconds = args.get_u64_or("stream-seconds", 30);

    // No `set_ice_servers()` override here — see module docs above.
    let mut publisher = WebRTCClient::new();
    publisher
        .connect_to_signaling_server(&signaling_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to reach signaling server at {signaling_addr}: {e}"));

    let session_id = publisher
        .create_session()
        .await
        .expect("failed to create session");
    realnet_common::print_session_id_banner(&session_id);

    publisher
        .publish(&session_id)
        .await
        .expect("failed to publish offer");

    println!("Waiting for a subscriber to join (up to 120s)...");
    publisher
        .wait_for_subscriber(120)
        .await
        .expect("no subscriber joined within 120s");

    println!("Subscriber joined — exchanging ICE candidates...");
    publisher
        .exchange_ice_candidates(false)
        .await
        .expect("ICE candidate exchange failed");

    let ice_connected =
        realnet_common::log_ice_summary("publisher", &publisher, Duration::from_secs(60)).await;
    if !ice_connected {
        println!("[publisher] Not streaming — ICE never connected (see the message above).");
        let _ = publisher.close_peer_connection().await;
        return;
    }

    publisher
        .send_data_channel_message("hello from webrtc_realnet_publisher")
        .await
        .expect("failed to send data channel message");

    let source: Box<dyn std::io::Read + Send> = match &file_path {
        Some(path) => Box::new(
            std::fs::File::open(path).unwrap_or_else(|e| panic!("failed to open {path}: {e}")),
        ),
        None => Box::new(std::io::Cursor::new(realnet_common::DEFAULT_SAMPLE_VIDEO_BYTES)),
    };
    publisher
        .start_stream(VideoSource::new(source), None)
        .await
        .expect("failed to start streaming");

    match &file_path {
        Some(path) => println!("Streaming {path} for {stream_seconds}s..."),
        None => println!("Streaming embedded sample video for {stream_seconds}s..."),
    }
    tokio::time::sleep(Duration::from_secs(stream_seconds)).await;

    publisher
        .stop_stream()
        .await
        .expect("failed to stop stream");
    let _ = publisher.close_peer_connection().await;

    println!("Done.");
}
