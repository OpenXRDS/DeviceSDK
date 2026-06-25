# Android Build Targets

> **Scope:** These build targets apply to **apps exported with the SDK** (i.e., projects
> built on top of `xrds-runtime`). The **editor and SDK tools themselves** target
> Windows and Linux only (macOS planned later) — they are never built for Android.

This directory contains per-platform Android build configuration for OpenXRDS DeviceSDK.

| Directory     | Platform           | Status                                 |
|---------------|--------------------|----------------------------------------|
| `quest/`      | Meta Quest 2/3/Pro | Active — use this for development      |
| `android-xr/` | Google Android XR  | Placeholder — no testable hardware yet |

## Common Prerequisites

All Android targets share the same Rust cross-compilation toolchain:

```sh
# Install Android target
rustup target add aarch64-linux-android

# Install cargo-ndk (handles NDK linker setup automatically)
cargo install cargo-ndk
```

You also need the **Android NDK** (r27 or later) installed and `ANDROID_NDK_HOME` set:

```sh
# Example — adjust path for your OS
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/27.2.12479018
```

## Architecture

The Rust library is built as a `cdylib` and loaded by Android's GameActivity.
OpenXR extensions activated per-platform are in `crates/xrds-openxr/src/openxr/init.rs`
under `#[cfg(target_os = "android")]`.

See the platform-specific README for manifest requirements and APK packaging steps.
