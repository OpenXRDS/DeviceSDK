//! Output-device enumeration.
//!
//! # Why this crate is only device enumeration
//!
//! It arrived from the `init-spatial-audio` branch as a full parallel audio stack: a rodio
//! `SpatialSink` with its own `OutputStream`, emitter and per-ear listener positions, and
//! play/pause/stop/volume/speed controls. All of that was **removed**, because Bevy 0.17
//! already provides every part of it and provides it better:
//!
//! | what the removed code did | what Bevy already does |
//! |---|---|
//! | rodio `SpatialSink` | the same primitive, used internally by `bevy_audio` |
//! | `set_left_ear_position` / `set_right_ear_position` | `SpatialListener { left_ear_offset, right_ear_offset }` (`bevy_audio/src/audio.rs:173`) |
//! | `set_emitter_position` pushed by hand each frame | derived from the entity's `GlobalTransform` |
//! | `play` / `pause` / `stop` / `set_volume` / `set_speed` | `AudioSink` + `PlaybackSettings` |
//! | own `OutputStream`, outside the ECS | `AudioPlayer` on an entity, with asset loading |
//!
//! Keeping it would have meant two audio stacks competing for one output device, with the
//! naming giving no hint which to use, and only one of them connected to the scene document
//! (`XrdsSceneAudioClip` -> `PlaybackSettings { spatial, .. }`, see `xrds-runtime`'s
//! `spawn.rs:117` and `:949`). That is a debugging trap, not a feature.
//!
//! # What is left, and why it cannot be deleted too
//!
//! Choosing the **output device** is the one audio capability Bevy does not offer.
//! `bevy_audio`'s `AudioOutput` calls `OutputStream::try_default()`
//! (`audio_output.rs:33`), is `pub(crate)`, and is installed with
//! `init_resource::<AudioOutput>()` — so there is no way to hand it a device from outside.
//!
//! This module therefore exists to answer "which devices are there?", which is the half a
//! device picker needs that Bevy cannot answer. Actually *routing* audio to a chosen device
//! would require patching `bevy_audio` (as this repo already patches `bevy_winit`), and is
//! deliberately not attempted here — an XR headset has a single audio path, so it only
//! matters on desktop, and nobody has needed it yet.

use cpal::traits::{DeviceTrait, HostTrait};

/// Why device enumeration failed.
///
/// A typed enum rather than `anyhow`, matching `XrdsNameError` and the other `Xrds*Error`
/// types: `anyhow::Result` appears nowhere else in the SDK's public surface, and a caller
/// building a device picker needs to distinguish "this platform has no audio host" from "one
/// device refused to give its name" — the first is fatal to the picker, the second is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsAudioError {
    /// A `cpal` host could not be opened. `host` is the host's name.
    HostUnavailable { host: String, detail: String },
    /// A host refused to list its output devices.
    DeviceEnumerationFailed { host: String, detail: String },
    /// A device exists but would not report its name, so it cannot be presented or matched.
    DeviceNameUnavailable { detail: String },
}

impl XrdsAudioError {
    /// One sentence a UI can show verbatim.
    pub fn message(&self) -> String {
        match self {
            Self::HostUnavailable { host, detail } => format!(
                "The audio host {host:?} could not be opened ({detail}). Other hosts may still \
                 work, so a device list can be partial rather than empty."
            ),
            Self::DeviceEnumerationFailed { host, detail } => format!(
                "The audio host {host:?} would not list its output devices ({detail})."
            ),
            Self::DeviceNameUnavailable { detail } => format!(
                "An audio device did not report a name ({detail}), so it cannot be shown to a \
                 user or matched against a saved preference."
            ),
        }
    }
}

impl std::fmt::Display for XrdsAudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

/// An audio output device: the name to show a user, and the handle to open it.
pub struct XrdsAudioDevice {
    /// Human-readable device name, as reported by the platform.
    ///
    /// A public field, like the other `Xrds*` descriptor types, rather than a getter.
    pub name: String,
    device: cpal::Device,
}

impl XrdsAudioDevice {
    /// The underlying `cpal` device.
    ///
    /// Exposed because without it this type is just a string and the crate has no purpose:
    /// a caller needs the handle to open a stream, and a future `bevy_audio` patch would
    /// need it to route Bevy's output.
    ///
    /// **The `cpal` version matters.** Hand this to rodio (or to `bevy_audio`, which wraps
    /// rodio) and the types must come from the *same* `cpal`. This crate re-exports its own
    /// `cpal` for that reason — see the crate root.
    pub fn cpal_device(&self) -> &cpal::Device {
        &self.device
    }

    /// Every output device across every available host.
    ///
    /// Returns `Ok` with a possibly-empty list on a machine with no sound card, rather than an
    /// error. "No audio hardware" is a normal condition — a headless CI runner is in it — and
    /// treating it as a failure would push an error path into every device picker.
    ///
    /// A host that cannot be opened or enumerated is **skipped with a warning**, not
    /// propagated: one broken host (a disconnected ASIO driver, say) should not hide the
    /// devices on every working one. Use [`Self::list_strict`] when a caller would rather see
    /// the failure.
    pub fn list() -> Vec<XrdsAudioDevice> {
        let (devices, failures) = Self::enumerate();
        for failure in failures {
            log::warn!("{}", failure.message());
        }
        devices
    }

    /// Like [`Self::list`], but reports what went wrong alongside what was found.
    ///
    /// For a settings UI that should say "3 devices found, 1 host unavailable" instead of
    /// quietly showing a short list.
    pub fn list_strict() -> (Vec<XrdsAudioDevice>, Vec<XrdsAudioError>) {
        Self::enumerate()
    }

    fn enumerate() -> (Vec<XrdsAudioDevice>, Vec<XrdsAudioError>) {
        let mut devices: Vec<XrdsAudioDevice> = Vec::new();
        let mut failures: Vec<XrdsAudioError> = Vec::new();

        for host_id in cpal::available_hosts() {
            let host_name = host_id.name().to_string();

            let host = match cpal::host_from_id(host_id) {
                Ok(host) => host,
                Err(e) => {
                    failures.push(XrdsAudioError::HostUnavailable {
                        host: host_name,
                        detail: e.to_string(),
                    });
                    continue;
                }
            };

            let outputs = match host.output_devices() {
                Ok(outputs) => outputs,
                Err(e) => {
                    failures.push(XrdsAudioError::DeviceEnumerationFailed {
                        host: host_name,
                        detail: e.to_string(),
                    });
                    continue;
                }
            };

            for device in outputs {
                match device.name() {
                    Ok(name) => devices.push(XrdsAudioDevice { name, device }),
                    Err(e) => failures.push(XrdsAudioError::DeviceNameUnavailable {
                        detail: e.to_string(),
                    }),
                }
            }
        }

        (devices, failures)
    }

    /// Diagnostic dump of every host and device, to stdout.
    ///
    /// Writes with `println!` rather than `log`, deliberately: it exists to be called from a
    /// CLI or an example when someone is working out why a device is missing, and that output
    /// should not depend on a logger being installed. Nothing in a render loop should call it.
    pub fn print_available() {
        println!("Supported hosts: {:?}", cpal::ALL_HOSTS);
        let available_hosts = cpal::available_hosts();
        println!("Available hosts: {:?}\n", available_hosts);

        for (host_ordinal, host_id) in available_hosts.into_iter().enumerate() {
            let host_index = host_ordinal + 1;
            println!("{}. {}", host_index, host_id.name());

            let host = match cpal::host_from_id(host_id) {
                Ok(host) => host,
                Err(e) => {
                    println!("  <host unavailable: {e}>");
                    continue;
                }
            };

            let default_device = host
                .default_output_device()
                .and_then(|d| d.name().ok())
                .unwrap_or_else(|| "<none>".to_string());
            println!("  Default output device: {default_device}");

            let devices = match host.devices() {
                Ok(devices) => devices,
                Err(e) => {
                    println!("  <devices unavailable: {e}>");
                    continue;
                }
            };

            for (device_index, device) in devices.enumerate() {
                let name = device
                    .name()
                    .unwrap_or_else(|_| "<unnamed device>".to_string());
                println!("\n  {}.{}. {}", host_index, device_index + 1, name);

                if let Ok(conf) = device.default_input_config() {
                    println!("    Default input stream config: {conf:?}");
                }
                if let Ok(configs) = device.supported_input_configs() {
                    for config in configs {
                        println!("    {config:?}");
                    }
                }

                if let Ok(conf) = device.default_output_config() {
                    println!("    Default output stream config: {conf:?}");
                }
                if let Ok(configs) = device.supported_output_configs() {
                    for config in configs {
                        println!("    {config:?}");
                    }
                }
            }
        }
        println!();
    }
}
