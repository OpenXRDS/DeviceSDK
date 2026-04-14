mod client;

mod xrds_websocket;
mod xrds_webrtc {
    pub mod webcam_reader;
    pub mod webrtc_client;
    pub mod media {
        pub mod transcoding {
            pub mod img2vid_encoder;
            pub mod jpeg2h264;
            pub mod pcm2opus;
        }
        pub mod audio_capturer;
        pub mod handlers;
        pub mod streaming_mp4_writer;
        pub use handlers::{AudioTrackHandler, MediaTrackHandler, VideoTrackHandler};
    }
}
pub use client::*;
pub use xrds_webrtc::*;
pub use xrds_websocket::*;

#[cfg(test)]
mod tests;
