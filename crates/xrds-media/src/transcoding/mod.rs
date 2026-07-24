//! Encoding for transport. Gated behind the `transcoding` feature.
//!
//! This is where codec concerns (JPEG→H264, PCM→Opus) live — deliberately
//! *not* in `xrds-net`. WebRTC transport requires a negotiated codec's
//! bitstream, but producing that bitstream from raw pixels/samples is a media
//! concern, not a networking one: encoder selection, hardware acceleration,
//! and platform codec-library interop (e.g. ffmpeg's Media Foundation
//! backend on Windows) are entirely orthogonal to sockets/ICE/RTP. See
//! `docs/xrds-net-capture-decoupling.md`.
//!
//! [`encode_jpeg_stream_to_h264`] and [`encode_pcm_stream_to_opus`] are the
//! ergonomic entry points: they take this crate's own capture outputs
//! (`video::Webcam`'s frame receiver, `audio::Microphone`'s PCM receiver) and
//! produce exactly what xrds-net's `VideoSource::EncodedH264` /
//! `AudioSource` expect — so a caller wiring capture → transcode → xrds-net
//! never has to touch [`jpeg2h264`] or [`pcm2opus`] directly.

pub mod img2vid_encoder;
pub mod jpeg2h264;
pub mod pcm2opus;
pub mod streaming_mp4_writer;

mod stream;

pub use stream::{encode_jpeg_stream_to_h264, encode_pcm_stream_to_opus, H264StreamEncoder, OpusStreamEncoder};
