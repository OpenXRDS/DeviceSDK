mod client;

mod xrds_websocket;
mod xrds_webrtc {
    pub mod webrtc_client;
    pub mod media {
        pub mod audio_pipeline;
        pub mod handlers;
        pub mod source;
        pub use handlers::{AudioTrackHandler, MediaTrackHandler, VideoTrackHandler};
        pub use source::{AudioSource, VideoSource};
    }
}
pub use client::*;
pub use xrds_webrtc::*;
pub use xrds_websocket::*;

#[cfg(test)]
mod tests;
