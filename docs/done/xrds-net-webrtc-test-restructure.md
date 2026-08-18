# xrds-net — WebRTC test restructure: integration vs. unit tests

## Context

While chasing WebRTC test flakiness (`docs/done/xrds-net-crypto-consolidation.md`
and the logger/crypto-provider/TURN-scheme fixes that followed it), it became
clear the underlying test suite has a design problem, not just a few isolated
bugs: **every "WebRTC test" is a full end-to-end integration test — real
signaling server, real ICE/STUN/TURN/DTLS handshake, real mDNS socket —
labeled and run as if it were a fast, isolated unit test.** The bugs already
fixed (unguarded global-singleton inits, port collisions, resource
contention under parallel execution) are symptoms of that gap, not the gap
itself.

Concretely, today's `#[serial(webrtc)]` fix works, but it's a workaround: 17
tests now share one global lock to avoid colliding on the same OS resources
(mDNS UDP 5353, signaling server ports), instead of each test getting its own
isolated process the way Cargo's `tests/` directory provides for free.

## Findings (see the review that prompted this plan)

- `crates/xrds-net/src/client/tests.rs` and `src/server/tests.rs` contain
  ~17 WebRTC tests, all full E2E, none actually unit-scoped.
- Sleep-based synchronization throughout (`sleep(Duration::from_secs(2/10/120))`)
  instead of polling for actual state.
- Port allocation via `line!() + 8000` — derives a port from the *source
  line number* of the call site; fragile to edits, no real collision
  guarantee.
- Weak/absent assertions in several server-side tests (`test_server_webrtc_run`,
  `test_server_webrtc_connect_signal`, `test_server_webrtc_multiuser`) — they
  `println!` and `assert!(true)`, so a silent regression wouldn't fail them.
- **Real bug found while reviewing:** `test_server_webrtc_run`
  ([server/tests.rs:473](../crates/xrds-net/src/server/tests.rs#L473)) does
  `server_handle.await.unwrap()` instead of `.abort()` like every sibling
  test — `XRNetServer::run()`'s accept loop never terminates on its own, so
  this test doesn't actually finish by design; it only "passes" via an
  external test-runner timeout.
- Pure logic (ICE server URL construction, signaling message
  serialization, session create/join/leave/list bookkeeping) is currently
  only exercised indirectly, through full network round-trips — it's
  trivially unit-testable in isolation and isn't.

## Goals

1. Structurally isolate integration tests from each other (separate
   processes, not a shared global lock) by moving them to Cargo's `tests/`
   convention.
2. Add real, fast, mock-free unit tests for the logic that doesn't need a
   live network at all.
3. Remove the specific fragilities (sleep-based sync, `line!()` ports, weak
   assertions) along the way — not just relocate the same flaky tests.
4. Don't lose coverage — every current integration scenario should still be
   exercised somewhere after this, just via a better mechanism.

## Non-goals

- Rewriting `WebRTCClient`/`WebRTCServer`'s actual runtime behavior. This is
  a test-suite restructure, not a behavior change.
- Achieving 100% unit-test coverage of the WebRTC stack. Some things
  (ICE negotiation, DTLS handshake) are inherently only verifiable
  end-to-end — the goal is pulling out what *can* be unit-tested, not
  eliminating integration tests entirely.

---

## Phase 0 — quick, safe fixes (do first, independent of the restructure)

Small, mechanical fixes that are correct regardless of where the tests end
up living. Low risk, do these before the bigger structural move so the
restructure starts from a correct baseline.

- [x] Fixed `test_server_webrtc_run`: `server_handle.await.unwrap()` →
      `server_handle.abort()`, matching every sibling test. It was
      genuinely hanging on the accept loop before; now finishes promptly.
- [x] Strengthened `test_server_webrtc_connect_signal` and
      `test_server_webrtc_multiuser`: assert the received `client_id`(s)
      parse as well-formed UUIDs (`uuid::Uuid::parse_str`), and that two
      concurrently-connected clients get *distinct* ids — rather than only
      `println!`-ing them.
- [x] Audited every "just println" pattern in the WebRTC test set. Most
      already had real assertions (`session_join`, `session_leave`,
      `session_list_participants`, `offer`, `answer`) — false positives from
      a first grep pass. Four were genuinely weak and fixed:
      - `test_server_webrtc_session_create` — now asserts the returned
        `session_id` is a well-formed UUID (was: unchecked).
      - `test_server_webrtc_session_list` — now asserts the created session
        actually appears in `list_sessions()` (was: printed and ignored).
      - `test_server_webrtc_session_multiple` — now asserts exactly 2
        sessions are listed after creating 2, and both ids are present (was:
        printed and ignored).
      - `test_server_webrtc_session_close` — now asserts the session is
        present before `close_session()` and **gone** after (was: printed
        both lists without ever comparing them).
      Verified: all 12 server-side WebRTC tests pass with the new
      assertions.

## Phase 1 — move to `tests/` (the structural fix)

Cargo runs each file under `tests/` as its own separate test binary /
process, which isolates this suite's shared OS resources from *other* test
binaries (the unit-test binary, any other `tests/*.rs` file). It does
**not** isolate the `#[test]`/`#[tokio::test]` functions *within* one
`tests/*.rs` file from each other — those still run concurrently in one
shared process by default, same as today. Corrected from the original plan:
`#[serial(webrtc)]` is still needed *within* `webrtc_integration.rs`, not
eliminated by the move — the move's win is isolating this file's mDNS/port
usage from the unrelated unit-test binary, not removing in-file
serialization.

- [x] Created `crates/xrds-net/tests/webrtc_integration.rs`.
- [x] Moved all 17 WebRTC tests (5 client-side, 12 server-side) there, along
      with their shared helpers (`establish_complete_webrtc_connection`,
      `establish_webrtc_with_custom_subscriber`, `run_server`,
      `is_valid_h264`, `init_crypto`, `init_logger`,
      `CustomVideoProcessor`, `custom_audio_handler`,
      `DEFAULT_DEBUG_FILE_PATH`), rewritten against the crate's public API
      (`xrds_net::{WebRTCClient, VideoSource, PROTOCOLS}`,
      `xrds_net::client::media::VideoTrackHandler`,
      `xrds_net::server::XRNetServer`,
      `xrds_net::common::{append_to_path, payload_str_to_vector_str,
      ensure_rustls_crypto_provider}`) since `tests/*.rs` files link the
      crate as an external dependency and can't see `crate::`-private items.
      `#[serial(webrtc)]` kept on every test (see correction above).
- [x] Removed the moved code from `client/tests.rs` and `server/tests.rs`,
      including now-unused imports/statics (`DEFAULT_DEBUG_FILE_PATH`,
      `is_valid_h264`, the WebRTC-only `run_server`/`init_crypto`/
      `init_logger` copies, `WebRTCClient`/`serial_test::serial` imports in
      `server/tests.rs`). `HTTP_ECHO_SERVER_URL` stayed in `client/tests.rs`
      — used by unrelated HTTP tests.
- [x] Confirmed the *other* `#[serial]` group (HTTP3 rate-limiting,
      `ensure_http3_test_spacing`) is unaffected — untouched, still in
      `client/tests.rs`.
- [x] Verified: `cargo test -p xrds-net --lib` → 128 passed, 1 failed
      (`test_ws_send`, pre-existing `echo.websocket.org` 429 rate-limit
      flakiness, unrelated to this change; zero WebRTC tests remain in this
      binary). `cargo test -p xrds-net --test webrtc_integration --
      --test-threads=1` → 17 passed, 0 failed.
- [ ] Update any doc referencing `cargo test -p xrds-net -- --nocapture`
      as "the" test command (e.g. `crates/xrds-net/README.md`) to mention
      the integration tests need `--test webrtc_integration` (or `--tests`
      for everything) explicitly if that command doesn't already cover them.

## Phase 2 — fix port allocation

Replace `line!() + 8000` with binding to port 0 and reading back the
OS-assigned port, everywhere it's used for these tests specifically (other
non-WebRTC tests using the same pattern are out of scope here — flag them
separately if this pattern turns out to be widespread).

- [x] Added the API: `WebSocketServer::run`/`WebRTCServer::run` now delegate
      to a new `run_reporting_port(port, Option<oneshot::Sender<u16>>)` that
      sends back the actual bound port (`listener.local_addr()?.port()`)
      right after `TcpListener::bind` succeeds, before entering the accept
      loop. `XRNetServer` gained `start_dynamic(&self) -> Vec<u32>`, mirroring
      `start()`'s per-protocol dispatch, but for `WS`/`WSS`/`WEBRTC` it wires
      one of these channels and awaits the real bound port before returning;
      other protocols echo back their configured port unchanged (out of
      scope here — `start()` itself is untouched, so no existing caller is
      affected).
- [x] Updated `tests/webrtc_integration.rs`'s `run_server` to call
      `XRNetServer::new(vec![protocol], vec![0]).start_dynamic().await` (port
      `0` = OS-assigned) and return `(JoinHandle<()>, u32)` — the actual
      port. Updated `establish_complete_webrtc_connection`,
      `establish_webrtc_with_custom_subscriber`, and all 17
      call sites to use the returned port instead of `line!() + 8000`.
      Also dropped the `sleep(Duration::from_secs(2))` that used to follow
      every `run_server` call — `start_dynamic` awaiting the real bind makes
      it provably redundant (the OS queues incoming connections once
      `bind()` succeeds, before `accept()` is ever called, so a client can
      connect immediately). The *other* sleeps (ICE-connected wait, stream
      duration) are untouched — that's Phase 3's job.
- [x] Confirmed no test hardcodes a specific port. Verified: full
      `cargo test -p xrds-net --test webrtc_integration -- --test-threads=1`
      → 17 passed on a clean run (an earlier run hit 2 failures from
      pre-existing STUN/TURN resolution flakiness in this sandbox's network,
      confirmed unrelated by re-running those 2 tests alone — both passed).

## Phase 3 — replace sleep-based synchronization with polling

- [x] "Wait until the server is accepting connections" — solved outright in
      Phase 2 rather than needing a poll: `start_dynamic` awaits the real
      `TcpListener::bind` before returning, so the blanket
      `sleep(Duration::from_secs(2))` after every `run_server` call was
      deleted with nothing needed in its place.
- [x] "Wait until ICE is connected" — `WebRTCClient` did *not* expose ICE
      state publicly (only an internal `wait_for_ice_connection` used by
      `start_stream`, capped at a hardcoded 10s). Added
      `WebRTCClient::ice_connection_state() -> Option<RTCIceConnectionState>`
      (additive accessor) and a `wait_for_ice_connected(client, max_wait)`
      test helper polling it every 250ms, called at the end of both
      `establish_*` helpers with a 45s bound. This stopped the tests from
      racing `start_stream`'s internal 10s window — under back-to-back load
      ICE here routinely needs longer, because the configured STUN/TURN
      hosts are unreachable from this sandbox and it falls back to
      host/mDNS candidates.
- [x] `test_client_webrtc_send_video_file`'s flat 120s sleep →
      `wait_for_file_to_stabilize(path, max_wait)`: polls the subscriber's
      debug video file size every 2s and returns once it's unchanged and
      non-zero for 3 consecutive reads, with a 150s bound. Happy-path
      runtime dropped from a fixed 120s to ~78s, and a stalled transfer now
      fails with a clear message instead of silently asserting against a
      partial file.
- [x] Added deterministic teardown: `WebRTCClient::close_peer_connection()`
      (new; `Drop` can't do it, since the cleanup is async) called at the
      end of every test that opens a peer connection. Without it, ICE agents
      held the shared mDNS multicast socket (UDP 5353) past the test's
      lifetime and the *next* test's ICE reproducibly failed — with the
      inter-test sleep gone (Phase 2), `test_client_webrtc_datachannel`
      failed on every run until this was added, then passed.
- [ ] **Not fully achieved: reliability.** Best observed result is a clean
      17/17 run, but repeat runs still intermittently fail 1-3 of the five
      *client-side* tests with `ICE reached terminal state Failed` during
      connection setup. This is environmental, not a regression — the same
      flakiness predates this restructure (the first full run of the moved
      suite, before any Phase 2/3 change, also failed 2). The sandbox has
      ~10 virtual/link-local adapters ICE tries and fails to bind, no IPv6
      route, and unreachable STUN/TURN, so candidate pairing is genuinely
      unreliable here. What Phase 3 *did* buy: the failure is now an
      explicit, named terminal state at a known point rather than an opaque
      10s timeout or a silent pass over a partial transfer. Recommend
      confirming the true pass rate on a normal network before treating
      the remaining flakiness as a code problem.

## Phase 4 — real unit tests for pure logic (new coverage, not a move)

These don't exist today in any form — they're net-new tests for logic
that's currently only exercised indirectly through full E2E round-trips.
Live in `crates/xrds-net/src/server/tests.rs` (or a new
`webrtc_server::tests` submodule) as plain `#[test]` — no `#[tokio::test]`,
no network, no sleep.

- [x] **ICE server / `RTCConfiguration` construction.** Extracted
      `fn build_ice_servers() -> Vec<RTCIceServer>` out of
      [`webrtc_server.rs`'s `setup_webrtc`](../crates/xrds-net/src/server/webrtc_server.rs)
      and added `build_ice_servers_uses_turns_scheme_for_the_tls_secured_port`
      (in the same file's new `#[cfg(test)] mod tests`) asserting STUN
      entries use `stun:`, the plain-TURN (13478) entries use `turn:`, and
      the TLS-secured TURN (13479) entry uses `turns:` — exactly the bug
      class fixed earlier this session. Runs in 0.00s, no server started.
- [x] **`WebRTCMessage` (de)serialization.** Added a `#[cfg(test)] mod
      tests` to `common/data_structure.rs` round-tripping every message
      type (`create_session`, `join_session`, `offer`, `answer`,
      `ice_candidate`, `list_participants`, an error payload) through
      `serde_json`, plus `Default` and missing-optional-field behavior (9
      tests total).
- [x] **Session bookkeeping.** Chose the lighter of the plan's two options:
      rather than extracting a new `SessionRegistry` type, added a
      `#[cfg(test)] mod tests` *inside* `webrtc_server.rs` itself, calling
      the existing private `async fn` handlers
      (`handle_create_session`/`join_session`/`leave_session`/
      `list_participants`/`close_session`/`handle_list_session`) directly
      against a bare `WebRTCServer::new()` — same-module tests can already
      see private items, so no visibility changes were needed at all, and
      none of the production handler logic was touched. No TCP/WebSocket
      connection, no signaling loop; 7 tests covering create (creator is
      sole participant), join (adds participant), re-join (resets rather
      than duplicates — an existing behavior the plan didn't call out
      explicitly but was worth locking in), leave, close, and list.
- [x] **`is_valid_h264`.** Promoted from a private test-only helper
      (duplicated in both `client/tests.rs` and the moved integration
      tests) to `pub fn is_valid_h264` in
      `client/xrds_webrtc/webrtc_client.rs`, with 7 direct unit tests (4-byte
      start code, 3-byte start code, start code mid-buffer, no start code,
      empty input, truncated 1-2 byte input, bare 3-byte code with nothing
      after). `tests/webrtc_integration.rs` now imports the real function
      via `xrds_net::client::webrtc_client::is_valid_h264` instead of
      keeping its own copy.

Verified: all 23 new tests pass, each in 0.00s (`cargo test -p xrds-net --lib
webrtc_server::tests`, `data_structure::tests`, `webrtc_client::tests`) —
genuinely no network, no sleep, no server. Full `cargo test -p xrds-net
--lib` → 150 passed, 2 failed (`test_client_mqtt_sub_pub_rcv`,
`test_ws_connect` — pre-existing public-broker/public-server flakiness,
unrelated; being tracked separately per the user, not a regression from this
phase).

## Phase 5 — re-evaluate the heaviest test's place in the default suite

- [x] `test_client_webrtc_send_video_file` (60-120s even after Phase 3's
      polling fix — real-time H.264 transfer over a real ICE/DTLS/SRTP
      connection can't be sped up without changing what it tests) gated
      behind `#[ignore = "slow: full real-time H.264 file transfer,
      60-120s"]`. Verified: default `cargo test -p xrds-net --test
      webrtc_integration` now reports "1 ignored" and finishes in ~153s
      instead of ~213-265s; `cargo test -p xrds-net --test
      webrtc_integration -- --ignored` (or `--include-ignored` for
      everything) still runs it standalone and it passes (~80s).

**Decision:** `#[ignore]`, not a separate file or feature flag — it's one
test, `#[ignore]` is the standard idiom, and it keeps the test physically
next to its sibling WebRTC tests (shared helpers, same serial group) rather
than needing its own file just to be excluded by default.

## Phase 6 — verification

- [x] Full run: `cargo test -p xrds-net --lib` (unit) → 150 passed, 2 failed
      (public-server flakiness — `echo.websocket.org`/mosquitto-adjacent
      tests, a different pair each run; pre-existing, being tracked
      separately per the user, not from this restructure). `cargo test -p
      xrds-net --test webrtc_integration` (default, heavy test excluded) →
      15 passed, 1 ignored, 1 failed (`ICE reached terminal state Failed` —
      the same environmental flakiness discussed in Phase 3; best observed
      run was a clean 17/17). Correction from the original plan text here:
      `#[serial(webrtc)]` is *not* eliminated — see Phase 1's corrected
      note; Cargo isolates test files into processes, not functions within
      one file.
- [x] Confirmed the Phase 4 ICE-config unit test actually catches its bug:
      temporarily changed the 13479 TURN entry back to `turn:` (the original
      bug), ran
      `cargo test -p xrds-net --lib build_ice_servers_uses_turns_scheme` →
      failed with a clear message naming the bad URL; reverted; re-ran →
      passed again.
- [x] No Android re-run needed: WebRTC is desktop-only per this repo's
      platform policy (never targets Quest/Android XR), and Phase 4's only
      production change (extracting `build_ice_servers()`) is a pure
      code-motion refactor with identical output, covered by the desktop
      integration suite (`test_server_webrtc_offer`/`_answer`/etc. all still
      pass end-to-end against the real `RTCConfiguration`).
- [x] Updated `crates/xrds-net/README.md` (new test file, `--ignored` flag
      for the heavy test) and `MANUAL.md` §16 (WebRTC test location, what
      moved to real unit tests, the `#[ignore]`d test). `MANUAL_WEBRTC.md`
      had no test-layout references to begin with — nothing to change
      there.
- [x] Moving this checklist to `docs/done/` as the final step of this
      response.

---

## Decision log

Fill in as each phase lands.

- Test file split (Phase 1): done — single `crates/xrds-net/tests/webrtc_integration.rs`
  holding all 17 tests (not split further into client/server files; they
  share enough helpers that one file was simpler). `#[serial(webrtc)]`
  retained within the file — see corrected Phase 1 note above.
- Port allocation fix (Phase 2): done — `XRNetServer::start_dynamic()` (new,
  additive; `start()` unchanged) binds to `:0` and reports the OS-assigned
  port back via a oneshot channel before the accept loop starts. Chose to
  add a parallel `start_dynamic`/`run_reporting_port` API rather than change
  `start()`/`run()`'s signature, since those are used outside the test
  suite too (no reason to force callers who don't need the reported port
  to deal with it).
- Sleep → polling conversions (Phase 3): done, with a caveat. Two new
  additive `WebRTCClient` APIs (`ice_connection_state`,
  `close_peer_connection`) plus two test-local polling helpers replaced the
  server-ready, ICE-ready, and transfer-complete sleeps. The remaining
  `sleep(10)`s in the stream tests are deliberate — they're "stream for 10
  seconds", a duration, not a guess at how long something takes. Reliability
  is still environment-limited; see the unchecked Phase 3 item.
- Session-bookkeeping extraction approach (Phase 4): done — chose "call the
  existing private async handlers directly from same-module tests" over
  extracting a new `SessionRegistry` type. Same-module tests already see
  private items, so this needed zero visibility changes and zero risk to
  production logic, at the cost of `#[tokio::test]` instead of a fully
  synchronous `#[test]` (negligible — no actual async work happens without
  a live connection).
- Heavy-test placement (Phase 5): done — `#[ignore]` on
  `test_client_webrtc_send_video_file` only. Run it explicitly with `cargo
  test -p xrds-net --test webrtc_integration -- --ignored`.
