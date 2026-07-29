# third_party/openssl

Prebuilt **static** OpenSSL for **Android**, one directory per ABI. These are
build-time inputs for `openssl-sys` (and later `curl-sys`) when cross-compiling
`xrds-net` to Android — they are statically linked into `libxrds_app.so`, so
**nothing here ships in the APK** as a separate `.so`.

Desktop targets (Windows/Linux/macOS) do **not** use these — they use the OS TLS
stack (system OpenSSL / SChannel / Security.framework). This directory is
Android-only, keyed by ABI, not "OpenSSL for every platform".

## Layout

```text
third_party/openssl/
  build-openssl-android.sh   # the recipe (committed)
  README.md                  # this file
  arm64-v8a/                 # COMMITTED prebuilt
    lib/{libssl.a, libcrypto.a}
    include/openssl/...
  x86_64/                    # optional (emulator), same shape, also committed
  .src/                      # downloaded OpenSSL source cache; gitignored
```

**Both the recipe and the built `lib/`/`include/` are committed to git** —
libs and headers are one artifact (`openssl-sys` cannot build without the
headers matching the libs' build), so they're checked in together for a
working checkout with no WSL/NDK step required. This follows the common
Rust-for-Android convention of committing prebuilt OpenSSL trees directly
(e.g. [PurpleI2P/OpenSSL-for-Android-Prebuilt](https://github.com/PurpleI2P/OpenSSL-for-Android-Prebuilt),
[XDcobra/openssl-android-prebuilt-and-buildscripts](https://github.com/XDcobra/openssl-android-prebuilt-and-buildscripts)),
rather than the fetch-on-demand approach used for the OpenXR loader
(`android/quest/libs`) — OpenSSL here is small, architecture-narrow, and
rarely changes, so committing it outweighs the cost of an extra fetch step.

Only the transient downloaded OpenSSL **source** (`.src/`, used only while
running the build script) is gitignored — never commit that.

**Bump procedure:** re-run `build-openssl-android.sh` with a new
`OPENSSL_VERSION`, verify, then commit the changed `lib/`/`include/` as a
normal diff.

## Build (run under WSL / Linux / macOS)

The OpenSSL build needs a complete Perl + `make`. The Windows git-bash/MSYS Perl
is missing modules and fails at `Configure` — build under WSL/Linux/macOS. The
resulting `.a` archives are Android objects and link fine from a Windows dev box.

```bash
ANDROID_NDK_HOME=/path/to/Sdk/ndk/28.x \
  ./third_party/openssl/build-openssl-android.sh arm64-v8a
# optionally add the emulator ABI:
#   ./build-openssl-android.sh arm64-v8a x86_64
```

Pin the version / API level via `OPENSSL_VERSION` and `ANDROID_API` env (default
3.5.0 / API 32 = Quest 3/Pro baseline).

## Consume it

The Quest build script exports these automatically (see `android/quest/build.sh`).
To build `xrds-net` directly:

```bash
export AARCH64_LINUX_ANDROID_OPENSSL_DIR="$PWD/third_party/openssl/arm64-v8a"
export OPENSSL_STATIC=1
cargo ndk -t arm64-v8a -p 32 build -p xrds-net --no-default-features
```

Use the **target-scoped** `AARCH64_LINUX_ANDROID_OPENSSL_DIR`, not plain
`OPENSSL_DIR` — a host `OPENSSL_DIR` (e.g. `C:\Program Files\OpenSSL-Win64`)
would otherwise be picked up with the wrong ABI.

See [`docs/xrds-net-android-shipping.md`](../../docs/xrds-net-android-shipping.md)
§2a for the full rationale.
