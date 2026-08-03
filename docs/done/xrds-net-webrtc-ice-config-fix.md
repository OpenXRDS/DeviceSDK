# xrds-net — WebRTC ICE handshake reliability: duplicated ICE config bug

## Context

While investigating ongoing WebRTC handshake flakiness (after
`docs/done/xrds-net-webrtc-test-restructure.md` had already restructured the
test suite and made the flakiness *visible and diagnosable* instead of an
opaque timeout), a real, live bug was found — not just environmental noise.

## Findings

1. **Two separate, un-synced copies of the ICE server list exist:**
   - `crates/xrds-net/src/server/webrtc_server.rs` — `build_ice_servers()`
     (extracted during Phase 4 of the test-restructure work, and unit-tested
     there).
   - `crates/xrds-net/src/client/xrds_webrtc/webrtc_client.rs` — an inline
     `ice_servers: vec![...]` literal inside `WebRTCClient::setup_webrtc()`
     (around line 881-910).

2. **The server's copy is dead code for actual handshakes.** `WebRTCServer`
   builds `rtc_config`/`api` in its own `setup_webrtc()` but never calls
   `new_peer_connection` on it — the comment says "in case of SFU" (a
   future mode, not implemented). `WebRTCServer` is a pure WebSocket
   signaling relay. **Every real `RTCPeerConnection` — publisher and
   subscriber alike — is created client-side**, using the client's own
   inline list.

3. **The client's live copy still has the `turn:`/`turns:` scheme bug**
   that was already found and fixed once, but only in the server's dead
   copy:
   ```rust
   // client/xrds_webrtc/webrtc_client.rs, setup_webrtc()
   RTCIceServer {
       urls: vec![
           "turn:turn.keti.xrds.kr:13478".to_owned(),
           "turn:turn.keti.xrds.kr:13478?transport=tcp".to_owned(),
           "turn:turn.keti.xrds.kr:13479".to_owned(),              // BUG
           "turn:turn.keti.xrds.kr:13479?transport=tcp".to_owned(), // BUG
       ],
       ...
   }
   ```
   Port 13479 is TURN-over-TLS and requires the `turns:` scheme (RFC 7065).
   `turn:` on that port is rejected by the ICE agent — logs show `Unable to
   handle URL in gather_candidates_relay turn:...13479?transport=tcp`. This
   is the exact bug class fixed in
   `docs/done/xrds-net-crypto-consolidation.md`, just never propagated to
   the copy that actually runs.

4. **Google's public STUN servers are already configured** — client-side
   only (`stun.l.google.com:19302`, `stun1.l.google.com:3478`,
   `stun2.l.google.com:19302`). They're commented out in the server's dead
   copy, which is presumably why the user asked whether they're used at
   all.

5. **Environmental noise on top of the code bug:** logs show `failed to
   resolve stun host: ...: No available ipv6 IP address found!` for
   essentially every one of the 11 configured STUN/TURN URLs, on every ICE
   gathering pass. This sandbox has broken/absent IPv6, so each of these
   hostnames fails DNS resolution before falling through to usable
   candidates, adding latency ahead of host/mDNS candidate gathering. For
   the test suite specifically — both peers on `127.0.0.1` — none of this
   remote STUN/TURN gathering is even necessary; host candidates alone
   should connect instantly on loopback. This latency plausibly eats into
   the hardcoded 10s ICE wait inside `start_stream` and the 45s bound added
   to the integration test helpers.

## Goals

1. Fix the live `turn:`/`turns:` bug in the client's ICE config.
2. Eliminate the duplication that let the fix silently not apply everywhere
   — one canonical ICE server list, used by both client and server (even
   though the server's use is currently dead code, keeping it in sync costs
   nothing once there's a single source).
3. Reduce ICE-gathering latency/noise for local/loopback test scenarios,
   without changing production (real-network) behavior.

## Non-goals

- Rewriting the WebRTC handshake/session logic itself.
- Implementing the SFU mode the server's dead `rtc_config` was reserved
  for — out of scope here.
- Fixing the sandbox's IPv6 environment — that's infrastructure, not code.

---

## Phase 1 — fix the live bug, de-duplicate the ICE config

- [x] Created `crates/xrds-net/src/webrtc_ice_config.rs` (new module,
      declared `mod webrtc_ice_config;` in `lib.rs`) with a single
      `pub(crate) fn build_ice_servers() -> Vec<RTCIceServer>` — the STUN
      list is now the client's fuller set (Google's 3 public STUN servers
      plus the keti ones on both ports/transports), and the TURN list has
      the port-13479 entry corrected to `turns:` (matching the fix that
      previously existed only in the server's dead copy).
- [x] `WebRTCClient::setup_webrtc()` (`client/xrds_webrtc/webrtc_client.rs`)
      and `WebRTCServer::setup_webrtc()` (`server/webrtc_server.rs`) both
      now call `crate::webrtc_ice_config::build_ice_servers()` — both
      inline `ice_servers: vec![...]` literals deleted, along with the
      now-unused `RTCIceServer` imports in each file.
- [x] Moved the Phase 4 unit test
      (`build_ice_servers_uses_turns_scheme_for_the_tls_secured_port`) from
      `server/webrtc_server.rs` into `webrtc_ice_config.rs`'s own
      `#[cfg(test)] mod tests` — it now exercises the single function both
      `WebRTCClient` (the live handshake path) and `WebRTCServer` call, so
      there is no copy left for a future fix to miss.
- [x] Verified: temporarily changed the shared function's port-13479 entry
      back to `turn:`, ran
      `cargo test -p xrds-net --lib build_ice_servers_uses_turns_scheme` →
      failed with a clear message naming the bad URL; reverted; re-ran →
      passed. Also ran `cargo test -p xrds-net --lib` (151 passed, 1 failed
      — `test_ws_send`, the pre-existing unrelated `echo.websocket.org`
      flakiness) and `cargo check --workspace` (clean) to confirm nothing
      else broke.

## Phase 2 — reduce test-suite ICE latency/noise

- [x] Added `WebRTCClient::set_ice_servers(&mut self, servers: Vec<RTCIceServer>)`
      — an optional override, stored in a new `ice_servers_override` field,
      read by `setup_webrtc()` (`self.ice_servers_override.clone()
      .unwrap_or_else(build_ice_servers)`). Must be called before
      `publish()`/`join_session()` (whichever triggers `setup_webrtc`
      first). Defaults to the full production list when never called, so
      production behavior is unchanged.
- [x] Updated every `WebRTCClient::new()` call site in
      `tests/webrtc_integration.rs` that goes on to `publish()` or
      `join_session()` (both `establish_*` helpers, plus
      `test_server_webrtc_session_join/_list_participants/_leave/_offer/_answer`)
      to call `.set_ice_servers(vec![])` right after construction — empty
      list means host + mDNS candidates only, which is all two peers on
      `127.0.0.1` need.
- [x] **Found and fixed a second, previously-hidden bug this surfaced:** a
      genuine race in `collect_ice_candidates()`
      (`client/xrds_webrtc/webrtc_client.rs`). Its `tokio::select!` loop
      read from an mpsc channel of candidates and a oneshot "gathering
      complete" signal with no ordering guarantee between them — if both
      were ready in the same poll (candidate already queued, completion
      signal also fired), `select!`'s pseudo-random choice could pick the
      completion branch and break *before* draining the last already-queued
      candidate, surfacing as `"No ICE candidates collected"` even though
      candidates had, in fact, arrived. This was invisible before because
      STUN/TURN network round-trips made gathering slow enough for the
      channel to always drain naturally; skipping them (this phase) makes
      gathering fast enough that the race fires reliably. Fixed by draining
      `candidate_rx.try_recv()` in a loop when the completion branch is
      taken, before treating gathering as done.
- [x] Re-ran the full integration suite 3 consecutive times: 16 passed, 0
      failed, 1 ignored, every time, at ~64s each (down from ~153-265s
      before this phase, and from an unreliable 14-16/17 pass rate). Also
      re-ran the `#[ignore]`d heavy test standalone — still passes (~72s),
      confirming the empty ICE server list doesn't affect the full-transfer
      path either (it never needed STUN/TURN — both peers are on
      loopback).

## Phase 3 — verification

- [x] `cargo check --workspace` clean. `cargo test -p xrds-net --lib` → 151
      passed, 1 failed (`dispatch_ws_reaches_a_real_server` this run —
      pre-existing `echo.websocket.org` flakiness, a different test each
      run, unrelated to this work).
- [x] Pass-rate comparison, `cargo test -p xrds-net --test
      webrtc_integration -- --test-threads=1` (default, heavy test
      excluded):
      | | before Phase 2 | after Phase 2 |
      |---|---|---|
      | Pass rate | 14-16 / 17 (flaky `ICE reached terminal state Failed`) | 16/16, 3/3 consecutive runs |
      | Wall time | ~153-265s | ~64s |
- [x] Checklist moved to `docs/done/` as the final step of this response.

---

## Decision log

Fill in as each phase lands.

- ICE config de-duplication approach (Phase 1): done — new top-level
  `crate::webrtc_ice_config` module (not nested under `common`, since it
  depends on the `webrtc` crate directly and `common` currently doesn't);
  both `client` and `server` call it. Chose the client's fuller STUN list
  (includes Google's public servers) over the server's leaner one, since
  the client's is the copy that actually ran in production.
- Test-suite ICE server override mechanism (Phase 2): done —
  `WebRTCClient::set_ice_servers()`, an additive opt-in override rather
  than an env var or a `WebRTCClient::new()` parameter change, so no
  existing call site (production or otherwise) is affected unless it
  explicitly opts in.
- Measured flakiness improvement (Phase 3): confirmed — 14-16/17 (flaky) →
  16/16 across 3 consecutive runs, ~153-265s → ~64s. Also uncovered and
  fixed an independent race in `collect_ice_candidates()` that Phase 2's
  speed-up exposed (see Phase 2 notes) — without that fix, Phase 2 alone
  would have traded STUN/TURN-DNS flakiness for a different, faster-firing
  flakiness.
