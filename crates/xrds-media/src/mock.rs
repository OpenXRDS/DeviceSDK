//! Synthetic, hardware-free capture sources. Enabled by the `test-util` feature.
//!
//! These satisfy the same output contract as the real device sources — a
//! `Receiver<Vec<u8>>` of complete JPEG frames for video, a
//! `(Receiver<Vec<i16>>, AudioFormat)` for audio — so tests (this crate's Tier 2
//! and, later, xrds-net integration tests) can run without a camera or mic.

use std::sync::mpsc::{channel, Receiver};

use crate::audio::AudioFormat;
use crate::video::{EOI, SOI};

/// Wrap `payload` in JPEG SOI/EOI markers to form one canned frame.
pub fn canned_jpeg(payload: &[u8]) -> Vec<u8> {
    let mut v = SOI.to_vec();
    v.extend_from_slice(payload);
    v.extend_from_slice(&EOI);
    v
}

/// A channel pre-loaded with `frames` (then closed). Mirrors
/// [`Webcam::open`](crate::video::Webcam::open)'s frame receiver without a
/// device.
pub fn mock_webcam(frames: Vec<Vec<u8>>) -> Receiver<Vec<u8>> {
    let (tx, rx) = channel();
    for f in frames {
        let _ = tx.send(f);
    }
    drop(tx);
    rx
}

/// A PCM receiver pre-loaded with `chunks` plus its declared `format` (then
/// closed). Mirrors [`Microphone::open_default`](crate::audio::Microphone::open_default).
pub fn mock_mic(chunks: Vec<Vec<i16>>, format: AudioFormat) -> (Receiver<Vec<i16>>, AudioFormat) {
    let (tx, rx) = channel();
    for c in chunks {
        let _ = tx.send(c);
    }
    drop(tx);
    (rx, format)
}
