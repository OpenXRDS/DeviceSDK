mod client;

mod xrds_websocket;
mod xrds_webrtc {
	pub mod webrtc_client;
    pub mod webcam_reader;
    pub mod media {
        pub mod transcoding {
            pub mod img2vid_encoder;
            pub mod jpeg2h264;
            pub mod pcm2opus;
        }
        pub mod streaming_mp4_writer;
        pub mod audio_capturer;
    }
}
pub use client::*;
pub use xrds_websocket::*;
pub use xrds_webrtc::*;

#[cfg(test)]
mod tests;