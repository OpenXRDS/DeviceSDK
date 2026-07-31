// Standalone WebRTC signaling server for the real-network verification
// procedure in docs/xrds-net-release-readiness.md Phase 3 — see
// docs/done/xrds-net-webrtc-realnet-binaries.md for why this exists as its own
// binary rather than reusing webrtc_file_stream.rs (which runs both peers
// from one process on loopback; this needs to run standalone so a real
// second machine can connect to it).
//
// Run with:
//   cargo run --example webrtc_realnet_signaling_server -- --port 9443
//
// Flags:
//   --port <u32>       Port to bind (default: 0 = OS-assigned; printed on
//                       startup either way).
//   --root-dir <path>  Working directory XRNetServer requires to exist
//                       (default: test_output; unused by the WEBRTC
//                       protocol itself, just a startup precondition).
//
// Runs until Ctrl-C. Give the printed port (with your machine's LAN/public
// IP — this can't reliably auto-detect which interface the other side can
// actually reach) to both the publisher and subscriber as their
// --signaling-addr.

// `realnet_common.rs` is shared across all three `webrtc_realnet_*`
// binaries via this `#[path]` inclusion; each binary only uses part of its
// API, hence `allow(dead_code)` here rather than in the shared file.
#[path = "realnet_common.rs"]
#[allow(dead_code)]
mod realnet_common;

use xrds_net::server::XRNetServer;
use xrds_net::PROTOCOLS;

#[tokio::main]
async fn main() {
    realnet_common::init_logger();

    let args = realnet_common::Args::parse();
    let port = args.get_u32_or("port", 0);
    let root_dir = args.get_or("root-dir", realnet_common::DEFAULT_OUTPUT_DIR);

    std::fs::create_dir_all(&root_dir)
        .unwrap_or_else(|e| panic!("failed to create root dir {root_dir}: {e}"));

    let server = XRNetServer::new(vec![PROTOCOLS::WEBRTC], vec![port]).set_root_dir(&root_dir);
    let ports = server.start_dynamic().await;
    let bound_port = ports[0];

    println!("WebRTC signaling server listening on 0.0.0.0:{bound_port}");
    println!(
        "Give both the publisher and subscriber: ws://<this machine's LAN/public IP>:{bound_port}/"
    );
    println!(
        "(can't reliably auto-detect which of this machine's interfaces the other side can \
         reach — check `ip addr` / `ifconfig` / your router's DHCP list)"
    );
    println!("Press Ctrl-C to stop.");

    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl-c");
    println!("Shutting down.");
}
