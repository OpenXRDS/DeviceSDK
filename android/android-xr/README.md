# Android XR (Google) — Placeholder

**Status: Not yet testable. This directory is reserved for Google's Android XR platform.**

Android XR is Google's XR operating system targeting headsets and glasses running Android
(e.g., Samsung Galaxy XR). It uses OpenXR as the session API, so the core `xrds-openxr`
integration is already compatible at the API level.

## What's different from Meta Quest

| Aspect | Meta Quest | Android XR |
|--------|------------|------------|
| OpenXR loader | Meta's `libopenxr_loader.so` | Google's loader (distributed via device system image) |
| System image | OculusOS | Android XR OS |
| Manifest category | `com.oculus.intent.category.VR/XR` | `android.intent.category.XR` (TBD, may change) |
| Min API level | 29 (Quest 2) | 35+ (Android XR devices) |
| Hand tracking ext | `XR_EXT_hand_tracking` | `XR_EXT_hand_tracking` (same) |
| Passthrough ext | `XR_FB_passthrough` | `XR_ANDROID_passthrough` (different!) |

## What needs to be done when hardware is available

1. **AndroidManifest.xml** — Create `android/android-xr/AndroidManifest.xml` based on
   Google's Android XR developer documentation. Key differences from Quest:
   - Replace `com.oculus.intent.category.VR/XR` with the correct Android XR category
   - Add `<uses-feature android:name="android.hardware.xr" android:required="true" />`
   - Update `package` name if distributing separately from the Quest build

2. **OpenXR loader** — Android XR devices ship the OpenXR loader as a system library.
   The APK does **not** bundle `libopenxr_loader.so` (unlike Quest).
   The build step that copies Meta's loader should be skipped.

3. **Passthrough extension** — `XR_ANDROID_passthrough` is different from `XR_FB_passthrough`.
   The passthrough code in `crates/xrds-openxr/` will need a compile-time or runtime branch.

4. **Extensions audit** — Review `crates/xrds-openxr/src/openxr/init.rs` (the
   `#[cfg(target_os = "android")]` block) and replace or gate any `XR_FB_*` or
   `XR_OCULUS_*` extensions that are Meta-specific.

5. **Build pipeline** — Same `cargo ndk` toolchain as Quest (same ABI: `arm64-v8a`).
   Only the packaging step changes (no loader copy, different manifest).

## References to watch

- Android XR developer preview: https://developer.android.com/xr
- OpenXR on Android XR: https://developer.android.com/xr/reference/openxr
- Samsung Galaxy XR SDK (companion to Android XR): check Samsung Developer portal
