# xrds-net — Android shipping checklist

Goal: make `xrds-net` build for Android (Quest 3/Pro) so XR app dev builds can
use networking on-device, instead of the current
`#[cfg(not(target_os = "android"))]` exclusion in
[`crates/xrds-runtime/Cargo.toml`](../crates/xrds-runtime/Cargo.toml).

**Approach: investigate before amputating.** We do *not* yet know which of the
native-linked dependencies can cross-compile to Android. So the plan is (1)
remove known-dead deps, (2) actually try to build each blocker for an Android
target, then (3) decide — per protocol — whether it ships or gets marked
`unsupported` on Android. Do not mark anything unsupported until step 2 proves
it can't build.

Android target for all checks: `aarch64-linux-android` (Quest 3/Pro baseline,
min API 32). Toolchain: Android NDK + `cargo-ndk` (or explicit `CC`/`AR`/linker
env). Record findings inline in this doc as each item is resolved.

> **⚠️ §2a (native-tls/OpenSSL) and §2b (curl) are SUPERSEDED.** Their
> conclusions ("supported via the committed OpenSSL prebuilt", "curl-sys builds
> against it") were correct at the time and the work described was real — but
> the **first full `xrds-app` link failed** with dozens of
> `duplicate symbol: SSL_*` errors, because quiche's vendored BoringSSL and
> that OpenSSL prebuilt define the same OpenSSL-API symbol names and can't both
> be statically linked into one binary. That triggered a crypto-library
> consolidation: OpenSSL, libcurl and `native-tls` are now **entirely removed**
> in favor of rustls (BoringSSL stays for QUIC only). See
> **[`xrds-net-crypto-consolidation.md`](done/xrds-net-crypto-consolidation.md)** —
> that document reflects the shipping state. `third_party/openssl/` and the
> `AARCH64_LINUX_ANDROID_OPENSSL_*` build-script wiring have both been
> **deleted** (recoverable from git history at commit `c23d360`; the
> cross-compile procedure itself is still written up in §2a.1 below, so it's
> reproducible from these docs alone if ever needed).
> Everything else here (Phase 1 toolchain, §2c quiche, §2d FTP server,
> Phase 3 feature plumbing, Phase 4 on-device notes) still stands.

---

## Phase 0 — Dead dependency cleanup (no Android impact, do first)

Both are leftovers from before the media decoupling and are unreferenced in any
`.rs` (verified: no `build.rs`, no `vcpkg::` call, no `MediaFoundation`/`windows::`
use in `src/`).

- [x] Remove the `vcpkg` **build-dependency** from
      [`crates/xrds-net/Cargo.toml`](../crates/xrds-net/Cargo.toml) (§`[build-dependencies]`).
- [x] Remove the `[target.'cfg(windows)'.dependencies] windows = { … "Win32_Media_MediaFoundation" }`
      block from the same file.
- [x] `cargo build -p xrds-net` (Windows) still passes after removal.
- [x] `cargo build -p xrds-runtime` still passes.

---

## Phase 1 — Toolchain setup

- [x] Install Android NDK and add the `aarch64-linux-android` Rust target.
      *Already present:* NDK `28.0.12674087` at `ANDROID_NDK_HOME`, target
      `aarch64-linux-android` installed.
- [x] Install `cargo-ndk`. *Present: `cargo-ndk 4.1.2`.*
- [x] Confirm a trivial `xrds-net`-free crate cross-compiles first. **PASS** —
      a hello-world crate builds clean with `cargo ndk -t arm64-v8a build`.
      Toolchain (NDK clang linker) is proven; any failures below are
      dependency problems, not toolchain problems.

---

## Phase 2 — Build each blocker for Android (the real question)

For each dependency: attempt an `aarch64-linux-android` build **in isolation**
(a tiny throwaway crate that depends only on it), record the result, and only
then decide the protocol's fate. A "blocker" is a native/C-linked dep; pure-Rust
deps (tokio, url, serde, coap-lite, rumqttc, …) are assumed fine unless a build
says otherwise.

### 2a. native-tls / OpenSSL — affects `wss://`, `ftps`

Pulled via `tokio-tungstenite = { features = ["native-tls"] }` and
`suppaftp = { features = ["native-tls"] }`.

Two empirical cross-compile experiments were run (both with the proven
toolchain from Phase 1):

- [x] **Default `native-tls` (system OpenSSL): FAILS.** `openssl-sys 0.9.110`
      finds no OpenSSL for the target — `AARCH64_LINUX_ANDROID_OPENSSL_DIR`
      unset, and it mis-detects the host `OPENSSL_DIR = C:\Program
      Files\OpenSSL-Win64` (wrong ABI). NDK clang CC is correctly wired, so
      this is a *missing-library* problem, not a toolchain problem.
- [x] **`openssl` `vendored` feature (build OpenSSL from source): NEARLY
      WORKS.** `openssl-src 300.6.1+3.6.3` **does** support Android — it
      auto-wired the NDK (`CC=…/clang`, `AR=…/llvm-ar`, `RANLIB=…/llvm-ranlib`)
      and invoked `perl ./Configure … linux-aarch64 -DANDROID
      --target=aarch64-linux-android21`. It failed **only** on a broken host
      Perl (`Can't locate Locale/Maketext/Simple.pm`) — a git-bash/MSYS perl
      missing modules. A complete Perl (or building under WSL/Linux) would very
      likely finish. **Conclusion: OpenSSL-for-Android is viable.**

**Conclusion:** OpenSSL for Android is a real, supported path (OpenSSL 3.x has
first-class `android-arm64` targets; `openssl-sys` links a prebuilt via env).
Two ways to feed it to `openssl-sys`:

- [x] **Route A — prebuilt cross-build (chosen).** Cross-build OpenSSL in C
      once for `aarch64-linux-android`, link it via env vars. Recipe in §2a.1.
      No per-build C compile, no Perl in the Rust build.
- [x] **Route B — `vendored` feature.** Add `openssl = { features =
      ["vendored"] }`; cargo builds OpenSSL from source each clean build.
      Requires a *complete* Perl + `make` on every build host (the git-bash
      Perl on this box is insufficient — proven above). Reproducible but adds
      C build time and a Perl dependency to CI.
- [x] **Decision:** wss/ftps → **supported via OpenSSL cross-build (Route A)**.
      `rustls` swap remains a viable fallback (both `tokio-tungstenite` and
      `suppaftp` expose `rustls-tls`) if maintaining the prebuilt proves
      painful, but the OpenSSL path is confirmed workable and is the plan.

#### 2a.1 Recipe — cross-build OpenSSL for aarch64 Android and link it

This is scripted at
[`third_party/openssl/build-openssl-android.sh`](../third_party/openssl/) — run
it once per ABI (see that dir's README). The prebuilt lands in
`third_party/openssl/<abi>/` (per the layout decision: a top-level, per-ABI,
build-time input shared by openssl-sys **and** curl-sys). Unlike the OpenXR
loader (`android/quest/libs`, fetched on demand), **the built `lib/`+`include/`
are committed to git alongside the recipe** — libs and headers are one
artifact and OpenSSL here is small/stable enough to check in directly,
matching the common convention of projects like
[PurpleI2P/OpenSSL-for-Android-Prebuilt](https://github.com/PurpleI2P/OpenSSL-for-Android-Prebuilt).
The steps below are what the script does.

- [x] **Layout decided and populated:** `third_party/openssl/arm64-v8a/{lib,include}`
      committed (built remotely on Linux, copied in: `libssl.a` 2.0 MB +
      `libcrypto.a` 11.3 MB + 142 headers, ~16 MB total).
- [x] **Full linked build VERIFIED — PASSES.**
      `cargo ndk -t arm64-v8a -P 32 build -p xrds-net --no-default-features`
      compiles clean end to end (see recipe below). This is the real, working
      recipe — including two gotchas discovered only by actually running it.

Build OpenSSL once (do this where Perl + `make` work cleanly — **Linux/WSL/macOS
recommended**; the Windows git-bash Perl is missing modules, proven earlier).
The resulting static `.a` archives are aarch64-android objects and link fine
from the Windows dev box afterward — this is exactly how the committed
`third_party/openssl/arm64-v8a/` prebuilt was produced (built remotely on Linux).

```bash
# 1. Cross-build OpenSSL 3.x (static) for arm64-v8a, API 32 (Quest 3 baseline)
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
HOST_TAG=linux-x86_64   # or windows-x86_64 / darwin-x86_64
export PATH="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/$HOST_TAG/bin:$PATH"
cd openssl-3.x
./Configure android-arm64 no-shared no-tests -D__ANDROID_API__=32 \
    --prefix="$PWD/build/arm64-v8a"
make -j"$(nproc)"
make install_sw          # → build/arm64-v8a/{lib/libssl.a,libcrypto.a, include/}
```

```bash
# 2. Point openssl-sys at it. VERIFIED WORKING FORM — set ALL THREE
#    target-scoped vars explicitly (not just *_OPENSSL_DIR).
export AARCH64_LINUX_ANDROID_OPENSSL_DIR="$PWD/third_party/openssl/arm64-v8a"
export AARCH64_LINUX_ANDROID_OPENSSL_LIB_DIR="$PWD/third_party/openssl/arm64-v8a/lib"
export AARCH64_LINUX_ANDROID_OPENSSL_INCLUDE_DIR="$PWD/third_party/openssl/arm64-v8a/include"
export OPENSSL_STATIC=1
cargo ndk -t arm64-v8a -P 32 build -p xrds-net --no-default-features
```

Key points (both below were real failures hit and fixed during verification,
not theoretical):

- **`-P` (capital), not `-p`, for the API level.** cargo-ndk 4.1.2 uses `-p`
  for cargo's own `--package` passthrough; `-P`/`--platform` is the Android API
  level flag. Using `-p 32` here got silently absorbed as an (invalid) package
  name and cargo-ndk panicked (`unknown package: 32`).
- **Must set all three `AARCH64_LINUX_ANDROID_OPENSSL_{DIR,LIB_DIR,INCLUDE_DIR}`
  vars, not just `_DIR`.** `openssl-sys` resolves `OPENSSL_LIB_DIR` /
  `OPENSSL_INCLUDE_DIR` independently, and a **non-target-scoped** `OPENSSL_LIB_DIR`
  already set globally in this dev environment (pointing at the host Windows
  OpenSSL, `C:\Program Files\OpenSSL-Win64\lib\...`) silently won over the
  derived `..._OPENSSL_DIR/lib` path — headers resolved correctly (no global
  `OPENSSL_INCLUDE_DIR` was set) but the link step failed with
  `could not find native static library 'ssl'` until the lib/include vars were
  set explicitly and target-scoped.
- Keep the API level aligned: OpenSSL `-D__ANDROID_API__=32` **and** cargo-ndk
  `-P 32`, matching the Quest 3/Pro baseline (min API 32 per `CLAUDE.md`).
- `no-shared` + `OPENSSL_STATIC=1` → static link, no `libssl.so` to ship in the
  APK.
- **Decision:** wss/ftps/http/https/quic — all confirmed buildable against this
  one prebuilt (see 2b/2c below); OpenSSL-for-Android is fully unblocked.

### 2b. libcurl (`curl` crate) — affects `http`/`https`

- [x] Attempt to cross-compile the `curl` crate for `aarch64-linux-android`.
      **PASSES**, with no extra wiring beyond 2a — `curl-sys` builds its own
      vendored libcurl and links it against the same committed OpenSSL
      prebuilt (`AARCH64_LINUX_ANDROID_OPENSSL_*`). No separate curl-specific
      env needed.
- [x] **Decision:** http/https → **supported** (builds against the committed
      OpenSSL prebuilt from 2a). No swap needed.

### 2c. quiche / BoringSSL — affects `quic`, HTTP/3

- [x] Attempt to cross-compile `quiche 0.24` for `aarch64-linux-android`
      (BoringSSL via cmake + NDK). **PASSES**, with one Windows-host-specific
      CMake wrinkle (details below) — not an Android/BoringSSL incompatibility.
- [x] Record: does BoringSSL's cmake build find the NDK toolchain cleanly?
      **Not by default on this box.** CMake's default generator on Windows
      picked up a stale, unrelated **Visual Studio "Android Application"**
      toolset (a leftover VS2019 Android workload bundling `android-ndk-r16b`)
      instead of our NDK 28/cargo-ndk clang — its ancient linker doesn't
      understand modern flags (`--no-rosegment`) and crashed. Fix: force CMake
      onto Ninja explicitly:
      ```bash
      export CMAKE_GENERATOR="Ninja"
      export CMAKE_MAKE_PROGRAM="$ANDROID_HOME/cmake/3.31.1/bin/ninja.exe"  # ships with Android SDK's cmake package
      ```
      With that, BoringSSL's CMake configure + build succeeded and produced
      valid aarch64 Android static libs (`libcrypto.a` 8.6 MB, `libssl.a` 14 MB,
      verified as real archives, not stubs).
- [x] **Fixed properly via a vendored patch.** quiche 0.24.6's `build.rs`
      expects its BoringSSL artifacts one directory higher (`out/libssl.a`)
      than where they land under a single-config generator like Ninja
      (`out/build/libssl.a`) — its output-dir guess
      (`get_boringssl_platform_output_path()`) branches on whether the *host*
      is MSVC and assumes a Visual-Studio-style `build/<Config>` subfolder
      regardless of which generator is actually used for the (Android)
      target build. Patched at
      [`patches/quiche/src/build.rs`](../patches/quiche/src/build.rs) — adds a
      fallback to the plain `{bssl_dir}/build` path before giving up — and
      wired via `[patch.crates-io]` in the workspace root
      [`Cargo.toml`](../Cargo.toml), following the same vendored-patch
      convention already used for `bevy_winit`. **Gotcha:** `[patch]` only
      substitutes an *exact* version match — with `quiche = "0.24.6"` (semver
      range `^0.24.6`), cargo silently resolved to a newer `0.24.9` from
      crates.io instead of the patch, and the patch was ignored (with a
      `patch ... was not used` warning easy to miss). Fixed by pinning
      [`crates/xrds-net/Cargo.toml`](../crates/xrds-net/Cargo.toml) to
      `quiche = "=0.24.6"` (exact). Verified: full clean build (quiche build
      dirs wiped first) succeeds with **no manual copy step**
      (`cargo ndk -t arm64-v8a -P 32 build -p xrds-net --no-default-features`
      → `Finished`, confirmed showing `Compiling quiche v0.24.6
      (…/patches/quiche)`).
- [x] **Considered upgrading to quiche 0.29.3 instead — investigated,
      rejected.** 0.29.3 drops the whole vendored-cmake-BoringSSL path in
      favor of the `boring`/`boring-sys` crate (`default = ["boringssl-boring-crate"]`),
      which would remove this bug class entirely. Desktop build + all 7 QUIC
      tests passed clean against 0.29.3 with **zero API drift** in our
      `quic.rs`/`quic_server.rs`. But cross-compiling `boring-sys` to Android
      on this Windows host hit a **different, open, unresolved upstream bug**:
      cargo-ndk's `CC_aarch64-linux-android`/`CXX_...` env vars point at
      `clang`/`clang++` without the `.exe` suffix; `cc-rs`-based crates
      (openssl-sys, curl-sys) tolerate this (Windows process-launch silently
      appends `.exe`), but CMake's own compiler-path validation does a strict
      exists-check and rejects it — and cargo-ndk unconditionally re-exports
      those vars itself, so overriding them before invoking `cargo ndk` has no
      effect. This is exactly
      [cloudflare/quiche#2020](https://github.com/cloudflare/quiche/issues/2020)
      (same host/target combo: Windows → `aarch64-linux-android`, CMake
      defaulting away from a working generator), open with no fix or
      workaround as of this writing. **Decision: stay on 0.24.6 + the patch
      above** rather than trade a 3-line fixable bug in our own dependency
      graph for an unresolved bug in someone else's crate. Revisit if/when
      that upstream issue is resolved.
- [x] **Decision:** quic/http3 → **supported** (BoringSSL cross-builds cleanly
      via the quiche 0.24.6 vendored patch). No `quinn` swap needed; no
      upgrade to 0.29.3 for now (see above).

### 2d. FTP server (`libunftp` + `unftp-sbe-fs`) — server-side only

An XR **client** app never needs to *host* FTP. This is dead weight on Android
regardless of whether it builds.

- [x] Feature-gate the FTP **server** behind a default-on `ftp-server` feature
      (`libunftp` + `unftp-sbe-fs` are now `optional`). Android disables it via
      `default-features = false`. `FTP`/`SFTP` with the feature off logs a
      warning instead of link-erroring. Verified: lib + tests compile both
      with and without the feature.
- [x] Confirm the client `transfer("ftp://…")` path does not depend on the
      server crates — it uses `suppaftp` (client), unaffected by the gate.
- [x] **Decision:** FTP server → **excluded on Android** (feature off); client
      `ftp`/`sftp` transfer still follows 2a (its `native-tls`).

### 2e. webrtc — out of scope

- [ ] No action. WebRTC stays desktop-only and out of the `xrds::net` path by
      decision. Not part of this shipping effort.

---

## Phase 3 — Wire up the Android build

- [x] Replaced the blanket `#[cfg(not(target_os = "android"))]` exclusion in
      [`crates/xrds-runtime/src/lib.rs`](../crates/xrds-runtime/src/lib.rs) —
      `xrds-net` is now a normal (unconditional) dependency of `xrds-runtime`,
      re-exported as `net` on every platform.
- [x] Wired the env vars from §2a.1/§2c into
      [`android/quest/build.sh`](../android/quest/build.sh) around the
      `cargo ndk` invocation: all three
      `AARCH64_LINUX_ANDROID_OPENSSL_{DIR,LIB_DIR,INCLUDE_DIR}` +
      `OPENSSL_STATIC=1`, and `CMAKE_GENERATOR=Ninja` +
      `CMAKE_MAKE_PROGRAM` (auto-detected: system `ninja` first, else the one
      bundled with the Android SDK's `cmake` package). Also added `-P 32`
      (API level) to the `cargo ndk` call, which was previously missing
      (defaulting to API 21) — now matches the OpenSSL prebuilt's API level
      and the Quest 3/Pro baseline in `CLAUDE.md`.
- [x] `cargo ndk -t arm64-v8a -P 32 build -p xrds-runtime --no-default-features`
      passes clean (only pre-existing unrelated warnings).
- [x] **`ftp-server` exclusion needed a real structural fix, not a
      `[target.cfg(...)]` toggle — document this well, it's non-obvious.**
      Discovered while testing `xrds-runtime` (not just `xrds-net` alone):
      `libunftp` → `aws-lc-sys` was still being cross-compiled despite
      `xrds-net`'s `ftp-server` feature supposedly being off for Android.
      Root-caused via `cargo metadata --filter-platform aarch64-linux-android`
      (shows the *actual* resolved feature set per package — trust this over
      `cargo tree`, whose `-e features` display doesn't reliably reflect
      target-cfg resolution) and Cargo's own emitted warning:
      > warning: default-features is ignored for xrds-runtime, since
      > default-features was not specified for workspace.dependencies.xrds-runtime
      Two compounding Cargo behaviors, both worth remembering:
      1. **Feature unification is workspace-wide, not per-`-p`.** Cargo
         resolves one merged feature set per dependency across *every*
         workspace member's manifest for a given target — even when you only
         build one crate with `-p`. An un-gated, default-featured dependency
         edge on `xrds-net`/`xrds-runtime` from *any* other workspace member
         (the root `xrds` package's dev-dependency for its examples, the
         editor, `xrds-app`) leaks `ftp-server` into every other consumer's
         Android build. `[target.'cfg(...)'.dependencies]` only controls
         whether the *edge exists* for a target — it does **not** create
         per-target-isolated feature sets for the same dependency.
      2. **A member's `default-features = false` override is silently
         ignored unless the *workspace-level* `[workspace.dependencies.X]`
         entry also explicitly declares `default-features` (and specifically
         `= false`, not `= true`).** Members may only be *more restrictive
         than or equal to* the workspace baseline, never *less* — so the
         workspace entry must declare the restrictive (`false`) baseline, and
         each consumer opts back in to just the named features it needs
         (`xrds-net/ftp-server`), which works regardless of the
         `default-features` flag since naming a feature directly bypasses
         `default`.
      **Fix applied** (real feature-forwarding chain, no target-cfg feature
      toggling anywhere):
      - [`Cargo.toml`](../Cargo.toml) — `[workspace.dependencies.xrds-net]`
        and `[workspace.dependencies.xrds-runtime]` now explicitly declare
        `default-features = false`.
      - [`crates/xrds-net/Cargo.toml`](../crates/xrds-net/Cargo.toml) —
        unchanged (`default = ["ftp-server"]`, already correct).
      - [`crates/xrds-runtime/Cargo.toml`](../crates/xrds-runtime/Cargo.toml)
        — single unconditional `xrds-net = { workspace = true, default-features = false }`
        edge (the two conflicting target-cfg blocks removed); own
        `[features] default = ["ftp-server"]` /
        `ftp-server = ["xrds-net/ftp-server"]`.
      - [`apps/xrds-app/Cargo.toml`](../apps/xrds-app/Cargo.toml) — same
        pattern, forwarding `xrds-runtime/ftp-server`.
      - [`apps/xrds-editor/src-tauri/Cargo.toml`](../apps/xrds-editor/src-tauri/Cargo.toml)
        — same pattern (editor is desktop-only per `CLAUDE.md` but must not
        leak an un-gated default-featured edge into the shared resolution).
      - Root [`Cargo.toml`](../Cargo.toml)'s own `[dependencies]` (desktop
        sample binary) and `[dev-dependencies]` (examples) — same pattern.
      - [`android/quest/build.sh`](../android/quest/build.sh) — the real
        Android build now passes `cargo ndk ... build -p xrds-app
        --no-default-features` to actually drop `ftp-server` for the shipped
        binary.
      **Verified:** `cargo check -p xrds-runtime --target aarch64-linux-android
      --no-default-features` no longer compiles `aws-lc-sys`/`libunftp` at
      all (confirmed via `Compiling` line absence), and the full
      `cargo ndk` build (env vars + `-P 32` + `--no-default-features`) of
      `xrds-runtime` finishes clean.
- [ ] Introduce finer-grained Cargo features in `xrds-net` if a future need
      arises to drop other individual protocols per platform (not needed yet
      — only `ftp-server` required exclusion).
- [ ] Any protocol marked `unsupported` returns a clear
      `NetError::Capability` at runtime on Android (never a link error, never a
      hang) — not yet explicitly re-tested after this session's changes;
      revisit in Phase 4's on-device pass.

---

## Phase 4 — On-device verification (Quest 3/Pro)

- [ ] Build a minimal `XrdsApp` that does one `request_async` (http) + one
      `listen_feed` (mqtt or ws) and poll them in `update()`.
- [ ] Deploy to Quest, confirm the calls complete on-device (not just link).
- [ ] Confirm no runtime crash from a missing TLS/crypto provider (rustls
      `ring`/`aws-lc` provider must init on Android — verify at startup).

---

## Phase 5 — Docs

- [ ] Update the platform matrix in [`README.md`](../crates/xrds-net/README.md)
      and [`MANUAL.md`](../crates/xrds-net/MANUAL.md) §12 with per-protocol
      Android support (supported / unsupported), replacing the blanket
      "desktop only" note.
- [ ] Update `CLAUDE.md` platform table if Android status for SDK/runtime
      networking changes.
- [ ] Move this checklist to `docs/done/` when shipped.

---

## Decision log

*(Record Phase 2 outcomes here as they land — one line per protocol: built /
swapped-to-X / unsupported, with the reason.)*

- native-tls (wss, ftps): ~~SUPPORTED via committed OpenSSL prebuilt~~ →
  **SUPERSEDED: replaced by rustls.** The prebuilt-OpenSSL approach did build,
  but collided with quiche's BoringSSL at final link. `native-tls` is gone
  entirely. See
  [`xrds-net-crypto-consolidation.md`](done/xrds-net-crypto-consolidation.md).
- curl (http/https): ~~SUPPORTED via the same OpenSSL prebuilt~~ →
  **SUPERSEDED: replaced by `reqwest` + rustls.** Same reason; libcurl has no
  rustls backend, so the crate had to go. Ditto.
- quiche (quic/http3): **SUPPORTED — verified**, with a documented CMake
  generator workaround (force Ninja, see §2c). The build-script output-path
  quirk was fixed properly via the vendored `patches/quiche` patch (no manual
  copy step). **This is the only remaining native/CMake dependency and the
  only non-rustls crypto backend.**
- ftp server: **excluded on Android** — feature-gated (`ftp-server`, off on
  Android); client transfer path unaffected.
