#!/usr/bin/env bash
# third_party/openssl/build-openssl-android.sh
#
# Cross-builds a STATIC OpenSSL for Android and installs it into a per-ABI
# directory next to this script:
#
#     third_party/openssl/<abi>/{lib/{libssl.a,libcrypto.a}, include/openssl/...}
#
# These static archives + headers are BUILD-TIME inputs consumed by openssl-sys
# (and later curl-sys). They are statically linked into libxrds_app.so, so
# NOTHING extra ships in the APK — there is no libssl.so to stage.
#
# The build itself needs a COMPLETE Perl + make. On Windows the git-bash/MSYS
# Perl is missing modules (Locale::Maketext::Simple) and WILL fail — run this
# under WSL / Linux / macOS. The resulting .a files are aarch64/x86_64 Android
# objects and link fine from the Windows dev box afterward.
#
# Usage:
#     ANDROID_NDK_HOME=/path/to/ndk ./build-openssl-android.sh [arm64-v8a|x86_64] ...
#
# Defaults to arm64-v8a (Quest 3/Pro device). Pass x86_64 too for the emulator.
#
# See docs/xrds-net-android-shipping.md §2a.1 for the rationale and env wiring.

set -euo pipefail

# --- config ----------------------------------------------------------------
OPENSSL_VERSION="${OPENSSL_VERSION:-3.5.0}"   # pin; bump deliberately
ANDROID_API="${ANDROID_API:-32}"              # Quest 3/Pro baseline (min API 32)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ABIS=("$@")
[ ${#ABIS[@]} -eq 0 ] && ABIS=("arm64-v8a")

# --- checks -----------------------------------------------------------------
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to your NDK (e.g. .../Sdk/ndk/28.x)}"
command -v perl >/dev/null || { echo "ERROR: perl not found"; exit 1; }
command -v make >/dev/null || { echo "ERROR: make not found"; exit 1; }
# Fail early on the known-bad Windows Perl rather than deep inside Configure.
perl -MLocale::Maketext::Simple -e1 2>/dev/null || {
    echo "ERROR: this Perl is missing Locale::Maketext::Simple (the git-bash/MSYS"
    echo "       Perl on Windows). Run this script under WSL / Linux / macOS."
    exit 1
}

case "$(uname -s)" in
    Linux*)  HOST_TAG="linux-x86_64" ;;
    Darwin*) HOST_TAG="darwin-x86_64" ;;
    *)       HOST_TAG="windows-x86_64" ;;   # unlikely to succeed; see note above
esac
TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG"
[ -d "$TOOLCHAIN" ] || { echo "ERROR: NDK toolchain not found at $TOOLCHAIN"; exit 1; }
export PATH="$TOOLCHAIN/bin:$PATH"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"

# --- fetch source (once, cached) -------------------------------------------
SRC_ROOT="$SCRIPT_DIR/.src"
SRC_DIR="$SRC_ROOT/openssl-$OPENSSL_VERSION"
mkdir -p "$SRC_ROOT"
if [ ! -d "$SRC_DIR" ]; then
    TARBALL="openssl-$OPENSSL_VERSION.tar.gz"
    URL="https://github.com/openssl/openssl/releases/download/openssl-$OPENSSL_VERSION/$TARBALL"
    echo "==> Downloading $URL"
    curl -fsSL "$URL" -o "$SRC_ROOT/$TARBALL"
    tar -xzf "$SRC_ROOT/$TARBALL" -C "$SRC_ROOT"
fi

# --- build per ABI ----------------------------------------------------------
for ABI in "${ABIS[@]}"; do
    case "$ABI" in
        arm64-v8a) OSSL_TARGET="android-arm64" ;;
        x86_64)    OSSL_TARGET="android-x86_64" ;;
        armeabi-v7a) OSSL_TARGET="android-arm" ;;
        x86)       OSSL_TARGET="android-x86" ;;
        *) echo "ERROR: unknown ABI '$ABI'"; exit 1 ;;
    esac

    PREFIX="$SCRIPT_DIR/$ABI"
    BUILD="$SRC_ROOT/build-$ABI"
    echo "==> Building OpenSSL $OPENSSL_VERSION for $ABI ($OSSL_TARGET, API $ANDROID_API)"
    rm -rf "$BUILD" "$PREFIX"
    cp -r "$SRC_DIR" "$BUILD"
    (
        cd "$BUILD"
        ./Configure "$OSSL_TARGET" no-shared no-tests no-apps \
            -D__ANDROID_API__="$ANDROID_API" --prefix="$PREFIX" --libdir=lib
        make -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
        make install_sw
    )
    echo "==> Installed: $PREFIX/lib/{libssl.a,libcrypto.a} + include/"
done

echo
echo "Done. Point openssl-sys at it (target-scoped var overrides any host OPENSSL_DIR):"
echo "  export AARCH64_LINUX_ANDROID_OPENSSL_DIR=$SCRIPT_DIR/arm64-v8a"
echo "  export OPENSSL_STATIC=1"
