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

//! Desktop-only media I/O for XRDS.
//!
//! This crate's mandate is broader than any single feature living in it today:
//! **capture** (this module tree, always available), **encoding for transport**
//! (the `transcoding` feature), and — planned, not yet implemented — **media
//! import and playback** for both the SDK (`xrds-runtime`) and the GUI editor.
//! Keeping all of these under one "media I/O" crate (rather than one crate per
//! concern) avoids reshuffling dependents again as each capability lands.
//!
//! It owns the platform device dependencies (`nokhwa`, `cpal`) that used to live
//! inside `xrds-net`, and, behind the `transcoding` feature, the codec
//! dependencies (`ffmpeg-next`, `opus`) that also used to live there. It
//! produces **raw or encoded media frames** and nothing else — no networking:
//!
//! - Video: [`video::Webcam`] sends one complete JPEG frame per message on a
//!   `Receiver<Vec<u8>>`. Wrap it in [`video::FrameReader`] if a specific
//!   consumer needs a `std::io::Read` instead.
//! - Audio: [`audio::Microphone`] streams PCM `i16` chunks over a channel plus an
//!   [`audio::AudioFormat`] descriptor.
//! - Transcoding (`transcoding` feature): [`transcoding::encode_jpeg_stream_to_h264`]
//!   and [`transcoding::encode_pcm_stream_to_opus`] turn the two outputs above
//!   into exactly what `xrds-net`'s `VideoSource::EncodedH264` / `AudioSource`
//!   expect — because encoding for a specific wire codec is a media concern,
//!   not a networking one (see `docs/xrds-net-capture-decoupling.md`).
//!
//! By design this crate does **not** depend on `xrds-net`; the consumer wires the
//! plain outputs into whatever transport it uses.
//!
//! The frame/format logic is deliberately split into free functions and small
//! types ([`video::find_complete_jpeg`], [`audio::f32_to_i16`], …) so it can be
//! unit-tested without any hardware.

pub mod audio;
pub mod video;

#[cfg(feature = "transcoding")]
pub mod transcoding;

#[cfg(feature = "test-util")]
pub mod mock;
