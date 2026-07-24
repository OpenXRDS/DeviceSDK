//! Graceful-absence tests (CI-safe).
//!
//! These call the enumeration path and assert it degrades gracefully — returning
//! a `Result` without panicking — regardless of whether a camera is present.

use xrds_media::video::list_available_devices;

#[test]
fn list_available_devices_never_panics() {
    // With no camera (typical CI) this is `Err`; with one it is `Ok(non-empty)`.
    // Either is acceptable — the contract is "no panic, well-formed result".
    match list_available_devices() {
        Ok(devices) => assert!(!devices.is_empty(), "Ok must carry at least one device"),
        Err(msg) => assert!(!msg.is_empty(), "Err must carry a message"),
    }
}
