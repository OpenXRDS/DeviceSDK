//! Tier 3 — real-hardware tests. All `#[ignore]`d: run manually on a dev machine
//! with a camera and microphone:
//!
//! ```sh
//! cargo test -p xrds-media -- --ignored
//! ```

use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use xrds_media::audio::Microphone;
use xrds_media::video::{list_available_devices, Webcam, EOI, SOI};

#[test]
#[ignore = "requires a real camera"]
fn enumerates_real_cameras() {
    let devices = list_available_devices().expect("expected at least one camera");
    assert!(!devices.is_empty());
    println!("cameras: {devices:?}");
}

/// Proves *sustained* streaming (not a one-shot) and per-frame integrity.
///
/// Each channel message is already exactly one complete camera frame, so this
/// validates SOI/EOI (on MJPEG streams) per message rather than across buffer
/// boundaries, and counts frames over a ~1s window.
#[test]
#[ignore = "requires a real camera"]
fn captures_a_webcam_stream() {
    const MIN_FRAMES: usize = 5; // ~1s at even a slow 10fps clears this

    let (webcam, frame_rx) = Webcam::open(0).expect("failed to open webcam 0");

    // First recv blocks through camera warm-up; start the timing window after it.
    let f0 = frame_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("no frame within 5s");
    assert!(!f0.is_empty(), "expected non-empty frame bytes");
    let mjpeg = f0[0..2] == SOI;
    println!("stream mode: {}", if mjpeg { "MJPEG" } else { "raw pixels" });

    let check_frame = |bytes: &[u8], idx: usize| {
        if mjpeg {
            assert_eq!(&bytes[0..2], &SOI, "frame {idx} must start with SOI");
            assert_eq!(&bytes[bytes.len() - 2..], &EOI, "frame {idx} must end with EOI");
        } else {
            assert!(bytes.len() > 1000, "raw frame {idx} implausibly small");
        }
    };
    check_frame(&f0, 0);

    let mut frames = 1usize;
    let (mut min, mut max) = (f0.len(), f0.len());
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        match frame_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(frame) => {
                check_frame(&frame, frames);
                frames += 1;
                min = min.min(frame.len());
                max = max.max(frame.len());
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break, // stream ended
        }
    }

    println!("captured {frames} frames in ~1s (sizes {min}..={max} bytes)");
    assert!(
        frames >= MIN_FRAMES,
        "expected sustained streaming, got only {frames} frame(s)"
    );

    drop(webcam); // stops capture + joins the thread; must not hang
}

/// Collects ~1s of PCM and asserts the signal is actually present (not digital
/// silence). Make some noise while this runs.
#[test]
#[ignore = "requires a real microphone"]
fn captures_microphone_pcm() {
    const SILENCE_THRESHOLD: i32 = 50; // i16 range is ±32767; a live mic clears this

    let (mic, rx, format) = Microphone::open_default().expect("failed to open microphone");
    assert!(format.sample_rate > 0 && format.channels > 0);
    println!("mic format: {format:?}");

    // Collect roughly one second of samples.
    let want = (format.sample_rate * format.channels as u32) as usize;
    let mut samples: Vec<i16> = Vec::new();
    let start = Instant::now();
    while samples.len() < want && start.elapsed() < Duration::from_secs(3) {
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(chunk) => samples.extend_from_slice(&chunk),
            Err(RecvTimeoutError::Timeout) => break,
            Err(RecvTimeoutError::Disconnected) => panic!("mic stream disconnected"),
        }
    }
    assert!(!samples.is_empty(), "no PCM received within timeout");

    let max_abs = samples.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0);
    println!(
        "collected {} samples over ~{:?}, peak amplitude {max_abs}",
        samples.len(),
        start.elapsed()
    );
    assert!(
        max_abs > SILENCE_THRESHOLD,
        "audio is essentially silent (peak {max_abs}); make some noise during the test"
    );

    drop(mic); // stops the stream; must not hang
}

#[test]
#[ignore = "requires a real camera"]
fn open_nonexistent_device_errors() {
    let result = Webcam::open(u32::MAX);
    assert!(result.is_err(), "opening a bogus device index should Err");
}
