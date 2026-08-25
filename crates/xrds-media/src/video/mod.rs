//! Webcam capture and the pure frame helpers it is built from.

mod frame_reader;
mod jpeg;

// Camera capture is desktop-only — see Cargo.toml.
#[cfg(not(target_os = "android"))]
mod webcam;

#[cfg(feature = "playback")]
mod decode;
#[cfg(feature = "playback")]
pub use decode::{probe_video_codec, VideoCodec, VideoDecoder, VideoFrame};

// Hardware decode, target-gated rather than feature-gated: on Android it is the
// only decode path worth having, and off Android it cannot compile at all.
#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::{HardwareBuffer, HardwareVideoDecoder};

pub use frame_reader::FrameReader;
pub use jpeg::{find_complete_jpeg, EOI, SOI};

#[cfg(not(target_os = "android"))]
pub use webcam::{list_available_devices, Webcam};
