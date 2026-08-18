# xrds-net protocol features — plan

**Status:** in progress. Branch `net-protocol-features`, off `main` at `d8cf8ff`.
Phase 1 done (`f385e94`). Next: Phase 2, `protocol-webrtc`.

Tracks [issue #7](https://github.com/OpenXRDS/DeviceSDK/issues/7) ("xrds-net has too many
functions without distinct features"). The plan summary was posted as
[a comment](https://github.com/OpenXRDS/DeviceSDK/issues/7#issuecomment-5323937296); this
file is the working detail.

## What the issue asked, and what is already done

Issue #7 proposed two options: **(1)** Cargo features per codec/protocol, **(2)** splitting
large blocks into their own crates.

**Option 2 is done for codecs.** Device capture and codec encoding moved to
`crates/xrds-media` (`audio/`, `video/`, `transcoding/`), and `xrds-net`'s manifest
references no codec dependency at all — no ffmpeg, openh264, or image crates. So
`codec-h264`-style features are moot here; the split went further than a flag would have.

Protocol *logic* is also already isolated: one handler per protocol under
`client/protocols/` behind a `ProtocolHandler` trait
(`docs/done/xrds-net-protocol-handler.md`). That satisfies the issue's maintainability
objective by a different route than proposed.

What remains is **Option 1 for protocols**.

## Why it is worth doing — measured

`xrds-net` pulls **287** transitive crates. Only **22** are pulled by no protocol at all,
so nearly the whole graph is protocol-attributable. What a consumer needing a single
protocol would compile, if protocols were gated:

| build with only… | crates | vs 287 |
|---|---:|---|
| quic / http3 | 42 | −86% |
| ftp (client) | 60 | −79% |
| mqtt | 71 | −75% |
| ws | 90 | −69% |
| http | 124 | −57% |
| ftp (server) | 137 | −52% |
| coap | 193 | −33% |
| webrtc | 213 | −26% |

### The measurement trap, recorded so it is not repeated

Asking instead "what is *unique* to each protocol" gives tiny numbers — webrtc 20, coap 7,
mqtt 5 — because **183 of the 287 crates are shared by two or more protocols**. That
measures "remove one protocol, keep the rest", which is not what this issue is about. The
per-protocol subtree sizes sum to 754 against a real total of 287; that gap is the overlap.

I reached the wrong conclusion from that metric once already and nearly recommended closing
the issue. Use the subset table above.

## Hard constraint: the protocol-agnostic contract must survive

`scheme_to_protocol` (`client/scheme.rs:30`) is total — any documented scheme maps to a
`PROTOCOLS` variant — and that totality *is* the design: write once, switch protocol by
URL.

If a feature could remove a scheme, an authored config pointing at `mqtt://` would parse,
validate, deploy, and then silently do nothing. **Authorable-but-inert is the worst failure
mode in this project** — it is exactly what made zone triggers appear broken for two device
sessions (`docs/done/player-body-collider-plan.md`). Do not reintroduce it here.

Therefore:

- **Every protocol enabled by default.** Cargo features are additive, so the default build
  stays exactly as capable as today. Narrowing requires an explicit
  `default-features = false` — deliberate, and only by someone who wants the trade.
- **`PROTOCOLS` and `scheme_to_protocol` stay total.** No `#[cfg]` on enum variants. This
  keeps the contract *and* avoids cfg churn across the ~18 files that match on `PROTOCOLS`,
  where a missed arm would only break in one feature combination.
- **Only the handler varies per feature.** Precedent already exists: `PROTOCOLS::WEBRTC`
  resolves to `UnsupportedHandler` (`client/handler.rs:119`), so "in the vocabulary,
  unavailable in this build" is an existing state rather than a new concept.

## Phases

### Phase 1 — an unavailable protocol says which feature to enable — **DONE** (`f385e94`)

**This phase's premise was wrong, and correcting it was most of the work.**

The claim was that `UnsupportedHandler` "errors nowhere" and would silently no-op. It does
not. The `ProtocolHandler` trait's default `request()` already returns
`NetError::Capability`, the three capability queries default to `None`, and every caller
converts that into a `Capability` error rather than skipping quietly — `client.rs:213`,
`net_intent.rs:182`, `net_channel.rs:54`, `event.rs:249`. The type's doc comment already
anticipated "future feature-disabled protocols once Cargo features exist", and tests already
pinned the erroring.

The real gap was *what it says*: the trait default reports "protocol does not support
request/response", which is true of WEBRTC (dedicated API) but would misattribute a
compiled-out MQTT to a protocol limitation — sending the reader hunting for a limitation
that does not exist.

- [x] `UnsupportedReason::{DedicatedApi { api }, FeatureDisabled { feature }}`; the disabled
      message names the feature to enable.
- [x] `FeatureDisabled` fails at `validate()` — the first thing every caller runs — so a
      missing feature surfaces at connect time, not on first send.
- [x] `DedicatedApi` still validates `Ok` (WebRTC is genuinely available), with a test
      guarding that distinction so the change is not later applied too broadly.
- [x] Three tests, including one asserting each case is *not* reported as the other.

**The open decision resolved itself:** no new `NetError` variant was needed.
`NetError::Capability { protocol, verb, detail }` already existed for exactly this shape, so
there is no API break and callers that already match on `Capability` keep working.

### Phase 2 — `protocol-webrtc`

The cheapest gate, because WebRTC was never wired into the handler dispatch.

- [ ] `webrtc = { version = "0.12.0", optional = true }`; feature `protocol-webrtc`.
- [ ] `#[cfg(feature)]` on `mod xrds_webrtc` (`client/mod.rs:21`) and `mod webrtc_server`
      (`server/mod.rs:5`), plus `webrtc_ice_config.rs`.
- [ ] The two `PROTOCOLS::WEBRTC` arms in `server/server.rs` (~134, ~194) need a
      not-enabled path that logs and declines rather than failing to compile.
- [ ] No change to `PROTOCOLS` or `scheme_to_protocol`.
- [ ] Verify `--no-default-features --features protocol-webrtc` and the inverse both build.

### Phase 3 — feature-matrix CI

Without this, `--no-default-features` builds break silently and nobody learns until a
consumer tries it. Each combination is a separate compilation.

- [ ] Add a job building at least: default, `--no-default-features`, and
      `--no-default-features --features protocol-webrtc`.
- [ ] Fold in `crates/xrds-media` while touching the workflow — it is *not* in the current
      path filter (`crates/xrds-net/**`, `Cargo.toml`, `Cargo.lock`), so changes to it
      trigger no CI at all.

### Phase 4 — `protocol-coap`, if the tax is worth paying

`coap` is the second-heaviest subtree (172 crates) and nothing in the SDK's own runtime,
apps, or examples appears to use it — the best value after webrtc. But CoAP *is* wired into
the dispatch, so this is the first phase to pay the ~18-site cost. Reassess after Phase 3.

## Out of scope

- **The remaining protocols** (http, ws, mqtt, ftp, quic). Each pays the ~18-site cfg tax
  for single-digit unique-crate savings when the others stay enabled. Not worth it until
  someone is actually blocked.
- **Trimming `tokio`.** `tokio = { features = ["full", "test-util"] }` is a *normal*
  dependency, so every consumer compiles all of tokio plus test scaffolding. Cheaper than
  anything here and needs no cfg work — but it is a separate change, not a feature gate.
- **Binary size.** Crate count is not binary size; unused code is largely stripped at link
  time, so gating may already be near-free in a shipped binary while still costing compile
  time. If size is the actual complaint it needs `cargo bloat` against a real target. Not
  measured.

## Verification

- [ ] `cargo check --workspace --all-targets` clean.
- [ ] `cargo test -p xrds-net --lib` no worse than `main` — note 6 pre-existing failures
      there are external-network tests (5 MQTT, 1 FTP), which is why that CI job is
      `continue-on-error`.
- [ ] Default build's dependency graph unchanged: still 287 crates, i.e. nobody loses a
      protocol by accident.
- [ ] A narrowed build actually drops crates — measure, do not assume.
- [ ] A narrowed build fails *loudly* when asked for a protocol it lacks.
