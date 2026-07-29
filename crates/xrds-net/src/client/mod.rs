mod client;

// Phase 0 scaffolding for the protocol-agnostic API (see
// docs/xrds-net-protocol-handler.md) — new types only, not wired into
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

#[cfg(test)]
mod tests;
