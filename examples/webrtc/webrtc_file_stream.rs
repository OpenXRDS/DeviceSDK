// External usage: publish a pre-recorded H.264 sample file over WebRTC and
// receive it on a subscriber — no camera/microphone hardware required.
//
// Complements webrtc_webcam_stream.rs, which needs a real webcam + mic and
// therefore can't run on a machine without one (or in CI). This example
// exercises the same signaling -> ICE -> media-transport path end to end
// using the crate's own test sample file instead, so it's fast,
// deterministic, and hardware-free — a template for smoke-testing the
// WebRTC path, e.g. from `docs/done/xrds-net-release-readiness.md`'s CI work.
//
// Run with: cargo run --example webrtc_file_stream
//
// Also demonstrates several fixes/practices from
// docs/done/xrds-net-webrtc-test-restructure.md and
// docs/done/xrds-net-webrtc-ice-config-fix.md:
//   - `ensure_rustls_crypto_provider()` — the guarded, idempotent crypto
//     install, instead of calling `CryptoProvider::install_default`
//     directly (which panics if something else in the process already
//     installed one first).
//   - `XRNetServer::start_dynamic()` for an OS-assigned port, rather than a
//     fixed port number that can collide with anything else running
//     locally.
//   - `WebRTCClient::set_ice_servers(vec![])` — both peers are on
//     `127.0.0.1` here, so host candidates alone are sufficient; this
//     skips remote STUN/TURN DNS resolution entirely, which is what makes
//     this example fast and reliable. A real deployment talking across an
//     actual network should NOT do this — just use `WebRTCClient::new()`
//     with no override, which defaults to the production STUN/TURN list.
//   - `WebRTCClient::close_peer_connection()` for deterministic teardown
//     (its ICE agent otherwise outlives the client, holding the mDNS
//     socket open until process exit).
//   - a data channel message alongside the media stream, to touch that
//     part of the API too.

use std::time::Duration;

use xrds_net::common::ensure_rustls_crypto_provider;
use xrds_net::server::XRNetServer;
use xrds_net::{VideoSource, WebRTCClient, PROTOCOLS};

const SAMPLE_VIDEO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/crates/xrds-net/samples/sample_video.h264"
);
const STREAM_SECONDS: u64 = 8;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    let _ = env_logger::try_init();

    ensure_rustls_crypto_provider();

    // XRNetServer::start_dynamic() requires root_dir to already exist.
    std::fs::create_dir_all("test_output").expect("failed to create test_output/");

    let port = run_signaling_server().await;
    let addr = format!("ws://127.0.0.1:{port}/");

    // --- Publisher ---
    let mut publisher = WebRTCClient::new();
    publisher.set_ice_servers(vec![]); // loopback demo — see module docs above
    publisher
        .connect_to_signaling_server(&addr)
        .await
        .expect("publisher failed to reach signaling server");
    let session_id = publisher
        .create_session()
        .await
        .expect("failed to create session");
    publisher
        .publish(&session_id)
        .await
        .expect("failed to publish offer");

    // --- Subscriber: saves whatever it receives under test_output/ ---
    let mut subscriber = WebRTCClient::new();
    subscriber.set_ice_servers(vec![]);
    subscriber
        .connect_to_signaling_server(&addr)
        .await
        .expect("subscriber failed to reach signaling server");
    subscriber
        .set_debug_dir_path("test_output")
        .await
        .expect("failed to set debug output dir");
    subscriber
        .join_session(&session_id)
        .await
        .expect("failed to join session");

    publisher
        .wait_for_subscriber(10)
        .await
        .expect("subscriber never joined");
    tokio::try_join!(
        publisher.exchange_ice_candidates(false),
        subscriber.exchange_ice_candidates(true),
    )
    .expect("ICE candidate exchange failed");

    println!("Signaling + ICE complete.");

    // A data channel message, alongside the media stream.
    publisher
        .send_data_channel_message("hello from webrtc_file_stream example")
        .await
        .expect("failed to send data channel message");

    let video_path = subscriber
        .get_debug_video_file_path()
        .cloned()
        .expect("subscriber should have a debug video path set up after joining");

    let video_file = std::fs::File::open(SAMPLE_VIDEO)
        .unwrap_or_else(|e| panic!("failed to open sample video at {SAMPLE_VIDEO}: {e}"));
    publisher
        .start_stream(VideoSource::new(Box::new(video_file)), None)
        .await
        .expect("failed to start streaming");

    println!("Streaming {SAMPLE_VIDEO} for {STREAM_SECONDS}s...");
    tokio::time::sleep(Duration::from_secs(STREAM_SECONDS)).await;

    publisher.stop_stream().await.expect("failed to stop stream");

    // Deterministic teardown — see module docs above.
    let _ = publisher.close_peer_connection().await;
    let _ = subscriber.close_peer_connection().await;

    println!("Done. Received stream saved to {video_path}.");
    println!("To visually verify the received video: ffplay \"{video_path}\"");
}

/// Starts the signaling server on an OS-assigned port and returns it once
/// the listener is actually bound — no guessed port, no fixed post-start
/// sleep needed to know the server is ready.
async fn run_signaling_server() -> u32 {
    let server = XRNetServer::new(vec![PROTOCOLS::WEBRTC], vec![0]).set_root_dir("test_output");
    let ports = server.start_dynamic().await;
    ports[0]
}
