//! Tests that can run without an audio device.
//!
//! Deliberately narrow. A real output device is needed to prove anything about playback, and
//! CI runners have no sound card, so what is covered here is the contract a headless machine
//! can prove. Device-dependent behaviour is left to manual verification rather than pretended
//! to be tested.

use super::*;

/// Listing must report absence, not failure, on a machine with no audio hardware.
///
/// This is the case a headless CI runner is in, and the distinction shapes the API: if listing
/// returned an error here, every caller would have to treat "no sound card" as an error path
/// when an empty list is the honest answer. That is why `list()` returns a `Vec`, not a
/// `Result`.
#[test]
fn listing_devices_never_fails_even_with_no_audio_hardware() {
    let devices = XrdsAudioDevice::list();

    // Whatever came back, every entry must carry a usable name — the name is the only handle a
    // caller has for presenting a picker or matching a saved preference. A nameless device is
    // dropped during enumeration precisely so this holds.
    for device in &devices {
        assert!(
            !device.name.is_empty(),
            "a device with an empty name cannot be shown to a user or matched by config"
        );
    }
}

/// The device list must be re-queryable.
///
/// Guards against a future refactor that caches host enumeration in a way that only works
/// once, which would break a settings UI that refreshes its list — the whole reason this crate
/// exists rather than using Bevy's audio, which offers no device selection at all.
#[test]
fn device_listing_is_repeatable() {
    let first = XrdsAudioDevice::list();
    let second = XrdsAudioDevice::list();
    assert_eq!(
        first.len(),
        second.len(),
        "device count changed between two immediate calls"
    );
}

/// `list` and `list_strict` must agree on the devices; they differ only in whether failures are
/// returned or logged.
///
/// Worth pinning because the two share an implementation today, and a later optimisation to one
/// path could silently make them disagree — leaving a settings UI showing a different set from
/// the rest of the app.
#[test]
fn strict_listing_returns_the_same_devices_as_the_lenient_one() {
    let lenient = XrdsAudioDevice::list();
    let (strict, failures) = XrdsAudioDevice::list_strict();

    assert_eq!(
        lenient.len(),
        strict.len(),
        "list() and list_strict() disagree on the device count"
    );

    // Every reported failure must produce a non-empty, human-readable sentence: these are shown
    // verbatim in a UI, per the `message()` convention the other Xrds errors follow.
    for failure in &failures {
        assert!(
            !failure.message().is_empty(),
            "an error with no message cannot be surfaced to a user"
        );
        assert_eq!(
            failure.message(),
            failure.to_string(),
            "Display must delegate to message(), as XrdsNameError does"
        );
    }
}
