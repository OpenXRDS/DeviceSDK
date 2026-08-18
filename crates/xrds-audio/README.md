# XRDS Audio — DEPRECATED

> **Not built, and slated for deletion.** This crate is excluded from the workspace
> (`exclude` in the root `Cargo.toml`), so `cargo build`, `cargo test` and CI all skip it, and
> nothing depends on it. Expect it to rot.
>
> **Why:** almost everything here duplicated Bevy — `SpatialListener` has per-ear offsets,
> emitter position comes from `GlobalTransform`, `AudioSink`/`PlaybackSettings` cover playback.
> After trimming that away, only output-device enumeration remained, and that is half a feature:
> routing audio to a chosen device needs a `bevy_audio` patch, a headset has a single audio path
> so it only matters on desktop, and the sole live use was a diagnostic print.
>
> **Reviving it** means doing so alongside that patch, with the consumer in `apps/xrds-editor`
> where a device preference belongs — not in a scene document, where a machine-specific device
> name would make documents unportable.

Audio **output-device enumeration** for XRDS applications.

## Which audio path do I want?

For anything in an authored scene: **not this crate.** Use `XrdsSceneAudioClip`, which drives
Bevy spatial audio — every XRDS camera acts as a listener, and the scene document carries
`spatial`, `distance_model`, `min_distance`, `max_distance`, `rolloff_factor` and `hrtf`. That
path is what the editor exposes and what round-trips through `scene.json`.

This crate answers one question Bevy cannot: **which output devices exist?** `bevy_audio`
opens `OutputStream::try_default()` and keeps its `AudioOutput` resource `pub(crate)`, so a
device picker has nowhere to get its list from.

## What this crate does *not* do

It lists devices; it does not route audio to one. Doing that requires patching `bevy_audio`
(this repo already patches `bevy_winit`, so it is not unprecedented) and has not been done
because an XR headset has a single audio path — device choice only matters on desktop.

## History

Adopted from the `init-spatial-audio` branch (April 2025), which implemented a full parallel
rodio stack: its own `OutputStream`, a `SpatialSink`, per-ear listener positions, and
play/pause/volume/speed controls.

All of that was removed on adoption, because Bevy 0.17 already provides each piece and
provides it integrated with the ECS — `SpatialListener` has `left_ear_offset`/
`right_ear_offset`, emitter position comes from `GlobalTransform`, and `AudioSink` plus
`PlaybackSettings` cover playback control. Keeping it would have left two audio stacks
competing for one output device with nothing in the naming to say which to use.

`src/audio.rs` documents the removal in a table, mapping each deleted capability to its Bevy
equivalent.

## Usage

```rust
use xrds_audio::XrdsAudioDevice;

for device in XrdsAudioDevice::list() {
    println!("{}", device.name);
}
```

`list()` returns a `Vec`, not a `Result`: a machine with no sound card is a normal condition,
not an error, and a headless CI runner is in it. A host that cannot be opened is skipped with a
warning so one broken driver does not hide every working device.

When a UI needs to say *"3 devices found, 1 host unavailable"* rather than quietly showing a
short list:

```rust
let (devices, failures) = XrdsAudioDevice::list_strict();
for failure in &failures {
    eprintln!("{}", failure.message());   // one sentence, safe to show verbatim
}
```

`XrdsAudioError` follows the same shape as the SDK's other error types (`XrdsNameError` and
friends): a plain enum, a `message()` returning one human sentence, and `Display` delegating to
it.

For a diagnostic dump of every host, device and supported config — intended for a CLI or
example, not a render loop:

```rust
XrdsAudioDevice::print_available();
```

`XrdsAudioDevice::cpal_device()` gives the underlying handle. Use this crate's re-exported
`cpal` (`xrds_audio::cpal`) rather than depending on `cpal` separately — the types only match
if the versions do.

## Requirements

Whatever `cpal` supports.
