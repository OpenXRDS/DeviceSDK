//! Microphone capture and the pure PCM helpers it is built from.
//!
//! [`AudioFormat`] is available everywhere; the capture device and the `cpal`
//! sample conversions are desktop-only, because `cpal` is (see Cargo.toml).

mod format;
pub use format::AudioFormat;

#[cfg(not(target_os = "android"))]
mod convert;
#[cfg(not(target_os = "android"))]
mod mic;

#[cfg(not(target_os = "android"))]
pub use convert::{f32_to_i16, u16_to_i16};
#[cfg(not(target_os = "android"))]
pub use mic::Microphone;
