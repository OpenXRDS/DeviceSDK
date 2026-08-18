# xrds-net: DeviceSDK integration (non-blocking task handles + re-export)

**Status: implemented (Phases A–C complete).** `xrds-net` is re-exported
through the DeviceSDK as `xrds::net` (desktop only; excluded on Android), and
the in-app surface — `XrdsNetTask` + `Option::take_ready()` for one-shots,
`NetFeed` for streams, over a bounded `EventStream` — is shipped and tested.
See the Implementation checklist below for what landed in each phase, and
[`examples/net_app.rs`](../examples/net_app.rs) for the recommended usage.
`plugin_net_bevy.rs` (and its `bevy_plugin` feature + optional `bevy`
dependency) has been **removed** — `xrds-net` is now Bevy-free (see its
section). Also shipped: an easy **bidirectional session** handle —
`XrdsNet::open`/`NetChannel` — over **`quic://`** (poll-based `SessionHandler`),
**`ws://`** and **`wss://`** (a `tokio-tungstenite` async-backend
`SessionHandler`; `wss` via the `native-tls` feature). All three are live
round-trip tested. Follow-up (not done): the video-playback epic — decode + an
`XrdsAPI` video-material surface — remains out of scope; this integration only
guarantees the transport leg (see "Boundaries / non-goals").

## TODO / remaining work

Phases A–C shipped (full history in the Implementation checklist below).
Outstanding work, in priority order:

### Done — bidirectional session (`XrdsNet::open` / `NetChannel`), QUIC + WS + WSS

Full design + rationale in "Bidirectional sessions".

- [x] `client/categories.rs`: new `SessionHandler` trait (`connect`/`send`/
      `poll_recv`/`close`); `ProtocolHandler::as_session()` capability query
      (default `None`) in `handler.rs`.
- [x] `protocols/quic.rs`: `QuicHandler` implements `SessionHandler` with the
      **real** stream API — `send` → `conn.stream_send(0, .., false)` + flush;
      `poll_recv` → drain pending datagrams, flush, then `conn.stream_recv`
      over `conn.readable()`, `Ok(None)` when nothing's available
      (non-blocking, single-threaded poll). Handshake factored into a shared
      inherent `establish`.
- [x] `protocols/ws.rs`: `WsHandler` implements `SessionHandler` via a
      separate `tokio-tungstenite` backend (`WsSession`) — dedicated runtime
      thread, one `select!` task (read → bounded channel, send-queue → write),
      sync bridge (`try_send`/`try_recv`). Sync dispatch/listen untouched.
      `ws://` + `wss://` (the latter via the `tokio-tungstenite` `native-tls`
      feature — no new native build dep, reuses suppaftp's).
- [x] `client/net_channel.rs`: `NetChannel` — `send` / `try_recv` (records a
      poll error for `take_error`) / `recv_timeout` / `close`; `Send + Sync`,
      drop-safe (`assert_send_sync` test).
- [x] `net_intent.rs`: `XrdsNet::open` + `open_async`. (`open_with`/
      `open_with_async` deferred — `ListenOptions` has no effect on QUIC's
      poll channel; land them when a policy-configurable buffer is wired.)
- [x] `client/mod.rs` + `lib.rs`: re-export `NetChannel` (+ `SessionHandler`
      at `client::`); `ClientContext::insecure` + `create_quic_config_insecure`
      for self-signed QUIC.
- [x] Tests: mock loopback-`SessionHandler` `NetChannel` tests; `QuicHandler`
      `as_session`/bad-peer units; **live QUIC round-trip** vs the test echo
      server (deterministic); **live `ws://` round-trip** (`XrdsNet::open` vs
      the `XRNetServer` WS echo, deterministic); **live `wss://` round-trip**
      (vs `wss://echo.websocket.org`, tolerant of network unavailability).
- [x] `MANUAL.md` §4/§12: `open`/`NetChannel` documented; QUIC + `ws://` +
      `wss://` session-capable.

### Testability

- [x] Added a minimal raw-QUIC echo server for tests —
      `server/quic_server.rs` (`#[cfg(test)]`, self-signed cert via the
      `rcgen` dev-dependency). The QUIC `NetChannel` now has a live,
      deterministic round-trip test (client connects with
      `ClientContext::insecure` to accept the self-signed cert). It's a
      standalone `QuicServer` the test starts directly, not wired into
      `XRNetServer`'s protocol dispatch (kept out of production).

### WS bidirectional session — done

- [x] `ws://` — `tokio-tungstenite` async-backend `SessionHandler`
      (`WsSession`), live round-trip against the `XRNetServer` WS echo server.
- [x] `wss://` — `tokio-tungstenite` `native-tls` feature enabled; live
      round-trip verified against `wss://echo.websocket.org`.

### Minor / opportunistic

- [ ] `EventStream::is_closed()` — distinguish "closed by remote" from "nothing
      right now"; lets `NetFeed` expose a remote-`Ended` state.
- [ ] Connection pooling/reuse — only if HLS/DASH segment-pull video becomes a
      target (avoids a TCP+TLS handshake per segment).

### Out of scope (tracked; not this crate's job)

- [ ] Video decode + playback (`xrds-media`) and an `XrdsAPI`
      video-material / texture-streaming surface — the "video in the XR scene"
      epic. This crate only guarantees the transport leg.

## Motivation

`xrds-net` (see `docs/xrds-net-protocol-handler.md` for its own design) is a
complete, tested networking crate — `XrdsNet::request/dispatch/listen/transfer`
plus the expert `ClientBuilder`/`Client` API. But today it has **zero path
into the DeviceSDK framework** (`XrdsApp`/`XrdsAPI`/`XrdsUpdateContext`):

- `xrds-runtime`'s `Cargo.toml` already depends on `xrds-net` (gated
  `#[cfg(not(target_os = "android"))]` — see "desktop-only C deps" comment,
  `crates/xrds-runtime/Cargo.toml:44-47`), but nothing in `xrds-runtime/src`
  or `apps/xrds-app/src` references `xrds_net` at all.
- The root `xrds` facade crate only has `xrds-net` as a `[dev-dependencies]`
  entry (`Cargo.toml:95`, used solely by `examples/net.rs`/`net_intent.rs`) —
  not re-exported, not usable by an actual `XrdsApp` implementation without
  the app author adding their own `xrds-net` Cargo dependency by hand.
- `xrds-net`'s `bevy_plugin` feature (`plugin_net_bevy.rs`: `NetPlugin`,
  `NetClientState`, command/output queues) was added in one commit
  (`d8d1750`) and never registered anywhere — not in `xrds-runtime`, not in
  any app. It's unfinished scaffolding, not a working integration (confirmed
  by research: no crate in the workspace enables the `bevy_plugin` feature).

**The blocking problem.** Every `xrds-net` call is synchronous and some are
slow: MQTT's `send`/`subscribe` block up to 5s waiting for
PUBACK/SUBACK, CoAP/HTTP3 have their own multi-second timeouts, FTP downloads
block for the whole transfer. `XrdsApp::update()` runs once per frame inside
Bevy's main schedule (`run_xrds_app_update` executes as an exclusive system in
`PostUpdate`, see `xrds_api.rs`) — calling `XrdsNet::request(..)` directly
from `update()` would freeze the frame (and every frame after it, since
`update()` is called again next frame while the first call is still
blocking the single thread it runs on).

**The precedent.** This isn't a new problem for this codebase:
`XrdsAPI::gltf_load_status(&handle) -> Option<XrdsGltfLoadStatus>` (and the
context-side equivalent) already solves "kick off slow work, poll a status
enum every frame until it resolves" for glTF asset loading
(`NotLoaded`/`Loading`/`Loaded`/`Failed(String)`). `xrds-net`'s own
`EventStream` already solves "ongoing background thread feeding a channel,
drained without blocking" for `listen()`. This plan combines both: a
one-shot poll-able task handle for `request`/`dispatch`/`transfer`, and a
non-blocking drain added to the already-existing `EventStream` for `listen`.

## Design decision: lives in `xrds-net`, not in `XrdsAPI`

Two shapes were considered:

1. **New `XrdsAPI`/`XrdsUpdateContext` methods** mirroring
   `gltf_load_status` exactly (`api.net_request(url, opts) -> handle`,
   `ctx.net_request_status(&handle) -> XrdsNetStatus`). More XRDS-native, but
   means `XrdsAPI`'s already-large method surface grows by a verb × poll
   pair per intent verb, and the handle would need somewhere to live across
   frames (a Bevy resource keyed by handle id), all for what is, underneath,
   plain non-Bevy Rust (a channel and a thread).
2. **A poll-able task type in `xrds-net` itself**, re-exported through
   `xrds-runtime`/`xrds` so `XrdsApp` code gets it "for free." The app's own
   struct holds `Option<XrdsNetTask<T>>` as a plain field and polls it in
   `update()` — no `XrdsAPI`/`XrdsUpdateContext` involvement needed at all,
   since the task owns its own channel and doesn't touch the `World`.

**Chosen: (2).** It's less API surface to design and maintain, it keeps
`xrds-net` usable identically whether or not you're inside the DeviceSDK
(matching "Bevy should remain an implementation engine, not required
interface" — nothing here is Bevy-shaped), and "can a non-expert use this
through XRDS concepts alone?" is satisfied by the re-export, not by growing
`XrdsAPI`. The ease-of-use bar is then met *within* `xrds-net` by a thin
ergonomic layer (`take_ready` + `NetFeed`, see below) over the primitives —
so there's a two-tier surface (easy default + primitive control path) that
still lives entirely in `xrds-net`, no `XrdsAPI` growth required.

## `XrdsNetTask<T>`

New file: `crates/xrds-net/src/client/net_task.rs`.

**`Sync` requirement (learned in Phase C).** An `XrdsApp` holds tasks as
fields and `Runtime::run_xrds<A>` bounds `A: XrdsApp + Send + Sync + 'static`
(Bevy stores it in a `Resource`). `std::sync::mpsc::Receiver` is `Send` but
**not `Sync`**, so a bare `rx: Receiver<..>` makes the whole app non-`Sync`
and `run_xrds` rejects it. The fix is to wrap the receiver in a `Mutex` — the
lock is never contended (the task lives in one place), it's purely to satisfy
the type bound. (Same class of gotcha as `ProtocolHandler: Send + Sync` from
the protocol-handler work; a `fn assert_send_sync::<XrdsNetTask<_>>()` /
`::<NetFeed>()` test guards it now.)

```rust
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Mutex;

/// A background-thread-backed handle to a still-running XrdsNet call.
/// Drain it every frame from `XrdsApp::update()` (or anywhere) with
/// `try_take()` — which never blocks, and neither does dropping it.
pub struct XrdsNetTask<T> {
    rx: Mutex<Receiver<Result<T, NetError>>>, // Mutex only so the task is Sync
    done: bool,
}

impl<T: Send + 'static> XrdsNetTask<T> {
    pub(crate) fn spawn(f: impl FnOnce() -> Result<T, NetError> + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        // Detached on purpose — never joined. The worker runs one blocking
        // call, sends the result once, and exits. If the task is dropped
        // first, the receiver is gone and `tx.send` just no-ops; the thread
        // still winds down on its own. Nothing to join, so nothing to block
        // on (see "Why no `Drop`" below).
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        Self { rx: Mutex::new(rx), done: false }
    }

    /// Non-blocking. `None` = still running. `Some(_)` = finished, and the
    /// **owned** result is handed to you (no clone needed for large bodies /
    /// frames); the task is then spent and every later call returns `None`.
    /// Poll this once per frame until you get `Some`.
    pub fn try_take(&mut self) -> Option<Result<T, NetError>> {
        if self.done {
            return None;
        }
        match self.rx.lock().unwrap().try_recv() {
            Ok(result) => {
                self.done = true;
                Some(result)
            }
            Err(TryRecvError::Empty) => None, // still running
            Err(TryRecvError::Disconnected) => {
                // Worker vanished without sending (e.g. it panicked mid-call).
                // Surface it once as an error rather than looking pending
                // forever, then mark spent.
                self.done = true;
                Some(Err(NetError::Network(
                    "network task ended without producing a result".to_string(),
                )))
            }
        }
    }
}
```

**Why no `Drop`.** Unlike `EventStream` (a *looping* worker that needs a
shutdown flag + join), this worker does exactly one blocking call then exits.
So the task holds no `JoinHandle` and has no `Drop` impl: dropping it just
closes the `Receiver`, and the detached worker finishes its in-flight call in
the background and exits. **This is the point** — dropping a still-pending
task (cancelling a request, or discarding a fire-and-forget `dispatch_async`)
**never blocks the caller**, even if the underlying call would otherwise take
5s. Blocking-on-drop would defeat the entire "never stall the frame loop"
premise, so it's deliberately avoided. (Trade-off: at process exit a detached
worker mid-call isn't waited on — acceptable, and never a hang.)

There's intentionally no `poll() -> &Result` borrow-view and no `is_ready()`:
`try_take()` alone covers "is it done, and if so give me the value," which is
all a per-frame consumer needs, and hands back ownership so the result can be
stored or moved without cloning.

## Four async constructors on `XrdsNet`

Added directly to the existing `impl XrdsNet` block in `net_intent.rs`:

```rust
impl XrdsNet {
    pub fn request_async(url: &str, opts: RequestOptions) -> XrdsNetTask<NetResponse> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::request(&url, opts))
    }

    pub fn dispatch_async(url: &str, payload: Vec<u8>) -> XrdsNetTask<()> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::dispatch(&url, payload))
    }

    /// Resolves once `connect()` (+ subscribe, for MQTT) succeeds or fails —
    /// the same up-front blocking `listen()` already does, just off the
    /// calling thread. The `EventStream` it resolves to is drained
    /// non-blockingly afterward (see `EventStream::try_recv` below). Uses
    /// `ListenOptions::default()` for the stream's internal buffer — see
    /// "Backpressure: bounded buffer + overflow policy" for why the buffer is
    /// bounded and when you'd want `listen_with_async` instead.
    pub fn listen_async(url: &str) -> XrdsNetTask<EventStream> {
        Self::listen_with_async(url, ListenOptions::default())
    }

    /// As `listen_async`, but with an explicit buffer size and overflow
    /// policy — the knob a video/high-throughput consumer needs (small
    /// buffer + `Overflow::DropOldest` for live; larger buffer +
    /// `Overflow::Block` for lossless VOD-style delivery).
    pub fn listen_with_async(url: &str, opts: ListenOptions) -> XrdsNetTask<EventStream> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::listen_with(&url, opts))
    }

    pub fn transfer_async(url: &str, op: TransferOp) -> XrdsNetTask<TransferResult> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::transfer(&url, op))
    }
}
```

The synchronous `request`/`dispatch`/`transfer` are unchanged. `listen` gains
a `listen_with(url, ListenOptions)` variant and its no-options form now
delegates to it with `ListenOptions::default()` (the only behavior change to
shipped Phase 0-5 code — see the next section for why, and note it's a
strictly-safer default, not a new hazard). Everything else here is a pure
addition.

**Which form to call.** Inside `XrdsApp` (`setup`/`update`) always use the
`_async` forms — the synchronous `XrdsNet::request`/`dispatch`/`listen`/
`transfer` run on the calling thread and will freeze the frame. The sync forms
are for scripts, tests, and non-Bevy tooling where blocking is fine. The
sync methods' doc comments should say so explicitly, since the naming
(`request` vs `request_async`) otherwise implies the blocking one is the
default when, for XR-app code, it's the trap. *(A future lint/wrapper could
enforce this, but a doc note is enough for now.)*

**One thread per call.** Each `_async` call spawns a fresh OS thread that
lives until the call finishes. That's right for one-shot `request`/`dispatch`/
`transfer` and for a long-lived `listen`. It is **not** a per-frame primitive:
calling `request_async` every frame spawns a thread per frame (churn + ~2 MB
reserved stack each). For a repeating feed, open **one** `listen` stream and
drain it; for periodic polling, keep the single in-flight task around and only
re-issue once it's `try_take`-n. (If high-frequency one-shots ever become a
real pattern, a shared worker pool behind `spawn` is the natural follow-up —
out of scope here.)

## `EventStream::try_recv()`: non-blocking per-frame drain

`EventStream` today only exposes blocking consumption (`Iterator::next` via
`rx.recv()`, and `recv_timeout`). Add:

```rust
impl EventStream {
    /// Never blocks. `None` means "no event waiting right now," not
    /// "the stream ended" — check the stream's own error handling
    /// (a dead connection ends the background loop, which will eventually
    /// show up as the buffer disconnecting; a future `is_closed()` could
    /// expose that distinctly, but isn't needed for this pass).
    pub fn try_recv(&self) -> Option<Event> { /* pop from the bounded buffer */ }
}
```

This is what an `XrdsApp::update()` loop actually calls every frame after
`listen_async` resolves — draining zero-or-more events per frame, exactly
like `XrdsUpdateContext::world_button_presses()`'s per-frame iterator-drain
convention, just without needing `ctx` since `EventStream` is a plain value
the app struct owns.

## Backpressure: bounded buffer + overflow policy (required for video)

**This is the one change to shipped Phase 0-5 code, and it's load-bearing for
the video-transport use case.** As shipped, `EventStream::spawn` forwards the
worker's `recv()` output over an **unbounded** `std::sync::mpsc::channel()`.
The worker's `send` never blocks, so if the consumer (the frame loop, or the
decoder behind it) ever falls behind the network arrival rate, the queue grows
without bound. For low-rate commands/telemetry this is invisible; for video
(5-50 Mbps, sustained, on a memory-tight headset) a single consumer hiccup
balloons the backlog — latency creep, then OOM. So the transport is fine for
video **threading-wise** (blocking I/O stays off the frame loop; ~1 worker
thread per stream), but **not resource-wise until the buffer is bounded.**

Replace the unbounded channel with a bounded buffer plus an explicit overflow
policy, selected per stream:

```rust
#[derive(Debug, Clone)]
pub struct ListenOptions {
    /// Max events held before `overflow` kicks in. Counted in events
    /// (chunks/frames), not bytes.
    pub buffer: usize,
    pub overflow: Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    /// Worker's send blocks when the buffer is full. For a TCP-backed
    /// transport this propagates into TCP flow control (the socket read
    /// buffer fills, the window closes, the sender throttles) — lossless.
    /// Right for reliable/VOD-style delivery. This is the default.
    Block,
    /// Worker drops the oldest buffered event to make room for the newest,
    /// never blocking. Bounds both memory and latency at the cost of losing
    /// stale data. Right for live/real-time video, where a late frame is
    /// worthless and you'd rather stay current.
    DropOldest,
}

impl Default for ListenOptions {
    // Lossless + a buffer big enough to absorb a normal frame-time spike,
    // but bounded. A plain `listen("...")` is now safe by default: under
    // sustained backpressure it applies flow control instead of growing
    // forever. The only observable change from the shipped unbounded version.
    fn default() -> Self { Self { buffer: 256, overflow: Overflow::Block } }
}
```

Implementation notes for `EventStream`:

- **`Overflow::Block`** → `std::sync::mpsc::sync_channel(buffer)`; the worker's
  `send` blocks when full. Because the worker then stops calling the handler's
  `recv()`, the underlying socket buffer fills and TCP does the throttling for
  us — no protocol-specific backpressure code needed.
- **`Overflow::DropOldest`** → an `Arc<(Mutex<VecDeque<Event>>, Condvar)>` ring
  of capacity `buffer`: the worker `push_back`s and, when at capacity,
  `pop_front`s first (drops oldest); the worker never blocks. `try_recv`/
  `recv_timeout`/`Iterator::next` pop from the front. (Std's `sync_channel`
  can't drop-oldest from the sender side, hence the small ring.)
- `try_recv`/`recv_timeout`/`next`/`close` keep the same public signatures
  across both policies — the buffer type is an internal detail.

A live-video consumer would build the stream with e.g. `ListenOptions {
buffer: 4, overflow: Overflow::DropOldest }` — a shallow queue that always
holds the freshest few frames and never stalls the network thread or bloats
memory. A file/VOD-style consumer keeps the lossless `Block` default.

`XrdsNet::listen`/`listen_with` (sync) and `listen_async`/`listen_with_async`
all thread `ListenOptions` down into `EventStream::spawn`.

## Recommended app surface: `take_ready()` + `NetFeed`

`XrdsNetTask` and `EventStream` above are the **primitives** — the control
path. They work, but they push per-frame bookkeeping (own an `Option`, poll,
null it; or hand-roll a connect→stream state machine) onto every app. Two thin
conveniences on top turn the common cases into one-liners and are the
**recommended default**; the primitives stay available for code that wants the
control. This is the same default-vs-expert split the rest of the SDK uses.

### `take_ready()` — one-shot without the bookkeeping

A one-shot task almost always lives in an `Option` field that gets nulled once
it resolves. An extension trait folds "poll, hand back the owned result, and
clear the slot" into one call:

```rust
// client/net_task.rs
pub trait NetTaskSlot<T> {
    /// Non-blocking. `Some(_)` exactly once, when the task finishes — and the
    /// slot is reset to `None` for you (can't forget it, and a later frame
    /// won't re-handle a spent task). `None` while pending or already taken.
    fn take_ready(&mut self) -> Option<Result<T, NetError>>;
}

impl<T: Send + 'static> NetTaskSlot<T> for Option<XrdsNetTask<T>> {
    fn take_ready(&mut self) -> Option<Result<T, NetError>> {
        let result = self.as_mut()?.try_take()?;
        *self = None; // spent — clear the slot
        Some(result)
    }
}
```

### `NetFeed` — streaming without the two-stage handshake

`NetFeed` hides the `connect → stream` transition behind a single value the
app holds and drains. It owns the `XrdsNetTask<EventStream>` while connecting,
flips to the live `EventStream` once it resolves, and surfaces a connect/
subscribe failure once — so the app never writes the 3-state enum by hand:

```rust
// client/net_feed.rs
enum FeedState {
    Connecting(XrdsNetTask<EventStream>),
    Streaming(EventStream),
    Ended,
}

pub struct NetFeed {
    state: FeedState,
    error: Option<NetError>,
}

impl NetFeed {
    /// Non-blocking. `None` = nothing available yet (still connecting, or no
    /// event arrived this frame). Drain in a `while let` each frame.
    pub fn try_recv(&mut self) -> Option<Event> {
        if let FeedState::Connecting(task) = &mut self.state {
            match task.try_take() {
                None => return None, // still connecting
                Some(Ok(stream)) => self.state = FeedState::Streaming(stream),
                Some(Err(e)) => {
                    self.error = Some(e);
                    self.state = FeedState::Ended;
                    return None;
                }
            }
        }
        match &mut self.state {
            FeedState::Streaming(stream) => stream.try_recv(),
            _ => None,
        }
    }

    /// A connect/subscribe failure, surfaced once. Poll it alongside
    /// `try_recv` if you want to react to the feed failing to come up.
    pub fn take_error(&mut self) -> Option<NetError> {
        self.error.take()
    }

    /// Stop the feed — non-blocking whether still connecting (drops the
    /// task) or streaming (immediate `EventStream::close()`).
    pub fn close(self) { /* drop task, or stream.close() */ }
}
```

`XrdsNet::listen_feed(url)` / `listen_feed_with(url, opts)` construct it,
mirroring `listen`/`listen_with`.

One honest limitation, inherited from `EventStream`: there's no distinct
"stream ended cleanly by the remote" signal yet — `try_recv() == None` means
"nothing right now," not "closed." A future `EventStream::is_closed()` (already
flagged in that type's docs) would let `NetFeed` expose a remote-`Ended` state;
not needed for this pass.

## Re-export: `xrds_runtime::net` (Android-excluded)

`crates/xrds-runtime/src/lib.rs` gets:

```rust
// xrds-net has desktop-only C deps (curl, ffmpeg, quiche, webrtc, libunftp) —
// excluded on Android, matching the Cargo.toml dependency gating.
#[cfg(not(target_os = "android"))]
pub use xrds_net as net;
```

This flows up through the root `xrds` crate's existing `pub use
xrds_runtime::*;` (`src/lib.rs`), making `xrds::net::XrdsNet` (and
`ClientBuilder`, `NetError`, `RequestOptions`, etc.) available to app code
without any extra Cargo dependency — mirroring the existing `pub use
xrds_scene_graph as scene_graph;` precedent for exposing a whole crate as a
named module of the facade. `xrds-net` moves conceptually from "example-only
dev-dependency of the root crate" to "part of the DeviceSDK surface,"
matching what the user asked for ("xrds-net is supposed to run with our
DeviceSDK framework").

**Also re-export `NetResponse` at the `xrds-net` crate root.** Today
`xrds-net/src/lib.rs` re-exports `XrdsNet`, `RequestOptions`, `Event`,
`EventStream`, `NetError`, `TransferOp`, `TransferResult` at the root — but
**not** `NetResponse`, which sits at `common::data_structure::NetResponse`. So
a single request round-trip currently forces the developer to import
`XrdsNet`/`RequestOptions` from `xrds::net::...` and their *return type* from
`xrds::net::common::data_structure::NetResponse` — two different depths for one
call. Adding `NetResponse` (and confirming `Event`/`TransferResult`, which are
already re-exported) to the root `pub use` closes that seam, so one
`use xrds::net::{XrdsNet, RequestOptions, NetResponse, ...};` covers everything
a request needs. Cheap, and it removes the "or wherever `NetResponse`
re-exports land" hedge from the example below.

Root `Cargo.toml`'s `[dev-dependencies]` entry for `xrds-net` (`Cargo.toml:95`)
can stay as-is or be removed once `examples/net.rs`/`net_intent.rs` are
updated to import via `xrds::net::...` instead of `xrds_net::...` directly —
deferred to the examples-update step below so the crate-level plumbing lands
first and is verified independently.

## Example usage inside an `XrdsApp`

These use the recommended surface (`take_ready` + `NetFeed`). The primitive
path (`XrdsNetTask::try_take` directly, or a hand-written connect→stream
state machine over `EventStream`) is documented in the sections above and
stays available for code that wants to observe the transitions itself.

### One-shot (`take_ready`)

```rust
use xrds::net::{XrdsNet, XrdsNetTask, NetTaskSlot, RequestOptions, NetResponse};

struct MyApp {
    manifest_request: Option<XrdsNetTask<NetResponse>>,
}

impl XrdsApp for MyApp {
    fn setup(&mut self, _api: &mut XrdsAPI) {
        self.manifest_request = Some(XrdsNet::request_async(
            "https://api.example.com/manifest",
            RequestOptions::get(),
        ));
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext) {
        if let Some(result) = self.manifest_request.take_ready() {
            match result {
                Ok(response) => { /* own `response` — store response.body, etc. */ }
                Err(e) => log::error!("manifest request failed: {e}"),
            }
            // slot is already cleared for us
        }
    }
}
```

No protocol enum, no async runtime, no Bevy system registration, no manual
slot-clearing — the app owns a field and drains it in one line.

### Streaming (`NetFeed`)

```rust
use xrds::net::{XrdsNet, NetFeed, ListenOptions, Overflow, Event};

struct MyApp {
    feed: Option<NetFeed>,
}

impl XrdsApp for MyApp {
    fn setup(&mut self, _api: &mut XrdsAPI) {
        // Live feed: shallow buffer + drop-oldest — always hold the freshest
        // few frames, never stall the network thread or bloat memory. For a
        // command/telemetry feed that can't drop, use `listen_feed(url)`
        // (bounded + lossless default) instead.
        let opts = ListenOptions { buffer: 4, overflow: Overflow::DropOldest };
        self.feed = Some(XrdsNet::listen_feed_with(
            "mqtt://broker.local/room/telemetry",
            opts,
        ));
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext) {
        if let Some(feed) = &mut self.feed {
            // Drain everything that arrived since last frame — never blocks,
            // and returns nothing until the subscription is live.
            while let Some(Event { payload, .. }) = feed.try_recv() {
                // hand `payload` to the decoder / apply it to the scene
            }
            if let Some(e) = feed.take_error() {
                log::error!("feed failed: {e}");
                self.feed = None; // drop is non-blocking
            }
        }
    }
}
```

No enum, no handshake bookkeeping — hold one value and drain it. Stopping the
feed (`self.feed = None`, or `feed.close()`) is non-blocking whether it's still
connecting or already streaming.

## `plugin_net_bevy.rs`: removed

**Removed (Phase C).** `NetPlugin`/`NetClientState`/`NetCommand`/`NetOutput`
were unfinished, unregistered scaffolding — never enabled by any crate, only
kept compiling by `--all-features` builds. The task-handle approach shipped
here supersedes them outright (it needs no Bevy `Resource`/`System`/
command-queue machinery), so the module, its `bevy_plugin` Cargo feature, and
the optional `bevy` dependency were all deleted. The empty
`examples/net_bevy.rs` stub that was meant to demonstrate the plugin was
deleted too, superseded by the working
[`examples/net_app.rs`](../examples/net_app.rs).

**Consequence — and the answer to "how does the net crate work with Bevy
now?":** `xrds-net` no longer depends on Bevy *at all* (confirmed: `bevy` was
its only Bevy reference, and `grep bevy crates/xrds-net/src` is now empty). It
integrates with a Bevy/DeviceSDK app precisely *because* it stays engine-
agnostic — plain threads + channels + non-blocking polls. The app (which is
the Bevy side, via `Runtime::run_xrds`) holds an `XrdsNetTask`/`NetFeed` as a
field and drains it in `update()`; nothing Bevy-shaped is needed inside
`xrds-net`. That's the whole point of the redesign, and it's what
`examples/net_app.rs` demonstrates. A plugin that reached into Bevy from the
network crate was the wrong direction of dependency; removing it is a
strict improvement.

## Boundaries / non-goals

This bridge is **transport only** — it moves bytes between the network and the
app's frame loop with a threading/resource model safe for video-rate data. It
deliberately does **not** own the rest of a media pipeline. For the scenario
"video streamed from a server, playing in the XR scene," this plan is one leg
of several; the others are explicitly out of scope here:

- **Decode** (encoded bytes → raw frames) — belongs to `xrds-media`'s
  stated-but-unbuilt playback mandate (see
  `docs/done/xrds-net-capture-decoupling.md`), not to `xrds-net`, which never
  touches a codec.
- **Present** (raw frames → GPU texture → scene material, per frame) — belongs
  to `xrds-runtime`/`XrdsAPI`, which has no texture/material-image surface
  today. This plan's "no new `XrdsAPI` surface" decision is right for
  data/telemetry but does nothing for video presentation; a video-material /
  texture-streaming surface is a separate `XrdsAPI` design.
- **Playback control** (pacing, jitter buffer, A/V sync, seek/pause) — again
  `xrds-media` playback, unbuilt.

Full "video in the XR scene" is therefore a distinct future epic composing
this transport bridge + an `xrds-media` decoder/player + an `XrdsAPI`
video-material surface. What this plan guarantees is that the *transport leg*
won't be the thing that makes it infeasible (threading stays off the frame
loop; memory stays bounded via `ListenOptions`).

### WebRTC stays on its own path (for now)

WebRTC — the usual choice for low-latency live media — is **not** wired
through this bridge and **not** part of the intent verbs. It keeps its
existing standalone `WebRTCClient` API (push/callback `on_video_track`, its own
internal tokio runtime and jitter buffer, receive side hands you raw
`TrackRemote` RTP). Wiring WebRTC into the DeviceSDK is its own future epic
with a different threading/resource model; nothing here touches it. So "any
video-delivery protocol" holds for **pull/message-shaped** delivery
(MoQ, WS-based, chunked pub-sub) via `listen`; WebRTC is the stated carve-out.

### Connection reuse — a flagged prerequisite *if* segment-pull video is targeted

Segment-pull protocols (HLS/DASH) aren't `listen` at all — they're a *sequence
of `request_async` GETs* driven by the app's player. That works, but the
plan's deliberate **no-connection-pooling** stance (each call builds a fresh
`Client` = fresh TCP+TLS handshake — see "Out of scope" in
`docs/done/xrds-net-protocol-handler.md`) means a full handshake **per
segment** (every ~2-6s). Tolerable but wasteful. **If** HLS/DASH-style video
becomes a real target, connection reuse/pooling graduates from "out of scope"
to a prerequisite for that path — flagged here, not solved here.

## Bidirectional sessions — `XrdsNet::open` / `NetChannel` (QUIC shipped; WS pending)

**The gap.** The intent verbs cover req/resp (`request`), publish (`dispatch`),
subscribe (`listen`), and file (`transfer`) — but **not "one connection I both
send on and read replies from."** That bidirectional-session shape is exactly
what WS and raw QUIC are for (a persistent control/data channel to a server).
`dispatch`/`listen` don't fill it: they're the *pub/sub* shape (two
independent, broker-mediated connections), which is why they compose over an
MQTT broker but not over a point-to-point WS socket.

**Status:** shipped for **`quic://`** (poll-based `SessionHandler`),
**`ws://`** and **`wss://`** (a `tokio-tungstenite` async-backend
`SessionHandler`; `wss` via the `native-tls` feature), via the shared
`NetChannel` / `XrdsNet::open`(`_async`). All three have live round-trip
tests.

**Intended interface** (an easy, frame-loop-friendly session handle):

```rust
let mut chan = XrdsNet::open("wss://host/control")?; // one connection
chan.send(b"hello".to_vec())?;                        // send on it
while let Some(ev) = chan.try_recv() { handle(ev); }  // read replies on it (non-blocking)
// + open_async(url) -> XrdsNetTask<NetChannel>, open_with(url, ListenOptions), close()
```

`NetChannel` would reuse the bounded `Buffer` + a background reader (like
`EventStream`) and add a send path — `Send + Sync`, drop-safe, same as the
task/feed types.

**Backend readiness** (why QUIC first, WS later):

- **QUIC — done** (`protocols/quic.rs`). The handler already pumped packets
  and completed the handshake; it just never touched the application-stream
  API. Wiring quiche's ready-made `stream_send`/`stream_recv` in (h3 already
  proved the machinery) made it session-capable: `send` → `stream_send(0, ..,
  false)` + flush; `poll_recv` → drain datagrams, flush, `stream_recv` over
  `conn.readable()`, `None` when empty. Single-threaded poll — no
  duplex-thread problem, so no background reader needed for QUIC.
- **WS — done for `ws://`** (`protocols/ws.rs`, `WsSession`). The sync
  `websocket`-crate path (`Arc<Mutex<Client>>` + blocking `recv_message` under
  the lock) can't do frame-safe duplex, so `open` gets a **separate**
  `tokio-tungstenite` backend: a dedicated OS thread hosts a current-thread
  runtime running one task that `select!`s over read → a bounded channel and a
  send-queue → write, bridged to the sync `SessionHandler` with channels
  (`send` = `try_send`, `poll_recv` = `try_recv`). Same
  background-runtime-+-channels pattern as `AudioTrackWriter`. The existing
  sync dispatch/listen path is untouched (WS temporarily has two backends —
  consolidation is future work). **`wss://` is enabled** via the
  `tokio-tungstenite` `native-tls` feature (reuses the `native-tls` stack
  `suppaftp` already pulls — no new native build dep); `connect_async` handles
  TLS automatically. Verified against `wss://echo.websocket.org`.

**Checklist:**

- [x] `SessionHandler` capability + QUIC `stream_send`/`stream_recv`
      (session-capable, non-blocking poll)
- [x] `client/net_channel.rs`: `NetChannel` (`send`/`try_recv`/`take_error`/
      `recv_timeout`/`close`); `Send + Sync`, drop-safe. (No background reader
      needed for the poll-based QUIC backend; the buffered-reader design
      returns for the WS-backed path.)
- [x] `net_intent.rs`: `XrdsNet::open` / `open_async` (the `_with` variants
      wait for the buffered WS path — `ListenOptions` is a no-op for QUIC)
- [x] `client/mod.rs` + `lib.rs`: re-export `NetChannel`
- [x] Unit tests over a mock loopback session handler (ordered round-trip,
      non-blocking empty, poll-error→`take_error`, Send+Sync) + `QuicHandler`
      `as_session`/bad-peer tests
- [x] MANUAL.md §4: `open`/`NetChannel` documented; WS-session caveat noted
- [x] A minimal raw-QUIC echo server for tests (`server/quic_server.rs`,
      `#[cfg(test)]`, self-signed cert via `rcgen`) + a live, deterministic
      QUIC `NetChannel` round-trip test. Client accepts the self-signed cert
      via `ClientContext::insecure` (`create_quic_config_insecure`).
- [x] **WS (`ws://`)**: `tokio-tungstenite` async-backend `SessionHandler`
      (`WsSession`) → `NetChannel` over `ws://`, with a live round-trip test
      against the built-in `XRNetServer` WS **echo** server (via
      `XrdsNet::open`). Sync dispatch/listen left untouched.
- [x] **WS (`wss://`)**: enabled the `tokio-tungstenite` `native-tls` feature
      (reuses the `native-tls` stack `suppaftp` already pulls — no new native
      build dep). Live round-trip verified against `wss://echo.websocket.org`.

## Implementation checklist

Phase A splits into three independent-as-possible workstreams plus a gate.
**A1 (bounded stream)** and **A2 (one-shot task)** touch disjoint files
(`event.rs` vs `net_task.rs`) and depend on nothing from each other — they can
land in either order or in parallel. **A3 (streaming glue)** is the only part
that needs both A1 and A2. **A4** is the final baseline gate. Each of A1–A3
compiles and is independently testable/shippable, and each re-exports the
types it introduces so the crate is never left half-wired.

### Phase A1 — Bounded `EventStream` (independent; closes the unbounded-memory hazard)

Entirely within `event.rs` + `listen`'s sync path; no dependency on the task
work. Valuable on its own (turns the shipped unbounded `listen` into a
bounded, video-safe one).

- [x] `client/event.rs`: `ListenOptions` + `Overflow` enum
- [x] `client/event.rs`: replace `EventStream`'s unbounded channel with a
      bounded buffer honoring the policy — implemented as **one unified
      `Mutex<VecDeque>`+`Condvar` buffer** (rather than `sync_channel` for
      `Block` + a separate ring for `DropOldest`): the two policies differ only
      in what `push` does when full (`Block` waits on a `not_full` condvar,
      `DropOldest` pops-front-then-pushes), which keeps the consumer side
      (`try_recv`/`recv_timeout`/`next`) single-implementation; `EventStream::
      spawn` takes `ListenOptions`
- [x] `client/event.rs`: `EventStream::try_recv()` (non-blocking drain)
- [x] `net_intent.rs`: `XrdsNet::listen_with(url, ListenOptions)`; make
      `listen(url)` delegate with `ListenOptions::default()`
- [x] `client/mod.rs` + `lib.rs`: re-export `ListenOptions`, `Overflow`
- [x] Unit tests: `Block` applies backpressure (a full buffer makes the
      producer wait, no unbounded growth); `DropOldest` caps at `buffer` and
      keeps the newest events, dropping oldest; both preserve order among
      retained events; `listen(url)` default is bounded; plus a
      consumer-gone-unblocks-`Block`-producer teardown test and a non-blocking
      `try_recv` drain test

### Phase A2 — One-shot async task (independent; needs nothing from A1)

Entirely within `net_task.rs` + the request/dispatch/transfer async wrappers.
`request`/`dispatch`/`transfer` (sync) already exist from Phase 0-5, so this
has no A1 dependency.

- [x] `client/net_task.rs`: `XrdsNetTask<T>` (`rx` + `done` flag) with `spawn`
      (detached worker, no `JoinHandle`) and `try_take() -> Option<Result<T,
      NetError>>`. No `XrdsNetPoll`, no `is_ready`, **no `Drop` impl** — drop
      must not block
- [x] `client/net_task.rs`: `NetTaskSlot<T>` trait + impl on
      `Option<XrdsNetTask<T>>` (`take_ready()` — returns the owned result once
      and clears the slot to `None`)
- [x] `net_intent.rs`: `XrdsNet::request_async`/`dispatch_async`/
      `transfer_async` (the ones that don't involve `EventStream`)
- [x] `net_intent.rs` / sync verbs: doc note on `request`/`dispatch`/`listen`/
      `transfer` that inside `XrdsApp` the `_async`/feed forms are the
      frame-safe ones (the sync forms block the caller)
- [x] `client/mod.rs` + `lib.rs`: re-export `XrdsNetTask`, `NetTaskSlot`,
      **and `NetResponse`** (so a request round-trip imports from one
      `xrds::net::...` path, not two)
- [x] Unit tests: `try_take()` returns `None` while pending (and returns
      *immediately* — non-blocking), `Some(owned)` exactly once on completion,
      `None` on every call after that (spent); a worker that dies without
      sending surfaces one `Err` then `None`; **dropping a still-pending task
      returns immediately and doesn't panic** (the no-block-on-drop
      guarantee); `Option::take_ready()` clears the slot on completion; plus a
      deterministic `request_async` end-to-end test (bad scheme → error
      delivered through the task, no network)

### Phase A3 — Streaming async glue (depends on A1 + A2)

The convergence point: the async listen constructors return
`XrdsNetTask<EventStream>` (needs A2's task + A1's bounded `listen_with`), and
`NetFeed` wraps that task + drains via A1's `EventStream::try_recv`.

- [x] `net_intent.rs`: `XrdsNet::listen_async` + `listen_with_async`
      (`XrdsNetTask<EventStream>`)
- [x] `client/net_feed.rs`: `NetFeed` (internal `FeedState`
      Connecting/Streaming/Ended) with `try_recv()`, `take_error()`,
      `close()` — hides the connect→stream handshake
- [x] `net_intent.rs`: `XrdsNet::listen_feed(url)` +
      `listen_feed_with(url, opts)` (construct a `NetFeed`)
- [x] `client/mod.rs` + `lib.rs`: re-export `NetFeed`
- [x] Unit tests (`NetFeed`): `try_recv()` yields `None` while the underlying
      task is still connecting, then drains events once it flips to streaming;
      a connect failure surfaces once via `take_error()` and then the feed is
      inert; `close()` is non-blocking in both the connecting and streaming
      states

### Phase A4 — Gate

- [x] `cargo test -p xrds-net --lib` — full suite (A1 + A2 + A3) still matches
      the established flaky baseline (127 passed; only the external-network
      flaky set — webrtc, file-download, the run-to-run `echo.websocket.org`
      WS tests — fails, none on the A1–A3 code paths)

### Phase B — Re-export through the DeviceSDK

- [x] `crates/xrds-runtime/src/lib.rs`: `#[cfg(not(target_os = "android"))]
      pub use xrds_net as net;`
- [x] Confirm `xrds::net::XrdsNet` resolves from the root crate — verified with
      a throwaway example importing `xrds::net::{XrdsNet, XrdsNetTask, NetFeed,
      NetTaskSlot, ListenOptions, Overflow, RequestOptions, NetResponse, …}`
      (`cargo check -p xrds --example …`, then removed)
- [x] Confirm the Android build target still excludes it — `cargo tree
      -p xrds-runtime -i xrds-net` shows it on the host but
      `--target aarch64-linux-android` prints "nothing to print" (excluded),
      so no curl/quiche/webrtc/ffmpeg leak into the Android graph

### Phase C — Example + docs

- [x] New example showing the recommended in-app surface from inside
      `XrdsApp::setup`/`update`: a one-shot via `Option<XrdsNetTask>::
      take_ready()` and a stream via `NetFeed` —
      [`examples/net_app.rs`](../examples/net_app.rs)
- [x] `examples/README.md` entry (added `net_app.rs` as the in-app path;
      re-labeled `net_intent.rs` as the standalone/synchronous one; dropped the
      superseded `net_bevy.rs` stub from the table)
- [x] Decide and note `plugin_net_bevy.rs`'s fate — **removed** (module +
      `bevy_plugin` feature + optional `bevy` dep + the `net_bevy.rs` stub);
      `xrds-net` is now Bevy-free (see "`plugin_net_bevy.rs`: removed" above)
- [x] This doc's `**Status:**` line updated once shipped
