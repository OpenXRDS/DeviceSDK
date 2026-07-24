//! Microphone capture and the pure PCM helpers it is built from.

mod convert;
mod format;
mod mic;

pub use convert::{f32_to_i16, u16_to_i16};
pub use format::AudioFormat;
pub use mic::Microphone;
