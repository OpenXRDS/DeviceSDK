# xrds-net — release-readiness plan (internal DeviceSDK milestone)

**Status:** Phases 1, 2, 4, 5, 6 complete and verified. **Phase 3 (a
real-network WebRTC handshake test) is the one open item** — it needs a
human with two real network endpoints; everything else in this crate has
been fixed, hardened, cleaned up, and re-verified from this sandbox. Not
moved to `docs/done/` until Phase 3 is actually run.

## Context

Following the WebRTC ICE-config bug fixes and test-suite reliability work
(`docs/done/xrds-net-webrtc-test-restructure.md`,
`docs/done/xrds-net-webrtc-ice-config-fix.md`), the question came up: is
xrds-net release-ready? Answer at the time: the two concrete bugs found are
genuinely fixed and verified, but "all WebRTC issues fixed" was too strong a
claim — real-network handshakes were never exercised (everything tested was
loopback), and a broader release-readiness pass hadn't been done at all
(security, crash-safety, docs, tooling).

**Scope decision:** this is for an **internal DeviceSDK milestone** release
(not a crates.io publish) — the bar is stability, no crashes under normal
and adversarial-ish input, and docs adequate for internal developers. Not
in scope: full semver/public-API discipline, crates.io metadata polish
beyond the cheap wins, or building CI from scratch unless explicitly
decided below.

A survey (clippy, `.unwrap()`/`panic!` audit, TODO scan, docs skim, Cargo.toml
metadata, CHANGELOG convention check, CI check) was run to ground this plan
in actual findings rather than guesswork. Summarized in Findings below.

## Findings

1. **`cargo clippy -p xrds-net --all-targets`**: 0 errors, 68 warnings.
   Production `src/` (non-test): only 6 warnings, all but one pure
   style/cosmetic (`module_inception` x2, `single_match` x1,
   `unnecessary_lazy_evaluations` x2). The one worth a look:
   `unnecessary_unwrap` at `server/server.rs:286` — checks `is_none()` then
   calls `.unwrap()` instead of matching the `Result` directly; not a bug
   today, but a fragile pattern if the check logic ever changes. The
   remaining 62 warnings are all in test code (`assert_eq!(x, true)` →
   `assert!(x)`, `assert!(false)` placeholders, `len() > 0` →
   `!is_empty()`, etc.) — trivially auto-fixable, cosmetic only.

2. **Signaling-server crash risk (the standout finding):** roughly
   90-100 `.unwrap()`/`.expect()`/`panic!()` calls in production `src/`
   (excluding test modules), concentrated in `server/webrtc_server.rs` (23)
   and `client/xrds_webrtc/webrtc_client.rs` (20). Most are internal
   invariants (mutex/socket state) or fail-fast startup config validation —
   not attacker-reachable. **But several are directly on the path of
   client-supplied WebRTC signaling data:**
   - `server/webrtc_server.rs:297` and `client/.../webrtc_client.rs:368` —
     `serde_json::from_str(msg).unwrap()` parsing an incoming signaling
     message with no error handling. Malformed JSON from either side
     panics that task.
   - `server/webrtc_server.rs:224, 352, 445, 508, 539, 557, 575, 611, 629`
     — `clients.get(&client_id).unwrap()` / `sessions.get(&session_id)
     .unwrap()` / `.get_mut(...).unwrap()` — these IDs come from
     client-supplied signaling requests. A stale, unknown, forged, or
     raced (e.g. client left and something else still references the old
     session) ID currently panics the signaling server instead of
     returning an error to that one client.
   - `server/webrtc_server.rs:361-366` — unwraps on client-controlled SDP
     payload (`request.sdp.clone().unwrap()`) and internal answer
     bookkeeping.

   This is the most release-relevant finding: **one malformed or
   out-of-order message from a single WebRTC client can currently crash
   the signaling task** (and depending on how `run()`'s accept loop and
   per-connection tasks are structured, potentially take other connected
   clients down with it, or at minimum silently drop them). This is a
   real availability/robustness gap for a multi-client signaling server,
   not a style nit.

3. **Session-lifecycle TODOs**, same file/subsystem as #2:
   `server/webrtc_server.rs:266, 513, 543` — `// TODO: remove the WebRTC
   connection too. (not implemented yet)` on leave/close paths. A client
   leaving or a session closing doesn't tear down the associated
   `RTCPeerConnection`/resources server-side tracking — a slow resource
   leak under real usage (repeated join/leave churn), compounding the
   crash risk above (stale references outliving their session).

4. **Hardcoded TURN credentials** (already identified pre-plan):
   `username: "gganjang"`, `password: "keti007"` in
   `crates/xrds-net/src/webrtc_ice_config.rs` — the single shared ICE
   config function (post de-duplication). **Decision: move to config/env
   before release** (per explicit instruction).

5. **`Cargo.toml` metadata**: `xrds-net` declares only
   `name`/`version`/`edition`/`[lib]`/`[features]`/deps — no `description`,
   `license`, `repository`, `authors`, or `homepage`. Inconsistent
   workspace-wide (some crates declare `license`/`description`, most
   don't) — not xrds-net-specific, and low priority for an internal
   release, but cheap to add.

6. **No CHANGELOG** anywhere in the workspace (not just xrds-net) — no
   existing convention to follow. Out of scope to invent workspace-wide
   here; decide explicitly whether this release gets one anyway.

7. **Docs (README/MANUAL.md/MANUAL_WEBRTC.md)**: all three are current and
   reasonably complete. MANUAL.md §16 already accurately documents the
   known-flaky public-server tests and the WebRTC test restructuring —
   nothing to fix there. **Gap:** MANUAL_WEBRTC.md has zero mention of TURN
   credentials or how ICE config is sourced — needs a docs update tied to
   item #4 once credentials move to config/env.

8. **Test baseline**: `cargo test -p xrds-net --lib` → 150 passed, 2 failed
   (`test_client_mqtt_sub_pub_rcv`, `test_ws_send` — both known-flaky
   public-server tests, matches the documented baseline). No new/unexpected
   failures. WebRTC integration suite (from prior work): 16/16 clean across
   3 consecutive runs on loopback.

9. **No CI exists anywhere in the repo** — no `.github/workflows`, no
   `.gitlab-ci.yml`, nothing. Every test run to date has been manual. This
   is a real gap for a "release-ready" milestone (nothing catches a
   regression automatically) but a nontrivial scope decision (needs a
   flaky-test allowance designed in from the start) — decide explicitly
   whether it's in scope for this milestone.

10. **Never verified: real-network WebRTC handshakes.** Everything tested
    this session was `127.0.0.1↔127.0.0.1` loopback. The TURN-relay path,
    restrictive-NAT scenarios, and actual reachability of
    `turn.keti.xrds.kr`/Google STUN from a real deployment target have not
    been exercised at all. The `turns:` fix should help there (same shared
    config now used in both places), but this is unverified, not confirmed
    fixed.

## Goals

1. Close the signaling-server crash risk (finding #2) — the one item that
   is a genuine availability bug, not polish.
2. Address the session-lifecycle resource-leak TODOs (finding #3) while in
   the same code.
3. Remove hardcoded TURN credentials from source (finding #4), per
   explicit decision.
4. Get at least one real-network WebRTC handshake verified (finding #10) —
   can't ship "release-ready" on loopback-only evidence.
5. Quick, low-risk wins: clippy cleanup, Cargo.toml metadata.
6. Make an explicit, recorded decision on CI and CHANGELOG scope rather
   than silently doing or skipping either.

## Non-goals

- crates.io-grade public API/semver discipline — this is an internal
  milestone.
- Implementing the SFU mode `WebRTCServer`'s dead `rtc_config`/`api` fields
  are reserved for — genuinely out of scope, not a release blocker.
- Fixing the *other* protocols' known-flaky public-server tests
  (mosquitto/websocket.org/rebex.net/coap.me) — already documented as
  accepted baseline noise; revisit separately per earlier decision.
- Building a full CI pipeline from scratch, unless Phase 5 decides to.
- Introducing a workspace-wide CHANGELOG convention, unless Phase 5
  decides to.

---

## Phase 1 — remove hardcoded TURN credentials

- [x] Mechanism chosen: environment variables (`XRDS_TURN_USERNAME`,
      `XRDS_TURN_PASSWORD`), read once per `build_ice_servers()` call — kept
      separate from `WebRTCClient::set_ice_servers()`, which replaces the
      *whole* list (including STUN) and stays as the loopback/test-only
      override it already was.
- [x] Implemented: `webrtc_ice_config.rs` now has
      `turn_credentials_from_env()` (reads the two env vars, `None` if
      either is unset) and `build_ice_servers_with(Option<TurnCredentials>)`
      (the testable pure core — takes credentials as a parameter instead of
      touching the environment, so tests don't race on process-global env
      state). `build_ice_servers()` is now a thin wrapper: reads env, logs
      a `warn!` and proceeds STUN-only if unset, otherwise includes the
      TURN entry with the supplied credentials. No hardcoded
      username/password remain in source.
- [x] Docs updated: `MANUAL_WEBRTC.md` §3 gained a new item documenting the
      two env vars, the STUN-only fallback behavior, and a pointer to
      `WebRTCClient::set_ice_servers(vec![])` for the loopback-testing case
      instead. (`docs/done/xrds-net-webrtc-ice-config-fix.md` never
      mentioned the literal credential strings, so nothing to update
      there.) Also fixed a stale snippet in the same `MANUAL_WEBRTC.md`
      section while there — it still showed the raw, unguarded
      `CryptoProvider::install_default(...)` call instead of
      `ensure_rustls_crypto_provider()` (the guarded/idempotent version
      established earlier this session).
- [x] Verified: 3 unit tests in `webrtc_ice_config::tests` — the existing
      `turns:`-scheme test now runs against `build_ice_servers_with(Some(..))`
      explicitly, plus two new ones:
      `turn_credentials_are_carried_through_to_the_turn_entry` and
      `no_credentials_omits_the_turn_entry_but_keeps_stun`. All pass, no
      real credentials or env mutation needed.

## Phase 2 — signaling-server hardening (crash risk + resource leaks)

- [x] Fixed the client-message-parsing `.unwrap()`s: `server/webrtc_server.rs`'s
      `signaling_handler` and `client/.../webrtc_client.rs`'s read loop both
      now `match` the `serde_json::from_str` result — malformed JSON from a
      peer logs a `warn!` and is dropped (server: returns `None`, sends
      nothing back; client: `continue`s to the next message), never panics.
- [x] Replaced every `clients.get(...).unwrap()` /
      `sessions.get(...).unwrap()` / `.get_mut(...).unwrap()` in
      `handle_answer`, `handle_offer`, `join_session`, `leave_session`,
      `list_participants`, `handle_ice_candidate`, and
      `handle_ice_candidate_ack` with `let Some(..) = ... else { return
      Self::error_response(...) }` — a new helper that builds a
      `WebRTCMessage` with `error: Some(...)` instead of panicking on an
      unknown/stale/forged session or client id. Also fixed
      `handle_answer`/`handle_offer` unwrapping a possibly-`None` `sdp`
      field the same way. `handle_ice_candidate_ack`'s publisher-sender
      lookup (a *client* id, not a session id) gets the same treatment —
      the publisher may have disconnected mid-flight; that's now logged and
      dropped, not a panic.
- [x] **Found and fixed a bigger version of the leak than the survey
      flagged**: disconnect cleanup didn't just have unimplemented TODOs —
      it only ran at all on an *explicit* WS close frame. A read error or
      the stream just ending (network blip, crashed client, no close
      frame) skipped cleanup entirely, on every code path that returned
      early (including two paths that returned *before* the read loop even
      started). Restructured `handle_connection` into a thin outer
      function that always runs cleanup after an inner function returns,
      regardless of *which* path it took. New `handle_client_disconnect`
      replaces the old sync `remove_client` (which used
      `Mutex::blocking_lock()` — itself a latent panic risk, since that
      method panics if called from within an async task on a multithreaded
      runtime, which this always was) and implements the "remove the
      session if the client is the creator" TODO plus general
      participant-list cleanup for sessions the client only joined.
      (Fixing this surfaced a `Send` requirement issue — `Box<dyn Error>`
      held across the cleanup `.await` isn't `Send`, breaking
      `tokio::spawn`'s bound on the outer per-connection task; switched the
      inner function's error type to `String`, converted back to `Box<dyn
      Error>` only at the outermost boundary.)
- [x] Also removed `Session::publisher_ice_candidates` — a dead field found
      while fixing `handle_ice_candidate`: it was written to a throwaway
      `.clone()` of the session that was never persisted back to the map,
      and nothing anywhere read it. Confirmed via a full-file grep before
      removing.
- [x] Added 8 new unit tests to `webrtc_server::tests` (same pattern as
      existing ones — direct calls against a bare `WebRTCServer::new()`, no
      network): unknown-session-id error responses for
      `join_session`/`leave_session`/`list_participants`/`handle_offer`/
      `handle_answer` (5), confirming `close_session` was already a safe
      no-op on an unknown id (1, documents existing safety rather than a
      fix), and disconnect cleanup actually removing a creator's session
      and a joiner's participant entry (2).
- [x] Re-ran `tests/webrtc_integration.rs` (the real end-to-end suite) — 16
      passed, 0 failed, 1 ignored, ~64s, matching the pre-hardening
      baseline exactly. Also re-ran the `#[ignore]`d heavy test standalone
      — still passes. `cargo test -p xrds-net --lib` → 161 passed, 1 failed
      (`test_ws_connect` this run — the same known public-server
      flakiness, a different specific test each run).

## Phase 3 — real-network verification

- [x] **Designed and tooled.** Three standalone binaries now exist purpose-built
      for this — `webrtc_realnet_signaling_server`, `webrtc_realnet_publisher`,
      `webrtc_realnet_subscriber` (all in `examples/webrtc/`; see
      `docs/done/xrds-net-webrtc-realnet-binaries.md` for the design). They
      replace the earlier plan of hand-modifying a copy of
      `webrtc_file_stream.rs` — these are real, reusable programs: each is
      independently runnable, uses the **default** production STUN/TURN ICE
      config (no loopback override), and prints the ICE connection state
      plus the winning candidate pair type (`host`/`srflx`/`relay`) so a
      human can read the actual outcome off the terminal. Procedure:

      1. **Machine A (signaling server):**
         ```bash
         cargo run --example webrtc_realnet_signaling_server -- --port 9443
         ```
         Note the printed port and Machine A's LAN/public IP.
      2. **Machine A (publisher, second terminal):** TURN credentials via
         env vars or `--turn-username`/`--turn-password` — either works,
         the flags just set the same env vars for you:
         ```bash
         export XRDS_TURN_USERNAME=<real turn username>
         export XRDS_TURN_PASSWORD=<real turn password>
         cargo run --example webrtc_realnet_publisher -- \
           --signaling-addr ws://<machine-a-ip>:9443/
         ```
         Copy the session id it prints (in a bordered banner — hard to
         miss) to Machine B.
      3. **Machine B (subscriber):**
         ```bash
         cargo run --example webrtc_realnet_subscriber -- \
           --signaling-addr ws://<machine-a-ip>:9443/ \
           --session-id <id from step 2> \
           --turn-username <real turn username> --turn-password <real turn password>
         ```
      4. **Read the result directly off both terminals** — each side prints
         `ICE connected: <state>` and `Active candidate pair: local=<type>
         remote=<type>` once connected. A `relay` type on either side is
         the strongest evidence the TURN path specifically works, not just
         direct/STUN. The subscriber also reports the received file's path
         and size at the end.
      5. For a stronger test of the TURN relay path specifically (not just
         "does *some* path work"), force it — e.g. run one side from a
         symmetric-NAT network (some mobile carriers) so ICE can't fall
         back to a direct/STUN candidate — or add an
         `ice_transport_policy: RTCIceTransportPolicy::Relay` variant if
         needed later.
- [x] **Local dry run passed** (this *doesn't* count as Phase 3 itself —
      loopback still can't exercise a real TURN relay — but de-risks the
      binaries before taking them to an actual two-machine setup, per
      `docs/done/xrds-net-webrtc-realnet-binaries.md` Phase 3). Ran all three as
      separate background processes on one machine: session id printed and
      handed off correctly, both sides reached `ICE connected: Connected`
      with `Active candidate pair: local=Host remote=...` (real local
      network interfaces, STUN servers genuinely reached and returned
      `srflx` candidates with a real public IP — this sandbox does have
      outbound network access, contrary to earlier assumptions), the data
      channel message round-tripped, and the subscriber reported a
      14,154,318-byte received file with a clean teardown on both ends.
- [ ] **Not yet run on two real machines.** This is still the genuine gap —
      the local dry run proves the binaries work, not that the real
      cross-machine/TURN-relay path works. Someone with two real network
      endpoints needs to execute the procedure above and record the
      outcome here (pass/fail; if fail, which `RTCIceConnectionState` it
      got stuck at, and which candidate types were reachable).
- [ ] If it fails: this becomes its own investigation, not something to
      guess-fix from the loopback-only evidence gathered so far.

## Phase 4 — code quality pass

- [x] Fixed `server/server.rs`'s `unnecessary_unwrap` — and, on closer look,
      it was worse than clippy's warning implied: the line right before the
      flagged one (`validate_path(self.root_dir.as_ref().unwrap())`)
      unwrapped `root_dir` *unconditionally*, before the `is_none()` check
      that was supposed to guard exactly that case. Rewrote as a single
      `match self.root_dir.as_deref() { Some(dir) if validate_path(dir).is_ok()
      => ..., _ => ... }` — no unwraps at all, and no longer panics if
      `root_dir` is ever actually `None`.
- [x] Fixed the two `unnecessary_lazy_evaluations` in `server.rs`
      (`.unwrap_or_else(|_| configured_port as u16)` → `.unwrap_or(configured_port
      as u16)`, `start_dynamic`'s port-reporting fallback).
- [x] `module_inception`: two *different* underlying cases, handled
      differently. `client::client`/`server::server` (the crate's real
      internal layout, referenced throughout) got
      `#[allow(clippy::module_inception)]` with a one-line reason.
      `client::tests::tests`/`server::tests::tests`/`common::tests::tests`
      (a file already named `tests` via its `mod tests;` declaration,
      wrapping its own content in another `mod tests { }`) is genuinely
      redundant nesting, not a naming choice — but flattening it means
      re-indenting 3 large files for a purely cosmetic path change with no
      behavior difference, so it got the same `#[allow]` treatment instead
      of a bigger mechanical diff.
- [x] Ran `cargo clippy --fix` for the auto-fixable subset (34 + 4
      suggestions), then manually fixed everything left over: the
      `Some(...)`-then-`.unwrap()` root-dir pattern (4 sites, 3 files),
      `assert!(true)`/`assert!(false)` placeholders (5, one replaced with a
      real `panic!(error)` since it was guarding a real error case),
      `assert_eq!(x, true)` → `assert!(x)` (already covered by
      `--fix`), `is_ok()`/`is_err()`-then-`.unwrap()` chains (4, converted
      to `if let`/`match`), and one `empty_line_after_doc_comments`.
      **Result: `cargo clippy -p xrds-net --all-targets` → 0 warnings**
      (down from 68).
- [x] Added `license = "Apache-2.0"` (matching `xrds-scene-graph`/
      `xrds-media`), a one-line `description`, and `repository` (matching
      the workspace root `Cargo.toml`'s) to `xrds-net/Cargo.toml`. Skipped
      `authors`/`homepage` — no established per-crate convention for those.
- [x] Verified: `cargo test -p xrds-net --lib` (162 passed, 2 known-flaky
      failures — different pair each run, as always), full
      `tests/webrtc_integration.rs` (16/16), `cargo check --workspace`
      clean. No test was lost or changed in meaning — every rewrite kept
      identical assertions/control flow, just idiomatic instead of
      redundant.

## Phase 5 — explicit scope decisions (CI, CHANGELOG)

- [x] **CI decision: yes, add it.** Repo is hosted on GitHub (not GitLab —
      corrected mid-investigation), so this is a GitHub Actions workflow,
      not `.gitlab-ci.yml`. Draft below; added as a real file — see the
      checklist under the script.
- [x] **CHANGELOG decision: no, not for this release.** No crate in the
      workspace has one; see the decision log for the reasoning.

### CI script draft (GitHub Actions)

Not yet added as a live workflow file — this is a ready-to-use draft;
adding it for real is a remaining action item (see checklist below the
script). Save as `.github/workflows/xrds-net-ci.yml`.

Design notes:

- **Runner/toolchain:** `ubuntu-latest`, pinned to the toolchain this repo
  actually builds with (`rustc 1.95.0` observed locally) via
  `dtolnay/rust-toolchain`. Update the pin if the repo's actual MSRV/CI
  toolchain policy differs — there's no `rust-toolchain.toml` in the repo
  today to read this from automatically.
- **Native build deps:** `xrds-net` depends on `quiche` (pinned
  `=0.24.6`), which vendors BoringSSL and needs `cmake` + `perl` + a C
  toolchain to build from source on a fresh runner (`build-essential` is
  present on `ubuntu-latest` but `cmake`/`perl` are installed explicitly
  below to be safe). This is a Linux x86_64 **host** build only — the
  workspace's `[patch.crates-io]` quiche fix
  (`patches/quiche`) addresses an *Android cross-compile* quirk
  (CMake generator detection under an MSVC host) that doesn't apply here,
  so no extra generator flags are needed for this job.
- **Caching:** `Swatinem/rust-cache` (keys off `Cargo.lock` automatically,
  handles the `target/` + registry cache invalidation correctly across
  branches) rather than hand-rolling `actions/cache` key logic.
- **Jobs split:** `check` (whole workspace, fast fail), `clippy` (xrds-net
  only, `-D warnings` — matches the near-zero-warning baseline from the
  Phase 4 survey, so this should stay clean going forward rather than
  slowly accumulating warnings), `test-unit` (xrds-net lib tests),
  `test-webrtc-integration` (the `tests/webrtc_integration.rs` suite,
  separate job — slower, and serialized internally via `#[serial(webrtc)]`
  so `--test-threads=1` is required, same as every manual run this session).
- **The flaky-test problem, unresolved:** `test-unit` is marked
  `continue-on-error: true` — a coarse, honest stopgap, **not a real
  fix**. The known-flaky tests (`test_ws_send`, `test_client_mqtt_sub_pub_rcv`,
  and others hitting live public servers per MANUAL.md §16) aren't tagged
  in code today the way the WebRTC suite's slow test is
  (`#[ignore]`) — there's no clean `cargo test -- --skip ...` list that
  wouldn't silently miss newly-added network-dependent tests. Making the
  whole job non-blocking means a **genuine** regression in `test-unit`
  also goes unnoticed by CI, which defeats much of the point of having it.
  **Recommended follow-up, not done here:** tag every test in
  `client/tests.rs`/`server/tests.rs` that hits a real public
  server/broker with `#[ignore = "hits a live public server; see MANUAL.md §16"]`,
  mirroring the precedent already established for the WebRTC 120s test —
  then `test-unit` can run the default (non-ignored) set as a real
  blocking gate, and a separate `test-network` job runs `-- --ignored`
  as non-blocking/informational. This crate's `--lib` suite has
  essentially the same "integration tests disguised as unit tests"
  problem that motivated the WebRTC test restructure
  (`docs/done/xrds-net-webrtc-test-restructure.md`) — just less acute,
  since it isn't hanging or crash-flaky, only network-flaky. Worth its own
  small pass at some point, not bundled into this release.
- **The heavy WebRTC test:** already `#[ignore]`d in code (per
  `docs/done/xrds-net-webrtc-test-restructure.md` Phase 5), so it's
  correctly excluded from `test-webrtc-integration`'s default run without
  any extra CI-side filtering.

```yaml
name: xrds-net CI

on:
  push:
    branches: [main]
  pull_request:
    paths:
      - "crates/xrds-net/**"
      - "Cargo.toml"
      - "Cargo.lock"
      - ".github/workflows/xrds-net-ci.yml"

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: cargo check (workspace)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install native build deps (quiche/BoringSSL)
        run: sudo apt-get update && sudo apt-get install -y cmake perl
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.95.0" # keep in sync with the repo's actual toolchain
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --workspace --tests

  clippy:
    name: clippy (xrds-net)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install native build deps (quiche/BoringSSL)
        run: sudo apt-get update && sudo apt-get install -y cmake perl
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.95.0"
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy -p xrds-net --all-targets -- -D warnings

  test-unit:
    name: test (xrds-net --lib)
    runs-on: ubuntu-latest
    # Stopgap, not a fix: a large fraction of this suite hits live public
    # servers (mosquitto/rebex/coap.me/echo.websocket.org) and isn't tagged
    # #[ignore] the way the WebRTC slow test is, so it can't be cleanly
    # split into blocking/non-blocking today. See the design notes above —
    # tagging those tests is the real fix, tracked as a follow-up.
    continue-on-error: true
    steps:
      - uses: actions/checkout@v4
      - name: Install native build deps (quiche/BoringSSL)
        run: sudo apt-get update && sudo apt-get install -y cmake perl
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.95.0"
      - uses: Swatinem/rust-cache@v2
      - run: cargo test -p xrds-net --lib

  test-webrtc-integration:
    name: test (xrds-net webrtc_integration)
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4
      - name: Install native build deps (quiche/BoringSSL)
        run: sudo apt-get update && sudo apt-get install -y cmake perl
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.95.0"
      - uses: Swatinem/rust-cache@v2
      # `#[serial(webrtc)]`-serialized within the file — must run
      # single-threaded, same as every manual run this session.
      - run: cargo test -p xrds-net --test webrtc_integration -- --test-threads=1
```

- [x] Added the real file at `.github/workflows/xrds-net-ci.yml` (identical
      to the draft above). Validated with `python -c "import yaml;
      yaml.safe_load(...)"` — syntactically valid YAML. `actionlint` isn't
      available in this sandbox, so deeper schema/expression validation
      wasn't possible here.
- [ ] **Not yet run for real.** This sandbox can't execute GitHub Actions —
      the first real run happens on the first push/PR that touches
      `crates/xrds-net/**` after this lands. Treat that first run as the
      actual validation of this workflow, not this checkbox.
- [ ] Decide whether to act on the flaky-test-tagging follow-up
      (`#[ignore]`-tagging the public-server-dependent unit tests, per the
      design notes above) now or track it separately from this release.

## Phase 6 — final verification

- [x] `cargo check --workspace` clean.
- [x] `cargo clippy -p xrds-net --all-targets` → 0 warnings.
- [x] `cargo test -p xrds-net --lib` → 160-162 passed (fluctuates slightly
      with which flaky tests happen to be included/excluded in a given
      run), 1-2 known-flaky public-server failures each time (a different
      specific test each run — `test_ws_send`, `test_ws_rcv`,
      `test_ws_connect`, `dispatch_ws_reaches_a_real_server`,
      `test_client_mqtt_sub_pub_rcv` all observed at different points this
      session), never a new/unexplained failure.
- [x] `cargo test -p xrds-net --test webrtc_integration -- --test-threads=1`
      → 16/16 passed, 1 ignored, 3 consecutive runs, ~64s each — no
      regression from Phase 2's hardening or Phase 4's cleanup.
- [ ] **Real-network handshake (Phase 3): not recorded, because not run.**
      This is the one item in this entire plan that could not be completed
      from this sandbox — it requires a human with two real network
      endpoints. Everything else in this document is genuinely done and
      verified; this one is a documented runbook waiting for someone to
      execute it, not a "check the box anyway" situation.
- [x] Docs updated: `MANUAL_WEBRTC.md` (TURN config + fixed the stale raw
      `CryptoProvider::install_default` snippet), `examples/README.md` (new
      `webrtc_file_stream.rs` example from earlier in this work), this
      document itself (every phase's findings-vs-fixed status, updated as
      each phase landed rather than only at the end).
- [ ] **Not moving this checklist to `docs/done/` yet** — Phase 3's
      execution is a real, unresolved gap, not a formality. Move it once
      someone runs the Phase 3 runbook and records a pass (or resolves a
      failure it surfaces). Everything else in Phases 1-2, 4-6 is complete
      and verified as of this pass.

---

## Decision log

Fill in as each phase lands.

- TURN credential sourcing mechanism (Phase 1): done — env vars
  (`XRDS_TURN_USERNAME`/`XRDS_TURN_PASSWORD`), STUN-only fallback (+
  `warn!` log) when unset, rather than failing hard. Chose env over a
  config struct threaded through constructors since it's the simplest
  change that removes the committed secret without touching every
  `WebRTCClient`/`WebRTCServer` call site's signature.
- Signaling-server error-handling approach (Phase 2): done — a shared
  `error_response()` helper returning `WebRTCMessage { error: Some(..), ..
  }` in place of every unwrap-on-client-data panic, plus a restructured
  `handle_connection` that guarantees disconnect cleanup runs on every exit
  path (not just an explicit close frame — the bigger gap actually found).
  Chose to keep the *response-to-that-one-client* error model already
  established by the message format (`error: Option<String>`) rather than
  inventing a new out-of-band error channel.
- Real-network verification result (Phase 3): **still pending on two real
  machines — genuinely blocking.** Purpose-built binaries now exist
  (`webrtc_realnet_signaling_server`/`_publisher`/`_subscriber`, see
  `docs/done/xrds-net-webrtc-realnet-binaries.md`) and passed a local loopback
  dry run (session hand-off, ICE connect, candidate-pair-type reporting,
  streaming, data channel, teardown all verified working). What's still
  missing is running them on an actual two-machine/real-NAT setup —
  loopback can prove the tooling works, not that TURN relay specifically
  works over a real network. Every other phase in this document is
  complete; this is the one thing standing between "the known bugs are
  fixed" and "this milestone is release-ready."
- CI scope decision (Phase 5): decided — yes, GitHub Actions (repo is on
  GitHub, not GitLab). Added as a real file at
  `.github/workflows/xrds-net-ci.yml`. `test-unit` is intentionally
  `continue-on-error: true` as an honest stopgap, not a real fix — the
  known-flaky public-server tests aren't tagged `#[ignore]` the way the
  WebRTC slow test is, so they can't be cleanly split from genuine
  regressions yet. Not yet validated by an actual push/PR run (can't
  execute GitHub Actions from this sandbox) — that first real run is the
  true test of this workflow, not anything checked off in this doc.
- CHANGELOG scope decision (Phase 5): decided — no, not for this release.
  No crate in the workspace has one; adding it just for `xrds-net` would
  introduce an inconsistent, one-off convention rather than a real
  workspace decision. Revisit as its own cross-crate decision if wanted
  later, not bundled into this milestone.
