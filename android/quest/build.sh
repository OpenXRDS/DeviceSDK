#!/usr/bin/env bash
# android/quest/build.sh — Full Quest APK build and packaging script
#
# Usage:
#   ./android/quest/build.sh [--scene-dir /path/to/dir]
#
#   --scene-dir  Optional. Copy scene.json and assets/ from this directory into
#                the APK so no adb push is needed at runtime. Without it, you
#                must push the scene separately — see "Install and run" below.
#
# Required environment variables:
#   OPENXR_LOADER   Path to libopenxr_loader.so from the Meta OpenXR Mobile SDK.
#                   Example: ~/OpenXR/Libs/Android/arm64-v8a/Release/libopenxr_loader.so
#
# Optional environment variables:
#   ANDROID_HOME    Android SDK root (default: ~/Android/Sdk)
#   BUILD_TOOLS_VER Android build-tools version (default: 35.0.0)
#   KEYSTORE        Keystore for signing (default: ~/.android/debug.keystore)
#   KEYSTORE_PASS   Keystore password (default: android)
#
# Output: android/quest/build/xrds-app.apk

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PACKAGE_NAME="org.openxrds.devicesdk"

# Defaults
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
BUILD_TOOLS_VER="${BUILD_TOOLS_VER:-35.0.0}"
KEYSTORE="${KEYSTORE:-$HOME/.android/debug.keystore}"
KEYSTORE_PASS="${KEYSTORE_PASS:-android}"
BUNDLED_LOADER="$SCRIPT_DIR/libs/arm64-v8a/libopenxr_loader.so"
OPENXR_LOADER="${OPENXR_LOADER:-}"
[[ -z "$OPENXR_LOADER" && -f "$BUNDLED_LOADER" ]] && OPENXR_LOADER="$BUNDLED_LOADER"
SCENE_DIR=""

BUILD_TOOLS="$ANDROID_HOME/build-tools/$BUILD_TOOLS_VER"
PLATFORM="$ANDROID_HOME/platforms/android-35"
JNI_DIR="$SCRIPT_DIR/jni/arm64-v8a"
BUILD_DIR="$SCRIPT_DIR/build"

# Locate NDK
NDK_HOME="${ANDROID_NDK_HOME:-${NDK_HOME:-${ANDROID_NDK_ROOT:-}}}"
if [[ -z "$NDK_HOME" && -d "$ANDROID_HOME/ndk" ]]; then
    NDK_HOME="$(ls -1d "$ANDROID_HOME/ndk/"* 2>/dev/null | sort -V | tail -1)"
fi

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --scene-dir) SCENE_DIR="$2"; shift 2 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

# Validate prerequisites
check_cmd() { command -v "$1" &>/dev/null || { echo "ERROR: '$1' not found. $2"; exit 1; }; }
check_cmd cargo-ndk "Install with: cargo install cargo-ndk"
check_cmd aapt      "Install Android build-tools via Android Studio SDK Manager"

[[ -n "$NDK_HOME" ]] || { echo "ERROR: Android NDK not found. Install via Android Studio SDK Manager or set ANDROID_NDK_HOME."; exit 1; }
LIB_CPP_SHARED="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so"
[[ -f "$LIB_CPP_SHARED" ]] || { echo "ERROR: libc++_shared.so not found at: $LIB_CPP_SHARED"; exit 1; }

[[ -n "$OPENXR_LOADER" ]] || {
    echo "ERROR: OpenXR loader not found. Run the fetch script first:"
    echo "  ./android/quest/fetch_loader.sh"
    echo ""
    echo "Or set OPENXR_LOADER to an existing libopenxr_loader.so:"
    echo "  OPENXR_LOADER=~/ovr_openxr_mobile_sdk/OpenXR/Libs/Android/arm64-v8a/Release/libopenxr_loader.so \\"
    echo "  ./android/quest/build.sh"
    exit 1
}
[[ -f "$OPENXR_LOADER" ]] || { echo "ERROR: OpenXR loader not found: $OPENXR_LOADER"; exit 1; }

if [[ -n "$SCENE_DIR" && ! -d "$SCENE_DIR" ]]; then
    echo "ERROR: --scene-dir not found: $SCENE_DIR"
    exit 1
fi

echo "==> Build configuration"
echo "    Android SDK    : $ANDROID_HOME"
echo "    Build tools    : $BUILD_TOOLS_VER"
echo "    OpenXR loader  : $OPENXR_LOADER"
DEFAULT_SCENE="$WORKSPACE_ROOT/res/default.json"
if [[ -n "$SCENE_DIR" ]]; then
    SCENE_LABEL="$SCENE_DIR"
elif [[ -f "$DEFAULT_SCENE" ]]; then
    SCENE_LABEL="res/default.json (bundled default)"
else
    SCENE_LABEL="<not set — push scene separately>"
fi
echo "    Scene          : $SCENE_LABEL"
echo ""

# Step 1: Build libxrds_app.so
echo "==> Step 1: Building libxrds_app.so..."
mkdir -p "$JNI_DIR"
# Absolute path so the linker (cargo-ndk proxy) can find libopenxr_loader.so.
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-L $SCRIPT_DIR/libs/arm64-v8a -C link-arg=-Wl,--allow-shlib-undefined"
(cd "$WORKSPACE_ROOT" && cargo ndk -t arm64-v8a -o "$SCRIPT_DIR/jni" build --release -p xrds-app)
unset CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS
echo "    Built: $JNI_DIR/libxrds_app.so"

# Step 2: Copy OpenXR loader
echo "==> Step 2: Copying OpenXR loader..."
cp "$OPENXR_LOADER" "$JNI_DIR/"
echo "    Copied: $JNI_DIR/libopenxr_loader.so"

# Step 3: Stage assets
echo "==> Step 3: Staging assets..."
STAGING="$BUILD_DIR/assets"
rm -rf "$STAGING"
mkdir -p "$STAGING"

# Copy SDK runtime assets (skip editor-only icons and dev-only sample models)
cp -r "$WORKSPACE_ROOT/assets/fonts"            "$STAGING/"
cp -r "$WORKSPACE_ROOT/assets/shaders"          "$STAGING/" 2>/dev/null || true
cp -r "$WORKSPACE_ROOT/assets/environment_maps" "$STAGING/" 2>/dev/null || true
cp -r "$WORKSPACE_ROOT/assets/sound"            "$STAGING/" 2>/dev/null || true
cp -r "$WORKSPACE_ROOT/assets/textures"         "$STAGING/" 2>/dev/null || true

# Bundle scene: prefer --scene-dir, fall back to res/default.json
if [[ -n "$SCENE_DIR" ]]; then
    echo "    Bundling scene from $SCENE_DIR..."
    cp "$SCENE_DIR/scene.json" "$STAGING/"
    if [[ -d "$SCENE_DIR/assets" ]]; then
        cp -r "$SCENE_DIR/assets/." "$STAGING/"
    fi
elif [[ -f "$DEFAULT_SCENE" ]]; then
    echo "    Bundling default scene from res/default.json..."
    cp "$DEFAULT_SCENE" "$STAGING/scene.json"
fi

# Generate ASSET_MANIFEST — a plain-text list of every file in the staging directory,
# relative to the staging root (= the APK assets/ root).  android_main reads this to
# extract all assets to the filesystem cache on the device, so Bevy can use normal
# file I/O instead of AAssetManager for GLBs, textures, fonts, audio, etc.
echo "    Generating ASSET_MANIFEST..."
(cd "$STAGING" && find . -type f | sed 's|^\./||' | LC_ALL=C sort) > "$STAGING/ASSET_MANIFEST"
echo "    $(wc -l < "$STAGING/ASSET_MANIFEST" | tr -d ' ') file(s) listed in manifest"

# Step 4: Package APK
echo "==> Step 4: Packaging APK..."
rm -f "$BUILD_DIR/resources.apk" "$BUILD_DIR/aligned.apk" "$BUILD_DIR/xrds-app.apk"
mkdir -p "$BUILD_DIR"

# Compile Android resources (strings.xml, etc.)
"$BUILD_TOOLS/aapt" package -f -m \
    -S "$SCRIPT_DIR/res" \
    -M "$SCRIPT_DIR/AndroidManifest.xml" \
    -I "$PLATFORM/android.jar" \
    -F "$BUILD_DIR/resources.apk"

# Add native libraries.
# Android Package Manager only extracts libs from lib/arm64-v8a/ in the APK.
# Stage them there, then add from $BUILD_DIR so the in-APK path is lib/arm64-v8a/*.so
mkdir -p "$BUILD_DIR/lib/arm64-v8a"
cp "$JNI_DIR/libxrds_app.so"      "$BUILD_DIR/lib/arm64-v8a/"
cp "$JNI_DIR/libopenxr_loader.so" "$BUILD_DIR/lib/arm64-v8a/"
cp "$LIB_CPP_SHARED"              "$BUILD_DIR/lib/arm64-v8a/"
cd "$BUILD_DIR"
"$BUILD_TOOLS/aapt" add "$BUILD_DIR/resources.apk" \
    "lib/arm64-v8a/libxrds_app.so" \
    "lib/arm64-v8a/libopenxr_loader.so" \
    "lib/arm64-v8a/libc++_shared.so"

# Add assets (must be added from build dir so paths in APK are assets/fonts/..., etc.)
cd "$BUILD_DIR"
find assets -type f | sort | while IFS= read -r f; do
    "$BUILD_TOOLS/aapt" add "$BUILD_DIR/resources.apk" "$f"
done

# Zipalign (required before signing)
cd "$WORKSPACE_ROOT"
"$BUILD_TOOLS/zipalign" -f 4 "$BUILD_DIR/resources.apk" "$BUILD_DIR/aligned.apk"

# Sign (debug keystore by default; swap for release keystore when shipping)
"$BUILD_TOOLS/apksigner" sign \
    --ks "$KEYSTORE" \
    --ks-pass "pass:$KEYSTORE_PASS" \
    --out "$BUILD_DIR/xrds-app.apk" \
    "$BUILD_DIR/aligned.apk"

echo ""
echo "==> Done: $BUILD_DIR/xrds-app.apk"
echo ""
echo "Install:"
echo "  adb install -r $BUILD_DIR/xrds-app.apk"
echo "  adb shell am start -n $PACKAGE_NAME/.MainActivity"

if [[ -z "$SCENE_DIR" && ! -f "$DEFAULT_SCENE" ]]; then
    echo ""
    echo "No scene was bundled. Push one before launching:"
    echo "  adb push scene.json /sdcard/Android/data/$PACKAGE_NAME/files/"
    echo "  adb push assets/    /sdcard/Android/data/$PACKAGE_NAME/files/assets/"
fi
