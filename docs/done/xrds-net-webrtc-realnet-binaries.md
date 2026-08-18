# xrds-net — standalone binaries for Phase 3 real-network WebRTC verification

**Status:** Done — all three binaries built, registered, and verified with
a local dry run. The actual two-machine/real-network run itself is tracked
in `docs/xrds-net-release-readiness.md` Phase 3, not here (that doc stays
open until someone runs it).

## Context

`docs/xrds-net-release-readiness.md` Phase 3 needs a real-network WebRTC
handshake verified — the one item in that whole plan that couldn't be done
from this sandbox (no second real-network host). The existing runbook there
says to hand-modify a *copy* of `examples/webrtc/webrtc_file_stream.rs`
(remove its `set_ice_servers(vec![])` loopback override) and run it on two
machines. That's workable but awkward: it requires editing example code
just to test it, both peers currently run from one `main()` so there's
nothing to run separately on Machine B, and there's no way to pass in a
signaling address, a session id, or which role (publisher/subscriber) to
play.

This plan is for three small, purpose-built, standalone binaries that
replace that hand-modification — reusable, not single-use throwaway code.

## Goals

1. Three independently runnable programs: a signaling server, a publisher,
   and a subscriber — so they can genuinely run as separate processes on
   separate machines (today, `webrtc_file_stream.rs` runs all three roles
   from one `main()` on one machine).
2. Use the **default** ICE config (`WebRTCClient::new()`, no
   `set_ice_servers()` override) — the actual production STUN/TURN path,
   unlike the loopback-only existing example.
3. Print enough operator-facing information to confirm what Phase 3 is
   actually trying to prove:
   - the session id (subscriber needs it, printed by the publisher —
     there's no shared state between two machines, so this has to be
     copy-pasted by a human)
   - final `RTCIceConnectionState` and, critically, the winning candidate
     **type** (`host` / `srflx` / `relay`) — a `relay` win is the strongest
     evidence the TURN path specifically works, not just direct/STUN.
4. Reuse, don't duplicate, the polling/diagnostic patterns already proven
   in `tests/webrtc_integration.rs` (`ice_connection_state()` polling) and
   `webrtc_file_stream.rs` (signaling/ICE/stream sequencing).
5. Live in the now-categorized `examples/webrtc/` (per the just-completed
   examples reorg), following the existing `webrtc_*` naming convention.

## Non-goals

- Not adding a CLI-parsing dependency (`clap` etc.) — the argument surface
  is small enough (3-4 flags per binary) that manual `std::env::args()`
  parsing keeps these dependency-free, consistent with every other example
  in this crate.
- Not building a GUI/TUI — plain stdout, readable by a human running these
  by hand on two terminals/machines.
- Not automating the two-machine orchestration itself (no SSH-and-run
  scripting) — a human still starts each binary manually per the runbook.
  That's inherent to what Phase 3 needs (a real second network, which
  can't be scripted from here anyway).
- Not replacing `webrtc_file_stream.rs`/`webrtc_webcam_stream.rs` — those
  stay as the loopback/hardware-local teaching examples; these new
  binaries are specifically for the cross-machine verification case.

---

## Phase 1 — shared support module

- [x] Added `examples/webrtc/realnet_common.rs`, included via
      `#[path = "realnet_common.rs"] #[allow(dead_code)] mod
      realnet_common;` in each of the three binaries (the `allow` is
      because each binary only exercises part of the shared API — it's
      compiled separately into each one via `#[path]`, so unused-per-binary
      items would otherwise warn).
- [x] Contains: a minimal `Args` struct (`--flag value` parsing over
      `std::env::args()`, plus `get_or`/`required`/`get_u32_or`/
      `get_u64_or` typed accessors), `log_ice_summary()`, and
      `print_session_id_banner()`, plus the shared `DEFAULT_SAMPLE_FILE`/
      `DEFAULT_OUTPUT_DIR` constants.
- [x] `log_ice_summary` ended up calling two new `WebRTCClient` methods
      rather than reimplementing polling logic locally:
      - `WebRTCClient::wait_for_ice_connected(max_wait)` — this used to be
        a private helper duplicated in `tests/webrtc_integration.rs`;
        **promoted to the public API** so callers (tests, these examples)
        don't need `webrtc`'s `RTCIceConnectionState` enum as a direct
        dependency just to match its variants. The test file's local copy
        was deleted and its two call sites switched to the method form.
      - `WebRTCClient::active_candidate_pair_summary()` — **new**,
        resolves the "does webrtc/WebRTCClient expose candidate pair type"
        question from the decision log: `RTCPeerConnection::get_stats()`
        (already in the `webrtc` 0.12.0 dependency) returns a
        `StatsReport` with `CandidatePair`/`LocalCandidate`/
        `RemoteCandidate` entries; finds the nominated pair, looks up
        both sides' `candidate_type`, returns a formatted
        `Option<String>` (not the raw `webrtc`-crate types, so examples
        don't need that dependency either). Covered by a unit test
        (`None` before a peer connection exists — the only branch testable
        without a live connection); the real behavior was confirmed by
        the Phase 3 dry run below, which printed real `Host`/
        `PeerReflexive` results.

## Phase 2 — the three binaries

- [x] `examples/webrtc/webrtc_realnet_signaling_server.rs` — flags
      `--port` (default `0`) and `--root-dir` (default `test_output`);
      `XRNetServer::new(...).set_root_dir(...).start_dynamic().await`,
      prints the bound port and a reminder that this machine's usable
      LAN/public IP can't be auto-detected reliably; runs until Ctrl-C.
- [x] `examples/webrtc/webrtc_realnet_publisher.rs` — flags
      `--signaling-addr` (required), `--file` (default: the shared sample
      file), `--stream-seconds` (default 30). No `set_ice_servers()`
      override — uses the real production STUN/TURN list. Creates a
      session, prints it in `print_session_id_banner()`, waits up to 120s
      for a subscriber, exchanges ICE, `log_ice_summary`, sends one data
      channel message, streams the file, then `stop_stream()` +
      `close_peer_connection()`.
- [x] `examples/webrtc/webrtc_realnet_subscriber.rs` — flags
      `--signaling-addr` (required), `--session-id` (required),
      `--output-dir` (default `test_output`). Same ICE-config approach as
      the publisher. Joins, exchanges ICE, `log_ice_summary`, waits 45s for
      the stream (no cross-process "stream complete" signal exists, unlike
      the same-process file-size-polling pattern in
      `tests/webrtc_integration.rs` — see the file's own comment), then
      reports the received file's path and size.
- [x] All three registered in the root `Cargo.toml`'s `[[example]]` list
      (autodiscovery doesn't reach `examples/webrtc/`, same as every other
      example post-reorg) and verified with `cargo check`/`cargo clippy`
      — zero warnings from any of the four new files (the three binaries
      + `realnet_common.rs`).

## Phase 3 — verification (local dry run before the real cross-machine run)

- [x] Ran all three as separate, genuinely independent OS processes on
      this machine (via the Bash tool's `run_in_background`, not shell
      `&` — the latter doesn't survive past its own tool call, which was
      the cause of a first failed attempt: the server and publisher got
      killed when their launching shell exited, and the subscriber then
      failed to connect at all). Over `ws://127.0.0.1:<port>/` for
      signaling, but **using the real default ICE config** (no loopback
      override) — so this dry run's ICE gathering hit real interfaces and
      real STUN servers, not synthetic loopback-only candidates.
      Confirmed working end to end:
      - Session id printed and manually handed off between processes.
      - Both sides reached `ICE connected: Some(Connected)`.
      - `Active candidate pair` printed real results — publisher:
        `local=Host remote=PeerReflexive`; subscriber:
        `local=Host remote=Host` (this machine's own interfaces won over
        the STUN/TURN candidates it also gathered — expected, since both
        peers are in fact on the same host regardless of ICE config).
      - STUN servers were genuinely reachable and returned real `srflx`
        candidates with a real public IP — this sandbox does have
        outbound network access (earlier assumptions in
        `docs/done/xrds-net-webrtc-ice-config-fix.md` about broken
        IPv6/STUN reachability applied to *that* session's environment,
        not universally).
      - Data channel message round-tripped (`Echo: hello from
        webrtc_realnet_publisher`).
      - Subscriber reported a 14,154,318-byte received file with a clean
        teardown on both ends.
      This proves the binaries themselves work — it does **not** prove
      the real cross-machine/TURN-relay path works, since both peers were
      still on one machine. That's the actual remaining Phase 3 gap in
      `docs/xrds-net-release-readiness.md`.
- [x] Updated `docs/xrds-net-release-readiness.md`'s Phase 3 runbook to
      reference these three binaries with concrete commands, replacing the
      "hand-modify a copy of `webrtc_file_stream.rs`" plan.
- [x] Updated `examples/README.md`'s Extension-First Examples table with
      a row for the three new binaries.

---

## Decision log

- CLI parsing approach (Phase 1): done — manual `std::env::args()`
  parsing via a small `Args` struct in `realnet_common.rs`, no new
  dependency, as planned.
- Candidate-pair-type introspection (Phase 1): done — needed a new
  accessor. `WebRTCClient::active_candidate_pair_summary()` wraps
  `RTCPeerConnection::get_stats()` (already available via the `webrtc`
  0.12.0 dependency) and returns a plain `String` summary, so callers
  don't need the `webrtc` crate's stats types directly. While implementing
  its polling counterpart, also promoted `wait_for_ice_connected` from a
  private `tests/webrtc_integration.rs` helper into
  `WebRTCClient::wait_for_ice_connected()` — same rationale (avoid needing
  `RTCIceConnectionState` as a direct dependency in callers), and it
  deduplicated code that used to be copy-pasted.
- Binary naming (Phase 2): confirmed as proposed —
  `webrtc_realnet_signaling_server` / `webrtc_realnet_publisher` /
  `webrtc_realnet_subscriber`. No naming collisions in the examples'
  single flat namespace.
- tokio `signal` feature (discovered during implementation, not in the
  original plan): the root package's `tokio` dev-dependency didn't have
  the `signal` feature enabled (needed for `tokio::signal::ctrl_c()` in
  the signaling server binary) — added it alongside the existing `time`
  feature.
- Post-completion follow-up: added optional `--turn-username`/
  `--turn-password` flags to the publisher and subscriber, as a
  convenience alternative to exporting `XRDS_TURN_USERNAME`/
  `XRDS_TURN_PASSWORD` — both do exactly the same thing
  (`apply_turn_credentials_from_args` in `realnet_common.rs` just sets the
  env vars from the flags before any `WebRTCClient` is created). Verified
  live: running the publisher with fake `--turn-username`/`--turn-password`
  values made the ICE agent actually attempt the TURN relay URLs (visible
  in its gather-candidates logs) instead of skipping straight to STUN-only
  with the "not set" warning — confirming the override reaches
  `build_ice_servers()` correctly.
- Post-completion follow-up, from real usage feedback after building
  these: two more fixes plus a genuine bug found along the way.
  1. **Logging noise.** `RUST_LOG=info` was unconditionally stomping any
     `RUST_LOG` the operator had already set, and "info" also enables
     `warn` (info is *less* severe than warn, not a stricter filter) — so
     every run was flooded with `webrtc_ice`/`webrtc_sctp`/`webrtc_mdns`'s
     internal per-interface-bind and STUN-resolution warnings, almost none
     of which are actionable. Added `realnet_common::init_logger()`:
     respects an existing `RUST_LOG`, otherwise defaults to
     `info,webrtc_ice=error,webrtc_sctp=error,webrtc_mdns=error`. All
     three binaries now call this instead of setting `RUST_LOG` directly.
  2. **The sample video path didn't survive being copied to another
     machine — the exact scenario these binaries exist for.**
     `DEFAULT_SAMPLE_FILE` was `concat!(env!("CARGO_MANIFEST_DIR"), ...)`
     — an *absolute path*, baked in at compile time, pointing at the
     building machine's checkout. Copy the built `.exe` anywhere else (as
     a real user did, to `Desktop\webrtc_test\`) and that path silently
     doesn't exist there. Fixed by switching to
     `include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), ...))` — the
     actual file *contents* are embedded in the binary, not a path to
     them, so `--file` unset now streams from an in-memory
     `std::io::Cursor` with zero filesystem dependency. `--file <path>`
     still works for streaming something else, resolved at runtime as a
     normal path.
  3. **Found via the live re-test, not the plan: ICE failing was handled
     inconsistently.** `log_ice_summary` printed `"ICE did not connect:
     ..."` on failure but returned `()`, so both binaries barrelled ahead
     into `send_data_channel_message()`/the stream-wait regardless. The
     publisher's `.expect(...)` on that call then panicked with a full
     backtrace — turning an already-diagnosed, already-printed, and
     genuinely expected outcome (this sandbox's known ICE flakiness) into
     what looks like a crash bug. `log_ice_summary` now returns `bool`;
     both binaries check it and exit cleanly (with `close_peer_connection()`)
     instead of proceeding. Confirmed via a live re-run: ICE connected
     successfully makes both binaries proceed and stream correctly.
