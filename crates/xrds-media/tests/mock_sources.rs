//! Tier 2 — mock-source tests (CI-safe, no hardware).
//!
//! Exercises the crate's output contract through the `test-util` synthetic
//! sources. The whole file compiles to nothing unless that feature is on.
#![cfg(feature = "test-util")]

use xrds_media::audio::AudioFormat;
use xrds_media::mock::{canned_jpeg, mock_mic, mock_webcam};
use xrds_media::video::find_complete_jpeg;

#[test]
fn mock_webcam_emits_byte_exact_frames() {
    let f1 = canned_jpeg(&[1, 2, 3]);
    let f2 = canned_jpeg(&[4, 5]);
    let rx = mock_webcam(vec![f1.clone(), f2.clone()]);

    let received: Vec<Vec<u8>> = rx.iter().collect();
    assert_eq!(received, vec![f1.clone(), f2]);

    // Each frame is independently a valid, complete JPEG.
    assert_eq!(find_complete_jpeg(&received[0]), Some((0, f1.len())));
}

#[test]
fn mock_mic_carries_format_and_sample_count() {
    let format = AudioFormat::new(48_000, 2);
    let chunks = vec![vec![0i16; 480], vec![1i16; 480]];
    let (rx, got_format) = mock_mic(chunks, format);

    assert_eq!(got_format, format);

    let total: usize = rx.iter().map(|c| c.len()).sum();
    assert_eq!(total, 960);
}
