// `server::server` — matches the existing module layout referenced
// throughout the crate; not worth a rename at this point.
#[allow(clippy::module_inception)]
mod server;
mod webrtc_server;
mod ws_server;

// A minimal raw-QUIC echo server for the crate's own tests (self-signed cert
// via the `rcgen` dev-dependency, so it's test-only). Gives the QUIC
// `NetChannel`/`SessionHandler` a live round-trip target — no public raw-QUIC
// echo endpoint exists.
#[cfg(test)]
pub(crate) mod quic_server;

pub use server::*;

#[cfg(test)]
mod tests;
