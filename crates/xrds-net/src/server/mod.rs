// `server::server` — matches the existing module layout referenced
// throughout the crate; not worth a rename at this point.
#[allow(clippy::module_inception)]
mod server;
#[cfg(feature = "protocol-webrtc")]
mod webrtc_server;
#[cfg(feature = "protocol-ws")]
mod ws_server;

// A minimal raw-QUIC echo server for the crate's own tests (self-signed cert
// via the `rcgen` dev-dependency, so it's test-only). Gives the QUIC
// `NetChannel`/`SessionHandler` a live round-trip target — no public raw-QUIC
// echo endpoint exists.
#[cfg(all(test, feature = "protocol-quic"))]
pub(crate) mod quic_server;

pub use server::*;

// These predate the feature split and drive the whole protocol surface, including the
// per-protocol expert-only extras (`run_ftp_command`, `mqtt_subscribe`). Rather than
// scatter a cfg over every test fn, the suite declares the set it needs; narrow
// configurations still type-check the library itself, which is what they exist to prove.
#[cfg(all(test, feature = "protocol-http", feature = "protocol-coap", feature = "protocol-mqtt", feature = "protocol-ws", feature = "protocol-quic", feature = "protocol-ftp"))]
mod tests;
