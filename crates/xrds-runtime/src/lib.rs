//! The runtime projection layer: XRDS concepts realized in a live Bevy world.
//!
//! Application code normally reaches this through the [`xrds`](https://docs.rs/xrds)
//! crate rather than depending on it directly. The two types worth knowing are
//! [`XrdsAPI`](xrds_api::XrdsAPI), available in `setup`, and
//! [`XrdsUpdateContext`](xrds_api::XrdsUpdateContext), available every frame — they
//! share a vocabulary deliberately, so moving a call between them is not a rewrite.
//!
//! # What "projection" means here
//!
//! An XRDS descriptor is not a Bevy component. `spawn` takes a descriptor, decides
//! what entity and components realize it, and hands back a typed
//! [`Handle`](xrds_api::Handle) that identifies the result without exposing the
//! entity. That indirection is what lets the SDK keep its own vocabulary while Bevy
//! changes underneath, and it is why an `XrdsCube` handle cannot be passed where a
//! light is expected.
//!
//! Scene documents from `xrds-scene-graph` are projected the same way, by
//! `import_scene_document` — with one difference worth knowing: some facts live in
//! the document's *hierarchy* rather than in a node's payload, so a per-node
//! projection cannot see them. Those are recovered by a second pass; see the
//! `reimport` module.

mod error;
mod runtime;
pub mod viewer;
pub mod xrds_api;

pub use error::*;
pub use runtime::*;
pub use xrds_api::*;
// XR input state — re-exported so desktop hosts (e.g. the editor's play mode)
// can drive it synthetically without depending on xrds-openxr directly.
pub use xrds_openxr::XrInput;

// Networking, re-exported as `net` so app code reaches
// `XrdsNet`/`XrdsNetTask`/`NetFeed`/… through the DeviceSDK facade
// (`xrds::net::…`) with no direct xrds-net dependency. Cross-compiles for
// Android (see docs/done/xrds-net-android-shipping.md); the FTP *server* and
// WebRTC remain desktop-only (feature-gated / never wired in, respectively —
// see crates/xrds-net/Cargo.toml and crates/xrds-runtime/Cargo.toml).
// See docs/done/xrds-net-devicesdk-integration.md.
pub use xrds_net as net;

pub mod sdk {
    pub use xrds_components::*;
}
