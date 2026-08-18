//! Tests that can run without an audio device.
//!
//! Deliberately narrow. Everything interesting in this crate needs a real output device —
//! `SpatialAudio::new` opens a stream, and CI runners have no sound card — so these cover
//! the parts that hold regardless, and the device-dependent paths are left to manual
//! verification rather than pretended to be tested.

use super::*;

/// `get_device_list` must report absence, not fail, when a machine has no output device.
///
/// This is the case a headless CI runner is in, and the distinction matters: an SDK that
/// returns `Err` here forces every caller to handle "no sound card" as an error path, when
/// an empty list is the honest answer and trivially handled.
#[test]
fn listing_devices_succeeds_even_with_no_audio_hardware() {
    let devices = SpatialAudio::get_device_list();
    assert!(
        devices.is_ok(),
        "expected Ok (possibly empty), got {:?}",
        devices.err()
    );

    // Whatever came back, every entry must carry a usable name — the name is the only handle
    // a caller has for presenting a device picker.
    for device in devices.unwrap() {
        assert!(
            !device.name.is_empty(),
            "a device with an empty name cannot be shown to a user or matched by config"
        );
    }
}

/// The device list must be re-queryable.
///
/// Guards against a future refactor that caches host enumeration in a way that only works
/// once, which would break a UI that refreshes its device list — the whole reason this crate
/// exists rather than using Bevy's audio, which offers no device selection at all.
#[test]
fn device_listing_is_repeatable() {
    let first = SpatialAudio::get_device_list().expect("first listing");
    let second = SpatialAudio::get_device_list().expect("second listing");
    assert_eq!(
        first.len(),
        second.len(),
        "device count changed between two immediate calls"
    );
}
