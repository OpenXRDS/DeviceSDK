# Meta Quest Build

Targets Quest 3 and Quest Pro (API 32+) via OpenXR. Quest 2 is not supported.

## Prerequisites

- **Rust via rustup** — install from [rustup.rs](https://rustup.rs) (do **not** use Homebrew `rust` on macOS — it lacks cross-compilation support).
  Then add the Android target:

  ```sh
  rustup target add aarch64-linux-android
  ```

- **cargo-ndk** — Cargo wrapper for the Android NDK toolchain:

  ```sh
  cargo install cargo-ndk
  ```

- **Android SDK** — install via [Android Studio](https://developer.android.com/studio) SDK Manager.
  Required components: NDK, any build-tools, any android platform.
  The build scripts auto-detect the SDK location and pick the latest installed NDK, build-tools, and platform.

- **OpenXR loader** — fetch the Khronos prebuilt (Apache 2.0) with one command:

  ```powershell
  # Windows
  .\android\quest\fetch_loader.ps1
  ```

  ```sh
  # Linux / macOS
  ./android/quest/fetch_loader.sh
  ```

  This downloads `libopenxr_loader.so` from the latest
  [KhronosGroup/OpenXR-SDK](https://github.com/KhronosGroup/OpenXR-SDK/releases)
  release and saves it to `android/quest/libs/arm64-v8a/`.
  The build scripts pick it up automatically from there.

  Alternatively, copy `libopenxr_loader.so` from the Meta OpenXR Mobile SDK
  (`OpenXR/Libs/Android/arm64-v8a/Release/`) into the same directory, or set
  `OPENXR_LOADER` to override the path entirely.

## Building the APK

Two equivalent build scripts are provided — use the one matching your host OS.

**Windows (PowerShell):**

```powershell
# Dev build — scene must be pushed to the device separately (see below)
$env:OPENXR_LOADER = 'C:\ovr_openxr_mobile_sdk\OpenXR\Libs\Android\arm64-v8a\Release\libopenxr_loader.so'
.\android\quest\build.ps1

# Bundled build — scene.json and assets\ are embedded in the APK
.\android\quest\build.ps1 -SceneDir C:\path\to\exported\scene
```

**Linux / macOS (Bash):**

```sh
# Dev build — scene must be pushed to the device separately (see below)
OPENXR_LOADER=/path/to/libopenxr_loader.so \
    ./android/quest/build.sh

# Bundled build — scene.json and assets/ are embedded in the APK
OPENXR_LOADER=/path/to/libopenxr_loader.so \
    ./android/quest/build.sh --scene-dir /path/to/exported/scene/
```

Output: `android/quest/build/xrds-app.apk`

### What the script does

| Step | Description                                                       |
|------|-------------------------------------------------------------------|
| 1    | Build `libxrds_app.so` via `cargo ndk -t arm64-v8a`               |
| 2    | Copy `libopenxr_loader.so` from Meta OpenXR Mobile SDK            |
| 3    | Stage SDK assets: fonts, shaders, env maps, sound, textures       |
| 4    | If `--scene-dir` given: add `scene.json` + user assets to staging |
| 5    | Package staged files into APK via `aapt`                          |
| 6    | Zipalign + sign → `xrds-app.apk`                                  |

### Customising the build

| Variable          | Default (Linux/macOS)              | Default (Windows)                       | Description                   |
|-------------------|------------------------------------|-----------------------------------------|-------------------------------|
| `OPENXR_LOADER`   | *(auto if fetch script was run)*   | *(auto if fetch script was run)*        | Path to `libopenxr_loader.so` |
| `ANDROID_HOME`    | `~/Library/Android/sdk` (macOS)    | `%LOCALAPPDATA%\Android\Sdk`            | Android SDK root              |
|                   | `~/Android/Sdk` (Linux)            |                                         |                               |
| `BUILD_TOOLS_VER` | *(auto: latest installed)*         | *(auto: latest installed)*              | `build-tools` version to use  |
| `KEYSTORE`        | `~/.android/debug.keystore`        | `%USERPROFILE%\.android\debug.keystore` | Signing keystore              |
| `KEYSTORE_PASS`   | `android`                          | `android`                               | Keystore password             |

## Install and run

```sh
# Enable developer mode on the headset first
adb install -r android/quest/build/xrds-app.apk
adb shell am start -n org.openxrds.devicesdk/android.app.NativeActivity
```

## Two runtime modes

The app selects its asset mode automatically at startup:

| Condition                     | Mode        | What happens                                   |
|-------------------------------|-------------|------------------------------------------------|
| External `scene.json` present | Dev         | Reads scene + assets from external storage     |
| No external `scene.json`      | APK-bundled | Reads scene from APK; Bevy uses AAssetManager  |

### Dev mode (push scene separately)

Use this during development to iterate on scenes without rebuilding the APK.

```sh
adb push scene.json /sdcard/Android/data/org.openxrds.devicesdk/files/
adb push assets/    /sdcard/Android/data/org.openxrds.devicesdk/files/assets/
```

Then launch the app — it detects the external scene automatically.

### APK-bundled mode

Build with `--scene-dir`. The APK contains everything; no `adb push` needed after install.
This is the intended distribution mode.

## Useful ADB Commands

```sh
adb logcat -s xrds               # filter SDK logs
adb logcat | grep -i openxr      # OpenXR loader messages
adb shell dumpsys activity | grep openxrds   # check if app is running
```

## Troubleshooting

### Build errors

**`Failed to load signer` / `debug.keystore not found`**

`apksigner` needs a keystore. Generate the standard Android debug one:

```sh
# macOS / Linux
keytool -genkeypair -keystore ~/.android/debug.keystore \
  -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
  -keypass android -storepass android \
  -dname "CN=Android Debug,O=Android,C=US"
```

```powershell
# Windows
keytool -genkeypair -keystore "$HOME\.android\debug.keystore" `
  -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 `
  -keypass android -storepass android `
  -dname "CN=Android Debug,O=Android,C=US"
```

**`apksigner: command not found` or Java errors**

`apksigner` requires Java. Install the JDK:

```sh
# macOS
brew install --cask temurin

# Ubuntu
sudo apt install default-jdk
```

```powershell
# Windows
winget install EclipseAdoptium.Temurin.21.JDK
```

### Install errors

**`INSTALL_FAILED_UPDATE_INCOMPATIBLE: signatures do not match`**

The device has an existing install signed with a different key. Uninstall it first:

```sh
adb uninstall org.openxrds.devicesdk
adb install -r android/quest/build/xrds-app.apk
```

### Runtime symptoms

| Symptom                            | Likely cause                                              |
|------------------------------------|-----------------------------------------------------------|
| Black screen, no crash             | `libopenxr_loader.so` missing from APK                    |
| `dlopen` error                     | lib_name in manifest doesn't match Cargo `[lib] name`     |
| OpenXR `XR_ERROR_RUNTIME_FAILURE`  | Missing `com.oculus.intent.category.XR` in manifest       |
| App not visible in Quest library   | Missing XR intent category                                |
| Hand tracking unavailable          | Missing `com.oculus.permission.HAND_TRACKING`             |
| "scene.json not found" in logcat   | No external scene pushed and no `--scene-dir` at build    |
