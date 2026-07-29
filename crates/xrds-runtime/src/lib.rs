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
// Android (see docs/xrds-net-android-shipping.md); the FTP *server* and
// WebRTC remain desktop-only (feature-gated / never wired in, respectively —
// see crates/xrds-net/Cargo.toml and crates/xrds-runtime/Cargo.toml).
// See docs/done/xrds-net-devicesdk-integration.md.
pub use xrds_net as net;

pub mod sdk {
    pub use xrds_components::*;
}
