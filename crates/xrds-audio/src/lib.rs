//! Device-level spatial audio.
//!
//! This is the **expert layer** for audio: a direct rodio `SpatialSink` bound to a chosen
//! `cpal` output device, with explicit emitter and per-ear listener positions.
//!
//! It is deliberately *not* the default path. `XrdsSceneAudioClip` drives Bevy's own spatial
//! audio (`PlaybackSettings { spatial, .. }`, every XRDS camera acting as a listener — see
//! `xrds-runtime`'s `spawn.rs`), which is what an authored scene uses and what the scene
//! document serializes. Use this crate when you need what Bevy does not offer: choosing the
//! output device, or driving ear positions yourself.
//!
//! Adopted from the `init-spatial-audio` branch (2025-04), which implemented it as a
//! standalone crate before the Bevy-based path existed.

pub mod audio;

pub use audio::*;

#[cfg(test)]
mod tests;
