// `client::client` — matches the existing module layout referenced
// throughout the crate; not worth a rename at this point.
#[allow(clippy::module_inception)]
mod client;

// Phase 0 scaffolding for the protocol-agnostic API (see
// docs/done/xrds-net-protocol-handler.md) — new types only, not wired into
// `Client`/`ClientBuilder` yet.
mod categories;
mod context;
mod error;
mod event;
mod handler;
mod net_channel;
mod net_feed;
mod net_intent;
mod net_task;
mod protocols;
mod scheme;

// Gated together because `media` (AudioSource/VideoSource) exists to feed WebRTC tracks
// and has no meaning without it.
#[cfg(feature = "protocol-webrtc")]
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
#[cfg(feature = "protocol-webrtc")]
pub use xrds_webrtc::*;

pub use categories::{FileTransferHandler, SessionHandler, StreamHandler};
pub use context::ClientContext;
pub use error::NetError;
pub use event::{Event, EventStream, ListenOptions, Overflow};
pub use handler::{create_handler, ProtocolHandler, UnsupportedHandler};
pub use net_channel::NetChannel;
pub use net_feed::NetFeed;
pub use net_intent::{RequestOptions, TransferOp, TransferResult, XrdsNet};
pub use net_task::{NetTaskSlot, XrdsNetTask};
pub use scheme::scheme_to_protocol;

// These predate the feature split and drive the whole protocol surface, including the
// per-protocol expert-only extras (`run_ftp_command`, `mqtt_subscribe`). Rather than
// scatter a cfg over every test fn, the suite declares the set it needs; narrow
// configurations still type-check the library itself, which is what they exist to prove.
#[cfg(all(test, feature = "protocol-http", feature = "protocol-coap", feature = "protocol-mqtt", feature = "protocol-ws", feature = "protocol-quic", feature = "protocol-ftp"))]
mod tests;
