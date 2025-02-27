mod client;

mod xrds_websocket;
mod xrds_webrtc;
pub use client::*;
pub use xrds_webrtc::*;

#[cfg(test)]
mod tests;