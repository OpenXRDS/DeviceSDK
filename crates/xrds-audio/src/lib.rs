//! # DEPRECATED — not built, slated for deletion
//!
//! This crate is **excluded from the workspace** (`exclude` in the root `Cargo.toml`) and is
//! not compiled by `cargo build`, `cargo test`, or CI. Nothing depends on it. Expect it to rot
//! and then be removed.
//!
//! **Why:** it duplicated Bevy's audio almost entirely. `SpatialListener` already carries
//! per-ear offsets, emitter position comes from `GlobalTransform`, and `AudioSink` plus
//! `PlaybackSettings` cover playback — so the parallel rodio stack this crate shipped with was
//! deleted. What remained was output-device enumeration, the one thing `bevy_audio` cannot do
//! (`AudioOutput` calls `OutputStream::try_default()` and is `pub(crate)`).
//!
//! That remainder was not enough to justify a crate:
//!
//! - **Listing devices without being able to route to one is half a feature.** Honouring a
//!   choice requires patching `bevy_audio`. Building a device picker before that patch exists
//!   would produce a setting that silently does nothing — the same authorable-but-inert trap
//!   that made zone triggers look broken for two device sessions.
//! - **It is desktop-only value.** It cross-compiles for Android, but a headset has a single
//!   audio path, so there is nothing to choose.
//! - **Its one live use was a diagnostic print.** Useful, but not worth a crate in the
//!   dependency graph.
//!
//! **If you need this again:** revive it deliberately, together with the `bevy_audio` patch
//! that makes device selection actually work, and put the consumer in `apps/xrds-editor` where
//! a device preference belongs — not in a scene document, since a device name describes a
//! machine and would make documents unportable.
//!
//! The manifest keeps explicit dependency versions so `cargo check --manifest-path
//! crates/xrds-audio/Cargo.toml` still works from outside the workspace.
//!
//! ---
//!
//! Audio output-device enumeration.
//!
//! **This is not the audio path for a scene.** Authored audio goes through
//! `XrdsSceneAudioClip`, which drives Bevy spatial audio — every XRDS camera is a listener,
//! and the scene document carries `spatial`, `distance_model`, `min_distance`,
//! `max_distance`, `rolloff_factor` and `hrtf`. Use that.
//!
//! This crate answers one question Bevy cannot: **which output devices exist?**
//! `bevy_audio` opens `OutputStream::try_default()` and keeps `AudioOutput` `pub(crate)`,
//! so a device picker has nowhere to get its list from.
//!
//! Note it only *lists* devices. Routing Bevy audio to a chosen one needs a `bevy_audio`
//! patch and is not attempted here — see [`audio`] for why.
//!
//! ```ignore
//! // Deprecated: shown for reference only; this crate is not built.
//! use xrds_audio::XrdsAudioDevice;
//!
//! for device in XrdsAudioDevice::list() {
//!     println!("{}", device.name);
//! }
//! ```
//!
//! Adopted from the `init-spatial-audio` branch (2025-04) and then trimmed: the branch also
//! carried a full parallel rodio stack, all of which Bevy already does. [`audio`] documents
//! exactly what was removed and what replaces it.

pub mod audio;

pub use audio::{XrdsAudioDevice, XrdsAudioError};

/// Re-exported so a caller can name the same `cpal` types this crate hands out.
///
/// Load-bearing: [`XrdsAudioDevice::cpal_device`] returns a `cpal::Device`, and passing it to
/// rodio or `bevy_audio` only compiles if both sides agree on the `cpal` version. Depending
/// on `cpal` separately is how that goes wrong.
pub use cpal;

#[cfg(test)]
mod tests;
