# xrds-media

Desktop-only camera and microphone capture for XRDS. Produces **raw** media
frames and nothing else — no networking, no codecs.

This crate owns the platform device dependencies (`nokhwa`, `cpal`) that used to
live inside `xrds-net`. See [`docs/xrds-net-capture-decoupling.md`](../../docs/xrds-net-capture-decoupling.md)
for the rationale and the target layering.

## What it produces

| Source | Output | Notes |
| --- | --- | --- |
| `video::Webcam` | `Receiver<Vec<u8>>` | One complete JPEG frame per message. Wrap in `video::FrameReader` if a consumer needs a `std::io::Read` instead. |
| `audio::Microphone` | `(Receiver<Vec<i16>>, AudioFormat)` | Interleaved PCM `i16` chunks + `{sample_rate, channels}`. |

It does **not** depend on `xrds-net`. The consumer wires these plain outputs into
whatever transport it uses.

## Platform

PC only (Windows / Linux; macOS planned). Never built for Android/Quest — device
access there uses the platform's own camera/passthrough APIs, not this crate.

## Testing

Logic and mock-source tests are hardware-free and run in CI:

```sh
cargo test -p xrds-media                      # Tier 1 (pure logic)
cargo test -p xrds-media --features test-util # + Tier 2 (mock sources)
```

Real-hardware tests are `#[ignore]`d — run them manually on a machine with a
camera and microphone:

```sh
cargo test -p xrds-media -- --ignored
```

## Features

- `test-util` — exposes synthetic sources (`mock::mock_webcam`, `mock::mock_mic`)
  that satisfy the same output contract as the real devices, for hardware-free
  tests (this crate's Tier 2, and later the xrds-net integration tests).
