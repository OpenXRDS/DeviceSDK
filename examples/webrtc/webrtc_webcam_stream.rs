// External usage: live webcam + microphone streamed over WebRTC.
//
// Demonstrates the full media pipeline split from
// docs/done/xrds-net-capture-decoupling.md:
//   - `xrds-media` (PC-only) owns device access (nokhwa webcam, cpal
//     microphone) AND encoding for transport (its `transcoding` feature,
//     JPEG->H264 / PCM->Opus) — both are media concerns, not networking ones.
//   - `xrds-net` never touches hardware and never encodes anything. It only
//     accepts already-encoded media (`VideoSource`/`AudioSource`) and handles
//     transport: RTP packetization and track writes.
//   - This example is the glue: it owns both dependencies and wires
//     capture -> transcode -> transport, which neither crate does for itself.
//
// Run with: cargo run --example webrtc_webcam_stream
//
// Spins up an in-process signaling server plus a publisher (the real webcam
// and mic, transcoded) and a subscriber (which saves the received H264/Opus
// into test_output/ so you can confirm the stream actually arrived).

use std::time::Duration;

use rustls::crypto::{ring, CryptoProvider};
use xrds_media::audio::Microphone;
use xrds_media::transcoding::{encode_jpeg_stream_to_h264, encode_pcm_stream_to_opus};
use xrds_media::video::Webcam;
use xrds_net::server::XRNetServer;
use xrds_net::{AudioSource, VideoSource, WebRTCClient, PROTOCOLS};

const PORT: u32 = 18080;
const STREAM_SECONDS: u64 = 15;
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: u32 = 30;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    let _ = env_logger::try_init();

    CryptoProvider::install_default(ring::default_provider())
        .expect("failed to install default crypto provider");

    // XRNetServer::start() requires root_dir to already exist (used for FTP/file
    // serving on other protocols); create it up front so the server doesn't
    // silently panic inside its spawned task.
    std::fs::create_dir_all("test_output").expect("failed to create test_output/");

    run_signaling_server();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let addr = format!("ws://127.0.0.1:{PORT}/");

    // --- Publisher: will stream the real webcam + mic ---
    let mut publisher = WebRTCClient::new();
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

    println!("Signaling + ICE complete. Opening webcam (0) and default microphone...");

    // Device access lives entirely in xrds-media. xrds-net never sees a
    // device — only the encoded byte/frame streams below.
    let (webcam, frame_rx) = Webcam::open(0).expect("failed to open webcam 0");
    let (mic, pcm_rx, mic_format) =
        Microphone::open_default().expect("failed to open default microphone");

    // Encoding also lives entirely in xrds-media (the `transcoding` feature).
    // xrds-net accepts only the resulting already-encoded streams.
    let (h264_encoder, h264_reader) = encode_jpeg_stream_to_h264(frame_rx, WIDTH, HEIGHT, FPS)
        .expect("failed to start H264 encoder");
    let (opus_encoder, opus_rx) =
        encode_pcm_stream_to_opus(pcm_rx, mic_format).expect("failed to start Opus encoder");

    let video = VideoSource::new(Box::new(h264_reader));
    let audio = AudioSource::new(opus_rx);

    publisher
        .start_stream(video, Some(audio))
        .await
        .expect("failed to start stream");

    println!("Streaming webcam + mic for {STREAM_SECONDS}s...");
    tokio::time::sleep(Duration::from_secs(STREAM_SECONDS)).await;

    publisher
        .stop_stream()
        .await
        .expect("failed to stop stream");

    // Dropping these stops the encoder threads, then device capture.
    drop(h264_encoder);
    drop(opus_encoder);
    drop(webcam);
    drop(mic);

    let video_path = subscriber.get_debug_video_file_path().cloned();
    let audio_path = subscriber.get_debug_audio_file_path().cloned();
    println!("Done. Received stream saved to {video_path:?} / {audio_path:?}.");
    if let Some(path) = &video_path {
        println!("To visually verify the received video: ffplay \"{path}\"");
    }
    if let Some(path) = &audio_path {
        println!("To verify the received audio:          ffplay \"{path}\"");
    }
}

fn run_signaling_server() {
    let server = XRNetServer::new(vec![PROTOCOLS::WEBRTC], vec![PORT]);
    tokio::spawn(async move {
        server.set_root_dir("test_output").start().await;
    });
}
