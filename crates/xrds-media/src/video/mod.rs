//! Webcam capture and the pure frame helpers it is built from.

mod frame_reader;
mod jpeg;
mod webcam;

#[cfg(feature = "playback")]
mod decode;
#[cfg(feature = "playback")]
pub use decode::{VideoDecoder, VideoFrame};

pub use frame_reader::FrameReader;
pub use jpeg::{find_complete_jpeg, EOI, SOI};
pub use webcam::{list_available_devices, Webcam};
