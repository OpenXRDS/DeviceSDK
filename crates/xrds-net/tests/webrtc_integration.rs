//! WebRTC integration tests — real signaling server, real ICE/DTLS/SRTP
//! handshakes, real mDNS multicast socket. Genuinely end-to-end, not unit
//! tests — that's why they live under `tests/` rather than in the crate's
//! own `#[cfg(test)]` modules: Cargo compiles each `tests/*.rs` file as its
//! own process, which isolates this suite's shared OS resources (mDNS on
//! UDP 5353, signaling server sockets) from the unrelated http/ws/ftp/mqtt
//! unit tests in `cargo test -p xrds-net --lib`.
//!
//! The tests in *this* file still share one process with each other, so
//! they're serialized against each other via `#[serial(webrtc)]` — moving
//! to `tests/` isolates this file from other test binaries, it doesn't
//! remove the need for serialization within the file itself.
//!
//! Ports are OS-assigned (`XRNetServer::start_dynamic` binds to `:0` and
//! reports back the actual port) rather than derived from `line!()` — the
//! old scheme broke whenever lines were added/removed above a test.
//!
//! See docs/done/xrds-net-webrtc-test-restructure.md for the plan this
//! implements, and docs/done/xrds-net-crypto-consolidation.md for the
//! logger/crypto-provider/TURN-scheme bugs this suite surfaced.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use serial_test::serial;
use tokio::time::{sleep, Duration};
use webrtc::track::track_remote::TrackRemote;

use xrds_net::client::media::VideoTrackHandler;
use xrds_net::client::webrtc_client::is_valid_h264;
use xrds_net::common::{append_to_path, payload_str_to_vector_str};
use xrds_net::server::XRNetServer;
use xrds_net::{VideoSource, WebRTCClient, PROTOCOLS};

static DEFAULT_DEBUG_FILE_PATH: &str = "test_output";

fn init_logger() {
    use std::sync::Once;
    static LOGGER_INIT: Once = Once::new();
    LOGGER_INIT.call_once(|| {
        std::env::set_var("RUST_LOG", "info");
        let _ = pretty_env_logger::try_init();
    });
}

fn init_crypto() {
    xrds_net::common::ensure_rustls_crypto_provider();
}

/// Starts a signaling server on an OS-assigned port (`:0`) and returns once
/// the listener is actually bound, along with the port it landed on — no
/// `line!()`-derived guess, and no fixed post-start sleep needed to know the
/// server is ready to accept connections.
async fn run_server(protocol: PROTOCOLS) -> (tokio::task::JoinHandle<()>, u32) {
    let crnt_dir = std::env::current_dir().unwrap();
    let target_dir = append_to_path(crnt_dir, "/test_root_dir");
    let root_dir = target_dir.as_path().to_str().unwrap().to_string();

    let server = XRNetServer::new(vec![protocol], vec![0]).set_root_dir(root_dir.as_str());
    let ports = server.start_dynamic().await;
    let actual_port = ports[0];

    // `start_dynamic` already spawned the accept loop as an independent
    // task; this handle exists only so call sites can keep calling
    // `.abort()` uniformly (a no-op here, matching the pre-existing
    // behavior of the outer wrapper task used to look the same way).
    let handle = tokio::spawn(async {});

    (handle, actual_port)
}

/// Explicitly closes both peer connections so their ICE agents release the
/// shared mDNS multicast socket (UDP 5353) before the next test starts.
/// `Drop` can't do this — the cleanup is async — and without it the next
/// test's ICE reproducibly fails with a terminal `Failed` state.
async fn teardown(publisher: &mut WebRTCClient, subscriber: &mut WebRTCClient) {
    let _ = publisher.close_peer_connection().await;
    let _ = subscriber.close_peer_connection().await;
}

// ICE-connect polling used to be a private helper here
// (`wait_for_ice_connected`); promoted to `WebRTCClient::wait_for_ice_connected`
// itself so it's reusable outside this test suite too (e.g. the
// `webrtc_realnet_*` example binaries) without depending on the `webrtc`
// crate's `RTCIceConnectionState` enum directly. See
// docs/done/xrds-net-webrtc-realnet-binaries.md.

/// Polls `path`'s size until it stops growing (3 consecutive unchanged,
/// non-zero reads, 2s apart) or `max_wait` elapses. Replaces a blind
/// `sleep(120)` after starting a file transfer — exits as soon as the
/// receiver stops writing instead of always waiting for the worst case.
async fn wait_for_file_to_stabilize(path: &str, max_wait: Duration) -> Result<(), String> {
    let start = Instant::now();
    let mut last_size: Option<u64> = None;
    let mut stable_rounds = 0;
    loop {
        let size = std::fs::metadata(path).map(|m| m.len()).ok();
        match (size, last_size) {
            (Some(s), Some(prev)) if s == prev && s > 0 => {
                stable_rounds += 1;
                if stable_rounds >= 3 {
                    return Ok(());
                }
            }
            _ => stable_rounds = 0,
        }
        last_size = size;

        if start.elapsed() > max_wait {
            return Err(format!(
                "timed out after {:?} waiting for {path} to finish growing (last size: {last_size:?})",
                max_wait
            ));
        }
        sleep(Duration::from_secs(2)).await;
    }
}

pub struct CustomVideoProcessor {}

impl VideoTrackHandler for CustomVideoProcessor {
    fn handle_video_track<'a>(
        &'a self,
        track: Arc<TrackRemote>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
        Box::pin(async move {
            println!("Custom video processor started for track: {}", track.id());
            let mut packet_count = 0;
            while let Ok((_rtp_packet, _attributes)) = track.read_rtp().await {
                packet_count += 1;
            }
            println!("Video processing ended: {} packets", packet_count);
            Ok(())
        })
    }
}

async fn custom_audio_handler(
    track: Arc<TrackRemote>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Custom audio processing started");
    let mut packet_count = 0;
    while let Ok((_rtp_packet, _)) = track.read_rtp().await {
        packet_count += 1;
    }
    println!("Audio processing ended: {} packets", packet_count);
    Ok(())
}

/// Establish a publisher + subscriber pair over a fresh signaling server,
/// fully connected (session created, published, joined, ICE exchanged).
async fn establish_complete_webrtc_connection(
) -> Result<(WebRTCClient, WebRTCClient, String, tokio::task::JoinHandle<()>), String> {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let addr_str = format!("ws://127.0.0.1:{}/", port);

    let mut publisher = WebRTCClient::new();
    // Both peers are on 127.0.0.1 — host candidates connect instantly, and
    // remote STUN/TURN gathering only adds DNS-resolution latency (this
    // sandbox has no working IPv6, so every one of those lookups fails
    // before falling through). See docs/done/xrds-net-webrtc-ice-config-fix.md.
    publisher.set_ice_servers(vec![]);
    publisher
        .connect_to_signaling_server(&addr_str)
        .await
        .map_err(|e| e.to_string())?;
    let session_id = publisher.create_session().await?;
    let _msg = publisher.publish(&session_id).await?;

    let mut subscriber = WebRTCClient::new();
    subscriber.set_ice_servers(vec![]);
    subscriber
        .connect_to_signaling_server(&addr_str)
        .await
        .map_err(|e| e.to_string())?;
    subscriber.set_debug_dir_path(DEFAULT_DEBUG_FILE_PATH).await?;

    subscriber
        .join_session(session_id.as_str())
        .await
        .map_err(|e| e.to_string())?;

    publisher.wait_for_subscriber(10).await?;

    tokio::try_join!(
        publisher.exchange_ice_candidates(false),
        subscriber.exchange_ice_candidates(true)
    )?;

    publisher.wait_for_ice_connected(Duration::from_secs(45)).await?;

    Ok((publisher, subscriber, session_id, server_handle))
}

/// As `establish_complete_webrtc_connection`, but lets the caller configure
/// the subscriber (e.g. register a custom track handler) before it joins.
async fn establish_webrtc_with_custom_subscriber<F>(
    subscriber_setup: F,
) -> Result<(WebRTCClient, WebRTCClient, String, tokio::task::JoinHandle<()>), String>
where
    F: FnOnce(&mut WebRTCClient),
{
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let addr_str = format!("ws://127.0.0.1:{}/", port);

    let mut publisher = WebRTCClient::new();
    publisher.set_ice_servers(vec![]);
    publisher
        .connect_to_signaling_server(&addr_str)
        .await
        .map_err(|e| e.to_string())?;
    let session_id = publisher.create_session().await?;
    let _msg = publisher.publish(&session_id).await?;

    let mut subscriber = WebRTCClient::new();
    subscriber.set_ice_servers(vec![]);
    subscriber
        .connect_to_signaling_server(&addr_str)
        .await
        .map_err(|e| e.to_string())?;
    subscriber.set_debug_dir_path(DEFAULT_DEBUG_FILE_PATH).await?;
    subscriber_setup(&mut subscriber);
    subscriber
        .join_session(session_id.as_str())
        .await
        .map_err(|e| e.to_string())?;

    publisher.wait_for_subscriber(10).await?;
    tokio::try_join!(
        publisher.exchange_ice_candidates(false),
        subscriber.exchange_ice_candidates(true)
    )?;

    publisher.wait_for_ice_connected(Duration::from_secs(45)).await?;

    Ok((publisher, subscriber, session_id, server_handle))
}

/* -------------------------- client-side tests -------------------------- */

// Slowest test in the suite (60-120s even on the happy path — a full H.264
// file transfer over a real ICE/DTLS/SRTP connection) — gated behind
// `--ignored` so `cargo test`/`cargo test --tests` stays fast by default.
// Run explicitly with `cargo test -p xrds-net --test webrtc_integration --
// --ignored` (or in CI).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(webrtc)]
#[ignore = "slow: full real-time H.264 file transfer, 60-120s"]
async fn test_client_webrtc_send_video_file() {
    init_crypto();

    let (mut publisher, mut subscriber, _session_id, server_handle) =
        establish_complete_webrtc_connection()
            .await
            .expect("Failed to establish connection");

    let video_debug_file_path = subscriber
        .get_debug_video_file_path()
        .cloned()
        .expect("subscriber should have a debug video file path set up after joining");

    let sample_file_path = "samples/sample_video.h264";
    let file = std::fs::read(sample_file_path).expect("Failed to read sample file");

    let video_file = std::fs::File::open(sample_file_path).expect("Failed to open sample file");
    publisher
        .start_stream(VideoSource::new(Box::new(video_file)), None)
        .await
        .expect("Failed to start file streaming");

    wait_for_file_to_stabilize(&video_debug_file_path, Duration::from_secs(150))
        .await
        .expect("video transfer should complete (file should stop growing) within timeout");
    let received_file = std::fs::read(&video_debug_file_path).expect("Failed to read received file");
    teardown(&mut publisher, &mut subscriber).await;
    server_handle.abort();

    assert!(is_valid_h264(&file), "Sent file is not valid H264");
    assert!(is_valid_h264(&received_file), "Received file is not valid H264");

    let size_ratio = (file.len() as f64) / (received_file.len() as f64);
    assert!(
        size_ratio > 0.9 && size_ratio < 1.1,
        "File size mismatch: sent={}, received={}",
        file.len(),
        received_file.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(webrtc)]
async fn test_client_webrtc_webcam_video_stream() {
    init_logger();
    init_crypto();

    let (mut publisher, mut subscriber, _session_id, server_handle) =
        establish_complete_webrtc_connection()
            .await
            .expect("Failed to establish WebRTC connection");

    publisher
        .start_stream(
            VideoSource::new(Box::new(
                std::fs::File::open("samples/sample_video.h264").expect("Failed to open sample file"),
            )),
            None,
        )
        .await
        .expect("Failed to start streaming");

    sleep(Duration::from_secs(10)).await;
    publisher.stop_stream().await.expect("Failed to stop streaming");
    teardown(&mut publisher, &mut subscriber).await;
    server_handle.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(webrtc)]
async fn test_client_webrtc_datachannel() {
    init_logger();
    init_crypto();

    let (mut publisher, mut subscriber, _session_id, server_handle) =
        establish_complete_webrtc_connection()
            .await
            .expect("Failed to establish WebRTC connection");

    publisher
        .send_data_channel_message("hello webrtc")
        .await
        .expect("Failed to send data channel message");

    sleep(Duration::from_secs(10)).await;
    teardown(&mut publisher, &mut subscriber).await;
    server_handle.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(webrtc)]
async fn test_client_webrtc_custom_handler() {
    init_logger();
    init_crypto();

    let (mut publisher, mut subscriber, _session_id, server_handle) =
        establish_webrtc_with_custom_subscriber(|subscriber| {
            let video_processor = Arc::new(CustomVideoProcessor {});
            subscriber.register_video_handler(video_processor);
        })
        .await
        .expect("Failed to establish connection with custom handler");

    let video_file =
        std::fs::File::open("samples/sample_video.h264").expect("Failed to open sample file");
    publisher
        .start_stream(VideoSource::new(Box::new(video_file)), None)
        .await
        .expect("Failed to start streaming");

    sleep(Duration::from_secs(10)).await;
    publisher.stop_stream().await.expect("Failed to stop streaming");
    teardown(&mut publisher, &mut subscriber).await;
    server_handle.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(webrtc)]
async fn test_client_webrtc_custom_callback_fn() {
    init_logger();
    init_crypto();

    let (mut publisher, mut subscriber, _session_id, server_handle) =
        establish_webrtc_with_custom_subscriber(|subscriber| {
            subscriber.on_audio_track(|track| Box::pin(custom_audio_handler(track)));
        })
        .await
        .expect("Failed to establish connection");

    let video_file =
        std::fs::File::open("samples/sample_video.h264").expect("Failed to open sample file");
    publisher
        .start_stream(VideoSource::new(Box::new(video_file)), None)
        .await
        .expect("Failed to start streaming");

    sleep(Duration::from_secs(10)).await;
    publisher.stop_stream().await.expect("Failed to stop streaming");
    teardown(&mut publisher, &mut subscriber).await;
    server_handle.abort();
}

/* -------------------------- server-side tests --------------------------- */

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_run() {
    let (server_handle, _port) = run_server(PROTOCOLS::WEBRTC).await;

    sleep(Duration::from_secs(15)).await;

    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_connect_signal() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let mut webrtc_client = WebRTCClient::new();
    let addr_str = "ws://127.0.0.1".to_owned() + ":" + &port.to_string() + "/";

    webrtc_client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let client_id = webrtc_client
        .get_client_id()
        .expect("server should have assigned a client_id on connect");
    assert!(
        uuid::Uuid::parse_str(client_id).is_ok(),
        "client_id should be a well-formed UUID, got: {client_id}"
    );

    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_multiuser() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let addr_str = "ws://127.0.0.1".to_owned() + ":" + &port.to_string() + "/";

    let mut webrtc_client = WebRTCClient::new();
    webrtc_client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let mut webrtc_client2 = WebRTCClient::new();
    webrtc_client2
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let client_id = webrtc_client
        .get_client_id()
        .expect("server should have assigned a client_id to client 1");
    let client_id2 = webrtc_client2
        .get_client_id()
        .expect("server should have assigned a client_id to client 2");

    assert!(uuid::Uuid::parse_str(client_id).is_ok());
    assert!(uuid::Uuid::parse_str(client_id2).is_ok());
    assert_ne!(
        client_id, client_id2,
        "each concurrently-connected client should get a distinct client_id"
    );

    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_session_create() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let mut webrtc_client = WebRTCClient::new();
    let addr_str = "ws://127.0.0.1".to_owned() + ":" + &port.to_string() + "/";

    webrtc_client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_id = webrtc_client
        .create_session()
        .await
        .expect("Failed to create session");

    assert!(
        uuid::Uuid::parse_str(&session_id).is_ok(),
        "session_id should be a well-formed UUID, got: {session_id}"
    );

    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_session_list() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let mut webrtc_client = WebRTCClient::new();
    let addr_str = "ws://127.0.0.1".to_owned() + ":" + &port.to_string() + "/";

    webrtc_client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_id = webrtc_client
        .create_session()
        .await
        .expect("Failed to create session");

    let session_ids = webrtc_client
        .list_sessions()
        .await
        .expect("Failed to list sessions");
    let session_list = payload_str_to_vector_str(&session_ids.session_id);

    assert!(
        session_list.contains(&session_id),
        "created session {session_id} should appear in list_sessions(): {session_list:?}"
    );

    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_session_multiple() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let mut webrtc_client = WebRTCClient::new();
    let addr_str = "ws://127.0.0.1".to_owned() + ":" + &port.to_string() + "/";

    webrtc_client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_id_1 = webrtc_client
        .create_session()
        .await
        .expect("Failed to create session");
    let session_id_2 = webrtc_client
        .create_session()
        .await
        .expect("Failed to create session");
    let session_ids = webrtc_client
        .list_sessions()
        .await
        .expect("Failed to list sessions");
    let session_list = payload_str_to_vector_str(&session_ids.session_id);

    assert_eq!(
        session_list.len(),
        2,
        "expected exactly 2 sessions after creating 2, got: {session_list:?}"
    );
    assert!(session_list.contains(&session_id_1));
    assert!(session_list.contains(&session_id_2));

    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_session_close() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let mut webrtc_client = WebRTCClient::new();
    let addr_str = "ws://127.0.0.1".to_owned() + ":" + &port.to_string() + "/";

    webrtc_client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let _ = webrtc_client
        .create_session()
        .await
        .expect("Failed to create session");

    let session_ids = webrtc_client
        .list_sessions()
        .await
        .expect("Failed to list sessions");
    let session_list = payload_str_to_vector_str(&session_ids.session_id);
    assert_eq!(session_list.len(), 1, "expected exactly 1 session before close");

    let session_id = session_list[0].clone();

    webrtc_client
        .close_session(session_id.as_str())
        .await
        .expect("Failed to close session");

    let lists = webrtc_client
        .list_sessions()
        .await
        .expect("Failed to list sessions");
    let session_list_after = payload_str_to_vector_str(lists.session_id.as_str());
    assert!(
        !session_list_after.contains(&session_id),
        "session {session_id} should be gone after close_session(), remaining: {session_list_after:?}"
    );

    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_session_join() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let mut webrtc_publisher = WebRTCClient::new();
    webrtc_publisher.set_ice_servers(vec![]);
    let addr_str = "ws://127.0.0.1".to_owned() + ":" + &port.to_string() + "/";

    let mut webrtc_subscriber = WebRTCClient::new();
    webrtc_subscriber.set_ice_servers(vec![]);

    webrtc_publisher
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let crt_session_id = webrtc_publisher
        .create_session()
        .await
        .expect("Failed to create session");
    let _msg = webrtc_publisher
        .publish(&crt_session_id)
        .await
        .expect("Failed to publish");

    webrtc_subscriber
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_ids = webrtc_subscriber
        .list_sessions()
        .await
        .expect("Failed to list sessions");

    let session_id = payload_str_to_vector_str(session_ids.session_id.as_str());
    let session_id = session_id[0].clone();

    let join_result = webrtc_subscriber
        .join_session(session_id.as_str())
        .await
        .map_err(|e| e.to_string());
    if join_result.is_err() {
        println!("Join session error: {}", join_result.clone().err().unwrap());
    }
    assert!(join_result.is_ok());
    teardown(&mut webrtc_publisher, &mut webrtc_subscriber).await;
    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_session_list_participants() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

    let mut client = WebRTCClient::new();
    client.set_ice_servers(vec![]);
    client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_id = client
        .create_session()
        .await
        .expect("Failed to create session");
    client.publish(&session_id).await.expect("Failed to publish");

    let session_ids = client
        .list_sessions()
        .await
        .expect("Failed to list sessions");
    let session_list = payload_str_to_vector_str(&session_ids.session_id);

    let session_id = session_list[0].clone();
    client
        .join_session(&session_id)
        .await
        .expect("Failed to join session");

    let participants_msg = client
        .list_participants(&session_id)
        .await
        .expect("Failed to list participants");
    let participants = payload_str_to_vector_str(participants_msg.ice_candidates.unwrap().as_str());

    let client_id = client.get_client_id().unwrap();
    assert!(participants.contains(client_id));

    let _ = client.close_peer_connection().await;
    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_session_leave() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

    let mut client = WebRTCClient::new();
    client.set_ice_servers(vec![]);
    client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_id = client
        .create_session()
        .await
        .expect("Failed to create session");
    client.publish(&session_id).await.expect("Failed to publish");

    let session_list_msg = client
        .list_sessions()
        .await
        .expect("Failed to list sessions");
    let session_list = payload_str_to_vector_str(&session_list_msg.session_id);

    let session_id = session_list[0].clone();
    client
        .join_session(session_id.as_str())
        .await
        .expect("Failed to join session");

    let participants = client
        .list_participants(&session_id)
        .await
        .expect("Failed to list participants");
    let participants_vec = payload_str_to_vector_str(participants.ice_candidates.unwrap().as_str());
    assert_eq!(participants_vec.len(), 1);

    client
        .leave_session(&session_id)
        .await
        .expect("Failed to leave session");

    let participants = client
        .list_participants(&session_id)
        .await
        .expect("Failed to list participants");
    let participants_vec = payload_str_to_vector_str(participants.ice_candidates.unwrap().as_str());
    assert_eq!(participants_vec.len(), 0);

    let _ = client.close_peer_connection().await;
    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_offer() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

    let mut client = WebRTCClient::new();
    client.set_ice_servers(vec![]);
    client
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_id = client
        .create_session()
        .await
        .expect("Failed to create session");
    client.publish(&session_id).await.expect("Failed to publish");
    let offer_len = client.get_offer().unwrap().sdp.len();
    assert!(offer_len > 0);

    let _ = client.close_peer_connection().await;
    server_handle.abort();
}

#[tokio::test]
#[serial(webrtc)]
async fn test_server_webrtc_answer() {
    let (server_handle, port) = run_server(PROTOCOLS::WEBRTC).await;

    let addr_str = "ws://127.0.0.1".to_owned() + ":" + port.to_string().as_str() + "/";

    let mut publisher = WebRTCClient::new();
    publisher.set_ice_servers(vec![]);
    publisher
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    let session_id = publisher
        .create_session()
        .await
        .expect("Failed to create session");

    publisher.publish(&session_id).await.expect("Failed to publish");

    let mut subscriber = WebRTCClient::new();
    subscriber.set_ice_servers(vec![]);
    subscriber
        .connect_to_signaling_server(addr_str.as_str())
        .await
        .expect("Failed to connect");

    subscriber
        .join_session(session_id.as_str())
        .await
        .expect("Failed to join session");

    publisher
        .wait_for_subscriber(10)
        .await
        .expect("Failed to wait for subscriber");

    let pub_answer = publisher.get_answer().unwrap().sdp.clone();
    let sub_answer = subscriber.get_answer().unwrap().sdp.clone();

    assert_eq!(pub_answer, sub_answer);

    teardown(&mut publisher, &mut subscriber).await;
    server_handle.abort();
}
