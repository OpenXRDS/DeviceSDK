# xrds-net — crypto library consolidation

## Context

While bringing up the Android build (see
[`docs/xrds-net-android-shipping.md`](xrds-net-android-shipping.md)), the first
full link of a real binary (`xrds-app`, not just a library crate) failed with
`ld.lld: error: duplicate symbol: SSL_use_certificate` and dozens more like it.

Root cause: `quiche`'s vendored BoringSSL and our OpenSSL prebuilt (pulled in
via `curl`/`native-tls`) both define the same OpenSSL-API-compatible symbol
names, and both were being statically linked into the same binary. Two
`rlib`s can coexist with colliding symbols (no link happens between library
crates); the moment a real binary/cdylib links both in, it's a hard error.

Investigating *why* led to a broader finding: `xrds-net` was accumulating
crypto/TLS backends ad hoc, one per protocol library, with no unifying
policy:

| Protocol | Backend (before) |
| --- | --- |
| http/https (`curl`) | OpenSSL (`curl-sys`) — no rustls option exists for libcurl |
| ws/wss (`tokio-tungstenite`) | `native-tls` (Schannel/SecurityFramework/OpenSSL depending on OS) |
| ws/wss (**dead** 2nd impl, the `websocket` crate) | `native-tls`, same as above |
| ftp/ftps client (`suppaftp`) | `native-tls`, same as above |
| quic/http3 (`quiche`) | BoringSSL (vendored) — no vanilla-OpenSSL option (needs `quictls`, a BoringSSL-API-compatible OpenSSL fork, if not BoringSSL) |
| mqtt (`rumqttc`) | already rustls |
| coap (`coap`/`coap-lite`) | already rustls |
| webrtc (out of scope) | already rustls |
| ftp **server** (`libunftp`, desktop-only, already Android-excluded) | already rustls |

Four different crypto backends (OpenSSL, `native-tls`'s three OS backends,
BoringSSL, rustls) for one crate. That's the "non-sense" — not a technical
requirement, just accumulated default choices.

**The sharper technical framing** (not "native vs pure-Rust", but "CMake vs
`cc-rs`"): every real fight in the Android bring-up — the stale-VS-toolset
generator hijack, the `.exe`-suffix rejection, the install-path bug — was
**CMake** specifically. `cc-rs`-based native deps (`ring`, `curl-sys`,
`openssl-sys`'s own shim) cross-compiled clean every time, no fighting.
`quiche`'s BoringSSL and `boring-sys`'s BoringSSL (CMake-based) both required
extensive workarounds — and `quinn`'s alternative (`boring-sys`) hits the
*same* open, unresolved upstream CMake bug on this host/target combo
(cloudflare/quiche#2020), so QUIC is stuck with a CMake-based BoringSSL build
no matter what we choose for everything else. That cost is already paid and
scripted around (`CMAKE_GENERATOR=Ninja`, the quiche install-path patch) — see
`xrds-net-android-shipping.md` §2c.

`rustls` + `ring` is `cc-rs`-based (no CMake) and has already cross-compiled
clean, twice, in this exact session. Its symbols are from-scratch crypto
primitives, not an OpenSSL-API-compatible fork, so it does **not** collide
with quiche's BoringSSL at the linker — removing OpenSSL removes both the
CMake-adjacent fragility *and* the symbol collision as a side effect.

## Decision

- **QUIC/HTTP3 keeps BoringSSL** (vendored inside `quiche`, via our patch).
  Unavoidable, already solved, not touched by this effort.
- **Everything else moves to rustls** (`ring` provider) — **universally**,
  not just for Android. One implementation per protocol, all platforms.
  Applying the swap only to Android would mean permanently maintaining two
  implementations of http/ws/ftp (desktop-curl vs Android-something-else) —
  the opposite of the goal.
- End state: **exactly two crypto backends, total** — BoringSSL (isolated
  inside quiche) and rustls+ring (everything else, incl. mqtt/coap/webrtc,
  which are already there).

Known, accepted risk: `ring` is proven *in this environment, today* — it has
had its own Android cross-compile issues historically on other setups (this
is exactly why `aws-lc-rs`, rustls's other backend, has a known-broken Android
arm64 bug — see `xrds-net-android-shipping.md`). Keep the OpenSSL-prebuilt
recipe (`third_party/openssl/`) and its build script documented/available
even after this migration, in case `ring` ever regresses on a future
NDK/toolchain bump — don't delete the institutional knowledge, even if the
artifact itself eventually stops being consumed.

---

## Phase 0 — quick win: retire the dead synchronous WebSocket path

Confirmed dead/superseded by the async `tokio-tungstenite`-based session path
(`XrdsNet::open`/`NetChannel`). Removing it deletes one of the three
`native-tls` consumers for free, before touching anything else.

- [x] Remove `crates/xrds-net/src/client/xrds_websocket.rs`.
- [x] Remove the `websocket = "0.27.1"` dependency from
      [`crates/xrds-net/Cargo.toml`](../../crates/xrds-net/Cargo.toml).
- [x] Remove references in `crates/xrds-net/src/client/mod.rs` and
      `crates/xrds-net/src/client/protocols/ws.rs`. Went further than a
      straight deletion: `StreamHandler` (dispatch/listen) was **rewired
      onto the same `WsSession`/`tokio-tungstenite` backend** `SessionHandler`
      (`open`) already used, rather than left unsupported — one WS
      implementation for all three verbs. `StreamHandler::recv` (must block
      until a message arrives, per the old contract) polls the non-blocking
      `poll_recv` in a loop; added a `disconnected: Arc<AtomicBool>` to
      `WsSession` (set on every worker-task exit path) so that loop can tell
      "nothing yet" from "nothing ever again" instead of spinning forever.
- [x] Confirmed no test coverage was specific to the old sync path in a way
      that needed porting — `StreamHandler`-targeting tests
      (`test_ws_connect`/`_send`/`_rcv`, `dispatch_ws_reaches_a_real_server`)
      exercise the *interface*, not the backend, so they kept working
      against the new shared backend unchanged (after two unrelated fixes
      below).
- [x] `cargo build -p xrds-net` + `cargo test -p xrds-net` pass — see the
      full-suite note at the end of Phase 1 (two real bugs found and fixed
      along the way, not just this deletion).

## Phase 1 — cheap rustls feature swaps

Both crates already expose a `rustls` feature; no code rewrite, just Cargo
feature changes plus whatever minimal connector-code adjustment each crate's
rustls path needs (check each — feature swaps sometimes change a type in the
connect call).

- [x] `suppaftp`: swapped `features = ["native-tls"]` → `features = ["rustls"]`
      in [`crates/xrds-net/Cargo.toml`](../../crates/xrds-net/Cargo.toml).
- [x] Checked `crates/xrds-net/src/client/protocols/ftp.rs` — **zero connector
      code changes needed**, the feature swap alone was enough (suppaftp's
      API is TLS-backend-agnostic at the call site).
- [x] `tokio-tungstenite`: swapped `features = ["native-tls"]` →
      `features = ["rustls-tls-webpki-roots"]` (webpki-roots' bundled Mozilla
      CA list, not the OS store — more portable across desktop *and* Android,
      where OS-cert-store access is less straightforward) in
      [`crates/xrds-net/Cargo.toml`](../../crates/xrds-net/Cargo.toml).
- [x] Checked `crates/xrds-net/src/client/protocols/ws.rs` — again **zero
      connector code changes needed** (`tokio_tungstenite::connect_async`
      is already TLS-backend-generic).
- [x] Re-ran the wss tests (`test_wss_session_round_trip`, the local WS
      session round-trip, plus the now-consolidated `StreamHandler` tests)
      — real, non-trivial failures surfaced (not flakiness), both fixed:
      1. **rustls needs its process-wide `CryptoProvider` installed before
         first use** (it panics otherwise: *"Could not automatically
         determine the process-level CryptoProvider..."*) — previously only
         handled ad hoc inside one WebRTC test file. This was silently
         relying on test *execution order* (whichever WebRTC test happened
         to run first installed it for everyone) — filtering the test run
         (`--lib ws`) excluded that test and every WS test failed. Fixed
         properly: added `common::ensure_rustls_crypto_provider()` (a
         `std::sync::Once`-guarded installer) and call it at the top of both
         `WsSession::connect` and `FtpHandler::connect` — the library now
         handles this itself, it's no longer an unstated requirement callers
         have to discover by crashing.
      2. `test_ws_connect` used `"https://echo.websocket.org/"` (not
         `ws://`/`wss://`) — the old, looser `websocket` crate tolerated the
         wrong scheme; `tokio_tungstenite::connect_async`'s stricter URI
         parsing correctly rejects it. Fixed the test (`wss://`), not the
         library — this was a pre-existing test bug the old backend
         happened to paper over.
- [x] Re-ran FTP tests — all 16 pass clean, no changes needed beyond the
      feature swap.
- [x] `cargo build -p xrds-net` + `cargo test -p xrds-net` pass: 139/145,
      the 6 failures are the pre-existing documented flaky baseline (5
      hardware-dependent WebRTC webcam tests + one private-server file
      download test), unrelated to this work. `native-tls` no longer
      appears anywhere in `cargo tree -p xrds-net --target all -i native-tls`.

## Phase 2 — audit before replacing curl

Before picking a replacement HTTP client, know exactly what `http.rs`
currently relies on from `curl` — the replacement needs to cover the same
ground, not less.

- [x] Read `crates/xrds-net/src/client/protocols/http.rs` in full. It's a
      genuinely thin `curl` usage — much smaller surface than anticipated:
      - Methods: **GET (default) and POST only** — nothing else.
      - Custom request headers (`ctx.req_headers`), POST body
        (`ctx.req_body`, `.post_fields_copy`).
      - Redirects: a single boolean toggle (`ctx.redirection` →
        `follow_location`) — no redirect-count limit configured.
      - Response: status code + headers + body — no streaming/chunked
        handling, no connection reuse (fresh `Easy2` handle per call).
      - **No** proxy support, **no** cookie jar, **no** HTTP/2-specific
        reliance, **no** TLS-verification toggle — `ctx.insecure` exists on
        `ClientContext` but is QUIC-only (confirmed via grep: only
        `quic.rs` reads it), `http.rs` never touches it.
      - **`PROTOCOLS::FILE` is a real gap for a straight curl→reqwest swap.**
        `request_file` calls curl's own `file://` URL handler to read local
        files (confirmed: `scheme.rs` maps `"file"` → `PROTOCOLS::FILE`,
        and the README documents `file` as "FILE (byte GET)"). **`reqwest`
        cannot fetch `file://` URLs at all** — it's HTTP-only. Decision:
        keep `request_file` on a **direct `std::fs::read`** (strip the
        `file://` prefix to a path), not reqwest — cleaner anyway, and
        removes FILE from the HTTP client's concern (and from any TLS
        discussion) entirely.
- [x] Checked `examples/net.rs`, `examples/net_app.rs`,
      `examples/net_intent.rs` — all plain GET requests checking
      `status_code`; nothing curl-specific (no header-casing checks, no
      error-message text matching). Low risk for the swap.
- [ ] Decide `ureq` vs `reqwest`:
      - `ureq`: synchronous/blocking, rustls-native, much lighter. Matches
        `XrdsNet::request` being blocking-by-design at the expert layer
        (today's `_async` forms already wrap the blocking call in a
        background thread — same shape `curl`'s blocking model already has).
      - `reqwest`: async (tokio), heavier, more complete out of the box
        (HTTP/2, cookie jar, connection pooling, redirects). Would need a
        blocking bridge to fit the existing sync `request()` — extra
        plumbing xrds-net doesn't currently have for this call.
      - Leaning `ureq` given the existing sync-first design, but confirm
        against the Phase 2 feature audit above (does xrds-net actually rely
        on anything `ureq` doesn't support, e.g. HTTP/2?).
- [x] Record the decision + rationale here before starting Phase 3.

**Decision: `reqwest`** (rustls-tls feature, blocking client via
`reqwest::blocking` to match `XrdsNet::request`'s existing sync-first shape —
avoids pulling a tokio runtime requirement into the sync expert-API path).
More complete out of the box (HTTP/2, redirects, cookies, proxies) than
`ureq`, reducing the risk of silently dropping a curl capability the Phase 2
audit finds.

## Phase 3 — replace curl

- [ ] Add the chosen crate (rustls-backed feature set) to
      [`crates/xrds-net/Cargo.toml`](../../crates/xrds-net/Cargo.toml); remove
      `curl`.
- [ ] Rewrite `crates/xrds-net/src/client/protocols/http.rs` against the new
      client, preserving the existing `ProtocolHandler`/`NetResponse`
      surface — this should be an internal swap, not a change to
      `XrdsNet::request`'s public behavior.
- [ ] Re-run existing HTTP tests (`test_client_http_*` or equivalent) against
      real endpoints (rust-lang.org, or whatever the existing suite already
      targets) to confirm parity, not just compilation.
- [ ] Confirm the `insecure` (self-signed/no-verify) `ClientContext` flag
      still works if `http.rs` used it — TLS-verification toggling needs a
      rustls equivalent (`dangerous()` config or similar), not silently
      dropped.
- [ ] `cargo tree -p xrds-net --target all -i openssl-sys` and
      `-i curl-sys` both report nothing at all — confirms OpenSSL is fully
      gone from the graph, every platform.

## Phase 4 — full re-verification

This is the actual point of the whole effort — confirm the symbol collision
is gone and nothing regressed, on both platforms.

- [x] `cargo test -p xrds-net` (desktop) — 139 pass. Remaining failures are
      the pre-existing flaky baseline only (hardware-dependent WebRTC webcam
      tests, a private-server file download, and public-server tests
      —`test.mosquitto.org` MQTT, `echo.websocket.org` — that rate-limit
      concurrent connections; the set shifts between runs and each passes in
      isolation). All HTTP (19/19), WS (8/8) and FTP (16/16) tests pass
      consistently.
- [x] `cargo build -p xrds-net` (desktop) — clean.
- [x] `cargo tree -p xrds-net --target all -i openssl-sys` and `-i curl-sys`
      both report **no matching packages** — OpenSSL and libcurl are entirely
      gone from the dependency graph, on every platform. `native-tls` too.
- [x] Android: the full `xrds-app` link — the exact thing that produced the
      duplicate-symbol error — **now succeeds**. `android/quest/build.ps1`
      runs end-to-end (exit 0), producing a 137 MB `xrds-app.apk`.
- [x] **On-device verified on Quest 3** (`adb install` + launch, output via
      logcat):
      ```text
      [net-smoke] kicking off http request_async...
      [net-smoke] opening wss session (blocking, one-time)...
      [net-smoke] wss open OK, sending...
      [net-smoke] wss send OK
      [net-smoke] http DONE status=200 body_len=559
      [net-smoke] wss DONE recv 32 bytes: "Request served by 4d896d95b55478"
      ```
      Both the reqwest/rustls HTTPS request and the rustls wss session
      round-trip **actually completed on the headset** — not just linked. No
      crypto-provider panic, no crash.
- [x] Notes for anyone repeating this on-device check:
      - `AndroidManifest.xml` needed `android.permission.INTERNET` (and
        `ACCESS_NETWORK_STATE`) added — it had neither, so *no* networking
        could have worked on-device regardless of the Rust side.
      - `eprintln!` is invisible on Android; the app routes `log::` macros to
        logcat via `android_logger` (tag `xrds`). Use `log::info!`.
      - naga's shader-validation debug output floods the default logcat ring
        buffer and evicts app lines within seconds — `adb shell logcat -G 16M`
        first, then launch, or the output is lost.
      - `android/quest/build.sh` had a real bug: its `uname -s` host detection
        had no Windows/MinGW case and silently fell through to
        `linux-x86_64`, so it couldn't find `libc++_shared.so`. Fixed (added
        `MINGW*|MSYS*|CYGWIN*` → `windows-x86_64`), though `build.ps1` is the
        native path on Windows and is what was actually used here.

## Phase 5 — cleanup & docs

- [x] **Removed `third_party/openssl/` entirely** (`git rm -r`, 146 files,
      ~16 MB) — nothing consumes it, and keeping a dead binary artifact plus
      inert build wiring is exactly the sprawl this effort set out to
      eliminate. Also removed: the `AARCH64_LINUX_ANDROID_OPENSSL_*` /
      `OPENSSL_STATIC` exports from both `android/quest/build.sh` and
      `build.ps1` (the `CMAKE_GENERATOR=Ninja` export stays — quiche's
      BoringSSL still needs it), and the `third_party/openssl/.src/`
      `.gitignore` rule.
      *Recovery note:* the artifact and its build recipe
      (`build-openssl-android.sh`) remain in git history at commit `c23d360`
      if OpenSSL is ever needed again — `git show c23d360 --stat` to find it.
      The full cross-compile procedure is also written up in
      [`xrds-net-android-shipping.md`](xrds-net-android-shipping.md) §2a.1,
      so it's reproducible from documentation alone.
- [x] Updated [`crates/xrds-net/README.md`](../../crates/xrds-net/README.md) and
      [`crates/xrds-net/MANUAL.md`](../../crates/xrds-net/MANUAL.md): platform
      table now says Android ✅ (was ✗/excluded), the stale "depends on native
      C libraries — curl/…" and "`wss` over native-tls / OS trust store"
      passages are replaced with the two-backend rustls+BoringSSL story, the
      `#[cfg(not(target_os = "android"))]` claim is gone, and the `open`
      footnotes in §4/§12 now say rustls + webpki-roots.
  - [x] Cross-link this doc from
      [`xrds-net-android-shipping.md`](xrds-net-android-shipping.md) (now
      also in `docs/done/`) §2a (native-tls) and §2b (curl), noting the
      OpenSSL/curl swap
      supersedes those sections' original "supported via OpenSSL prebuilt" /
      "supported via curl-sys" conclusions.
- [ ] Move this checklist to `docs/done/` once Phase 4 is fully green.

---

## Decision log

*(Fill in as each phase lands — one line per swap: what changed, verified
how.)*

- Dead sync WebSocket path (`websocket` crate): **done** — removed;
  `StreamHandler` consolidated onto the `tokio-tungstenite`-backed
  `WsSession` that `SessionHandler` already used (one WS backend total).
- suppaftp → rustls: **done** — feature swap only, no code changes.
- tokio-tungstenite → rustls: **done** — feature swap only, no code changes;
  surfaced and fixed a real gap (rustls `CryptoProvider` install was ad hoc)
  and a pre-existing test bug (wrong URL scheme).
- HTTP client (curl → reqwest): **done** — `reqwest` (blocking,
  `rustls-tls-webpki-roots`) replaces `curl`. `PROTOCOLS::FILE` moved to a
  direct `std::fs::read` (reqwest can't fetch `file://`), which is a cleaner
  split anyway. All 19 HTTP tests pass.
- Full Android link (the original duplicate-symbol repro): **done** —
  `xrds-app` links clean, APK builds, and both HTTPS + wss **verified working
  on a Quest 3** (see Phase 4). `openssl-sys`, `curl-sys` and `native-tls`
  are all gone from the dependency graph entirely.
- Net result: **two crypto backends total** — BoringSSL (inside quiche, QUIC
  only) and rustls+ring (everything else). Down from four.
