mod client;

mod xrds_websocket;
mod xrds_webrtc {
	pub mod webrtc_client;
    pub mod webcam_reader;
}
pub use client::*;
pub use xrds_websocket::*;
pub use xrds_webrtc::*;

#[cfg(test)]
mod tests;