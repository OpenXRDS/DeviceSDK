/*
Copyright 2025 KETI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! Injected media sources for the WebRTC client.
//!
//! xrds-net does not acquire media from hardware, and it does not encode media
//! for a specific wire codec either — both are media concerns, not networking
//! ones (see `docs/xrds-net-capture-decoupling.md`). It accepts media **already
//! encoded** in the codec WebRTC negotiated (H264 video, Opus audio) and only
//! handles transport: RTP packetization and track writes.
//!
//! The caller is responsible for producing these — typically via the
//! `xrds-media` crate's `transcoding` feature, which turns its own capture
//! outputs into exactly these shapes. xrds-net does not depend on `xrds-media`
//! or any codec library.

use std::io::Read;
use std::sync::mpsc::Receiver;

/// An injected video source: a byte stream already encoded as H264 Annex-B
/// (SPS/PPS + NAL units, each prefixed with a start code). Parsed directly via
/// [`H264Reader`](webrtc::media::io::h264_reader::H264Reader) and written to
/// the video track as-is — xrds-net never transcodes.
pub struct VideoSource(pub Box<dyn Read + Send>);

impl VideoSource {
    pub fn new(reader: Box<dyn Read + Send>) -> Self {
        Self(reader)
    }
}

/// An injected audio source: Opus frames, one per message (conventionally
/// 20ms each — the standard Opus frame size and what
/// `xrds_media::transcoding::encode_pcm_stream_to_opus` produces).
///
/// The producer sends frames on `rx` until it drops the sender, which the
/// consumer treats as end-of-stream.
pub struct AudioSource {
    pub rx: Receiver<Vec<u8>>,
}

impl AudioSource {
    pub fn new(rx: Receiver<Vec<u8>>) -> Self {
        Self { rx }
    }
}
