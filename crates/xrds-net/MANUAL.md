# xrds-net — Reference Manual

`xrds-net` is the OpenXRDS DeviceSDK's networking crate. It provides
**protocol-agnostic client networking** that integrates into a synchronous
XR application loop without an async runtime.

This manual is the complete reference: design, the full public API (both
layers), the protocol/capability matrix, DeviceSDK integration, platform
support, and non-goals. For a quickstart, see [README.md](./README.md).

---

## 1. Overview and scope

An application developer expresses **intent** — "get me a reply", "send this,
don't wait", "give me an ongoing feed", "move this file" — as one of four verbs
plus a URL. The crate infers the wire protocol from the URL scheme and executes
it. The developer never selects a protocol, writes `.await`, or registers an
engine plugin.

**Target:** a *client* application (an XR app, a CLI tool, the GUI editor)
making a modest number of network calls from a synchronous loop.

**Explicit non-targets** (see §14):

- High-concurrency **servers** — the model uses one background OS thread per
  in-flight operation; it is not an async, many-connections-per-thread design.
- Game-state / entity **replication** — this is a byte-transport crate, not a
  sync/prediction framework.
- Media **decode / playback / presentation** — that belongs to `xrds-media`
  and the runtime's `XrdsAPI`; this crate delivers the encoded bytes only.

---

## 2. Design philosophy

Four choices define the crate; each is a deliberate trade for approachability
and engine-fit over generality and scale.

1. **Protocol-agnostic at the intent layer.** `XrdsNet::request("mqtt://…")`
   and `request("https://…")` are the same call; the scheme picks the
   protocol. Switching transports is a URL change, not a code change.
2. **Synchronous, poll-based — not async-first.** Blocking work runs on a
   background thread; the app polls a non-blocking handle (`try_take` /
   `try_recv`) once per frame. No `tokio`/executor in the application. This
   mirrors the DeviceSDK's existing "kick off slow work, poll a status each
   frame" idiom (e.g. `XrdsAPI::gltf_load_status`).
3. **Engine-agnostic core.** The crate has **no Bevy dependency**. Integration
   is "the app holds a value and polls it" — the dependency points from the app
   *into* the crate, never the reverse. The identical crate runs in a CLI, a
   test, the editor, and the XR runtime.
4. **Transport only.** It moves bytes. Decode, presentation, and replication
   are out of scope by design, keeping the surface small and composable.

---

## 3. Architecture: two layers on one mechanism

There are two public surfaces, sharing one internal mechanism.

- **Intent layer — `XrdsNet`** (primary). Four verbs (`request`, `dispatch`,
  `listen`, `transfer`) plus their frame-safe `_async` forms and the ergonomic
  `NetFeed`. This is what app code should use.
- **Session layer — `ClientBuilder` / `Client`** (expert). A protocol-aware,
  chainable client (`set_protocol`, `.connect()`/`.send()`/`.rcv()`/`.close()`,
  `.request()`, plus per-protocol extras like `run_ftp_command` and
  `mqtt_subscribe`). Use it when you need lower-level control. `XrdsNet` is
  built on top of it.

Internally both layers dispatch through a per-protocol `ProtocolHandler`
selected by a single factory. Protocols advertise their shape via capability
queries (request-shaped, stream-shaped, or file-transfer-shaped) rather than a
flat "every protocol implements every method" trait, so an unsupported verb is
a structured error, not a silent no-op. (The `ProtocolHandler` trait itself is
internal; it is documented in
[`docs/done/xrds-net-protocol-handler.md`](../../docs/done/xrds-net-protocol-handler.md).)

---

## 4. Intent API — `XrdsNet`

All verbs are associated functions on the unit struct `XrdsNet`. The plain
forms are **blocking**; the `_async` forms run on a background thread and
return a poll-able handle (§8).

### request — one-shot, expects a reply

```rust
fn request(url: &str, opts: RequestOptions) -> Result<NetResponse, NetError>
fn request_async(url: &str, opts: RequestOptions) -> XrdsNetTask<NetResponse>
```

HTTP/HTTPS/FILE/CoAP always support it; HTTP/3 via the expert override
(§10, §11); MQTT only as a declared opt-in (see the capability matrix, §12).

### dispatch — one-shot, fire-and-forget

```rust
fn dispatch(url: &str, payload: Vec<u8>) -> Result<(), NetError>
fn dispatch_async(url: &str, payload: Vec<u8>) -> XrdsNetTask<()>
```

Stream-shaped protocols only (MQTT publish, WS/QUIC send). The URL **path** is
the topic (`mqtt://broker/sensors/temp` → topic `sensors/temp`); topic-less
transports ignore it.

### listen — ongoing feed

```rust
fn listen(url: &str) -> Result<EventStream, NetError>
fn listen_with(url: &str, opts: ListenOptions) -> Result<EventStream, NetError>
fn listen_async(url: &str) -> XrdsNetTask<EventStream>
fn listen_with_async(url: &str, opts: ListenOptions) -> XrdsNetTask<EventStream>
fn listen_feed(url: &str) -> NetFeed
fn listen_feed_with(url: &str, opts: ListenOptions) -> NetFeed
```

Stream-shaped protocols (WS/WSS, raw QUIC, MQTT subscribe; future MoQ).
Returns an [`EventStream`](#7-streaming-eventstream-and-backpressure) (or, for
the frame-loop path, a [`NetFeed`](#netfeed--streaming-without-the-handshake)).
For MQTT the URL path is the subscription topic. `ListenOptions` (§7) tunes the
buffer and overflow policy — important for video-rate feeds.

### transfer — bulk file operation

```rust
fn transfer(url: &str, op: TransferOp) -> Result<TransferResult, NetError>
fn transfer_async(url: &str, op: TransferOp) -> XrdsNetTask<TransferResult>
```

FTP/SFTP. The URL **path** is the remote file/directory path. Credentials
travel as URL userinfo (`ftp://user:pass@host:21/path`); absent credentials
produce `NetError::MissingInput` (§6).

### open — bidirectional session

```rust
fn open(url: &str) -> Result<NetChannel, NetError>
fn open_async(url: &str) -> XrdsNetTask<NetChannel>
```

A **session** is a distinct shape from the four verbs above: **one connection
you both send on and read replies from** (a persistent control/data channel),
rather than the pub/sub `dispatch`/`listen` pair (two separate,
broker-mediated connections). It's what WS and raw QUIC are natively for.

Supported by session-capable protocols: **`quic://`** (poll-based) and
**`ws://` / `wss://`** (a `tokio-tungstenite` backend; `wss` over rustls).
`open` on a non-session protocol returns `NetError::Capability`.

The returned `NetChannel` is non-blocking and frame-loop-friendly:

```rust
impl NetChannel {
    fn send(&mut self, data: Vec<u8>) -> Result<(), NetError>;
    fn try_recv(&mut self) -> Option<Event>;         // None = nothing this poll
    fn take_error(&mut self) -> Option<NetError>;     // a poll error, surfaced once
    fn recv_timeout(&mut self, timeout: Duration) -> Result<Event, NetError>;
    fn close(self) -> Result<(), NetError>;
}
```

```rust
let mut chan = XrdsNet::open("quic://host:443/")?;
chan.send(b"hello".to_vec())?;
while let Some(ev) = chan.try_recv() { /* ev.payload */ }
```

Inside `XrdsApp`, use `open_async` and hold the resulting
`XrdsNetTask<NetChannel>` / `NetChannel` as a field (§8, §9).

---

## 5. Option and result types

```rust
// Request configuration. `RequestOptions::get()` is the no-frills GET.
pub struct RequestOptions {
    pub method: Option<String>,        // e.g. "POST"
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout: Option<u64>,
}

// transfer() input
pub enum TransferOp { Upload(Vec<u8>), Download, List, Delete }

// transfer() output
pub enum TransferResult { Uploaded, Downloaded(Vec<u8>), Listed(Vec<String>), Deleted }

// One message from a listen stream.
pub struct Event { pub topic: Option<String>, pub payload: Vec<u8> }

// request()/Client::request() result.
pub struct NetResponse {
    pub protocol: PROTOCOLS,
    pub status_code: u32,             // HTTP status; CoAP uses its own codes
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub error: Option<String>,        // per-protocol soft error (transport ok)
}
```

`Event.topic` is `Some` for topic-addressed transports (MQTT, future MoQ) and
`None` for topic-less ones (WS, raw QUIC).

---

## 6. Errors — `NetError`

Every fallible call returns `Result<_, NetError>` (implements
`std::error::Error` + `Display`):

```rust
pub enum NetError {
    /// A URL scheme with no known protocol mapping.
    UnrecognizedScheme(String),
    /// The protocol does not support this verb, or supports it only as an
    /// unmet opt-in (e.g. request over MQTT). Never a hang — a clear "no".
    Capability { protocol: PROTOCOLS, verb: &'static str, detail: String },
    /// A required input is missing before the op can even be attempted; the
    /// `hint` says what to provide (e.g. FTP credentials). Guided error.
    MissingInput { protocol: PROTOCOLS, field: &'static str, hint: String },
    /// Underlying I/O / transport failure (socket, DNS, TLS, …).
    Network(String),
    /// Server / protocol-level rejection (auth failed, broker refused, …).
    Protocol(String),
}
```

`Capability` and `MissingInput` are the ones worth matching on — they are the
difference between "this protocol can't do that" and "you forgot something",
programmatically, not just in a message string.

---

## 7. Streaming: `EventStream` and backpressure

`listen`/`listen_with` return an `EventStream` — a background thread reads from
the connection and buffers `Event`s; the consumer drains without blocking the
frame loop.

```rust
impl EventStream {
    fn try_recv(&self) -> Option<Event>;                       // non-blocking
    fn recv_timeout(&self, timeout: Duration) -> Result<Event, NetError>;
    fn close(self);                                            // stop + join
}
impl Iterator for EventStream { /* next() blocks until an event or end */ }
```

The buffer is **bounded** — this is what keeps a fast producer (e.g. a video
feed) from growing memory without bound when the consumer falls behind. The
policy is chosen per stream:

```rust
pub struct ListenOptions { pub buffer: usize, pub overflow: Overflow }
pub enum Overflow {
    Block,       // producer waits for space → TCP flow control throttles the
                 // sender; lossless. DEFAULT (buffer: 256).
    DropOldest,  // producer drops the oldest buffered event; never waits.
                 // Bounds memory *and* latency; right for live video.
}
impl Default for ListenOptions { /* { buffer: 256, overflow: Block } */ }
```

A live-video consumer uses e.g. `ListenOptions { buffer: 4, overflow:
Overflow::DropOldest }` — a shallow queue always holding the freshest frames. A
lossless (VOD/telemetry) consumer keeps the default.

---

## 8. Frame-loop layer: tasks and feeds

The plain verbs block; these types make them frame-safe.

### `XrdsNetTask<T>` — a poll-able one-shot handle

Returned by `request_async` / `dispatch_async` / `transfer_async` /
`listen_async`.

```rust
impl<T> XrdsNetTask<T> {
    /// Non-blocking. None = still running. Some(_) = finished (owned result,
    /// no clone); the task is then spent (later calls return None).
    fn try_take(&mut self) -> Option<Result<T, NetError>>;
}
```

- **Dropping a pending task never blocks** — the worker is detached (there is
  no `Drop`/join). Cancelling a request or discarding a fire-and-forget
  `dispatch_async` is instant.
- The task is **`Send + Sync`** (an `XrdsApp` holding one must be, for
  `Runtime::run_xrds`). Internally the receiver is `Mutex`-wrapped purely to
  satisfy that bound.
- **One OS thread per call.** Right for one-shots and long-lived `listen`s; do
  *not* call an `_async` verb every frame. For a repeating feed use one
  `listen`/`NetFeed`; for periodic polling keep the single in-flight task until
  it resolves.

### `NetTaskSlot::take_ready()` — one-shot without the bookkeeping

An extension trait on `Option<XrdsNetTask<T>>`:

```rust
trait NetTaskSlot<T> {
    /// Some(_) exactly once when the task finishes — and the slot is reset to
    /// None for you. None while pending / already taken / empty.
    fn take_ready(&mut self) -> Option<Result<T, NetError>>;
}
```

Collapses "poll, handle, null the field" into `if let Some(r) =
self.field.take_ready() { … }`.

### `NetFeed` — streaming without the handshake

Returned by `listen_feed` / `listen_feed_with`. Hides the two-stage
`connect → stream` handshake behind one value the app holds and drains.

```rust
impl NetFeed {
    fn try_recv(&mut self) -> Option<Event>;   // None while connecting or idle
    fn take_error(&mut self) -> Option<NetError>; // connect/subscribe failure, once
    fn close(self);                            // non-blocking in either state
}
```

---

## 9. DeviceSDK integration

Within the DeviceSDK the crate is re-exported as **`xrds::net`** (via
`xrds_runtime`), so app code needs no direct Cargo dependency. It is available
on **desktop and Android** (Quest 3/Pro); only the FTP *server* and WebRTC are
desktop-only. Standalone, depend on `xrds-net` and use `xrds_net::…`.

Recommended in-app pattern (see [`examples/net_app.rs`](../../examples/networking/net_app.rs)):

```rust
use xrds::net::{XrdsNet, XrdsNetTask, NetTaskSlot, NetFeed, RequestOptions, NetResponse};

struct MyApp { req: Option<XrdsNetTask<NetResponse>>, feed: Option<NetFeed> }

impl XrdsApp for MyApp {
    fn setup(&mut self, _api: &mut XrdsAPI) {
        self.req  = Some(XrdsNet::request_async("https://…", RequestOptions::get()));
        self.feed = Some(XrdsNet::listen_feed("mqtt://broker/telemetry"));
    }
    fn update(&mut self, _ctx: &mut XrdsUpdateContext) {
        if let Some(r) = self.req.take_ready() { /* handle once */ }
        if let Some(f) = &mut self.feed {
            while let Some(ev) = f.try_recv() { /* ev.payload */ }
            if let Some(e) = f.take_error() { self.feed = None; }
        }
    }
}
```

Because `XrdsApp: Send + Sync + 'static`, any handle held as an app field must
be `Send + Sync` — `XrdsNetTask`, `NetFeed`, and `EventStream` all are.

---

## 10. Expert session API — `ClientBuilder` / `Client`

Protocol-aware, chainable. `XrdsNet` is built on this; reach for it directly
when you need control the intent verbs don't expose. `Client` is intentionally
**not `Clone`** (a client owns a live connection).

```rust
// Builder
ClientBuilder::new() -> ClientBuilder
    .set_protocol(PROTOCOLS) -> Self
    .set_user(&str) -> Self
    .set_password(&str) -> Self
    .build() -> Client
ClientBuilder::from_url(&str) -> Result<Client, NetError>   // infer protocol from scheme

// Client — configuration (chainable, consume+return Self)
.set_method(&str) / .set_url(&str) / .set_follow_redirect(bool)
.set_req_headers(Vec<(&str,&str)>) / .set_req_body(&str) / .set_timeout(u64)
.set_user(&str) / .set_password(&str)

// Client — introspection
.get_protocol() -> PROTOCOLS
.get_id() -> String
.get_mqtt_connection() -> Option<Arc<Mutex<rumqttc::Connection>>>   // MQTT only

// Client — operations
.connect(self) -> Result<Self, NetError>          // stream/file protocols
.send(self, Vec<u8>, Option<&str>) -> Result<Self, NetError>  // topic optional
.rcv(&mut self) -> Result<Vec<u8>, NetError>
.close(&mut self) -> Result<(), NetError>
.request(self) -> Result<NetResponse, NetError>   // request protocols
.mqtt_subscribe(self, &str) -> Result<Self, NetError>          // MQTT only
.run_ftp_command(&self, FtpPayload) -> FtpResponse             // FTP only
```

FTP raw-command types:

```rust
pub struct FtpPayload { pub command: FtpCommands, pub payload_name: String, pub payload: Option<Vec<u8>> }
pub struct FtpResponse { pub payload: Option<Vec<u8>>, pub error: Option<String> }
pub enum FtpCommands { CWD, CDUP, QUIT, RETR, STOR, APPE, DELE, RMD, MKD, PWD, LIST, NOOP }
```

Calling the wrong verb for a protocol's shape (e.g. `.connect()` on an
HTTP client) yields a specific `NetError::Capability` ("protocol is
request-only — use `.request()` instead").

---

## 11. Protocols and schemes

```rust
pub enum PROTOCOLS { HTTP, HTTPS, FILE, COAP, MQTT, FTP, SFTP, WS, WSS, WEBRTC, HTTP3, QUIC }
```

Scheme → protocol inference (used by `from_url` and every `XrdsNet` verb):

| Scheme | Protocol |
| --- | --- |
| `http` | HTTP |
| `https` | HTTPS |
| `file` | FILE |
| `coap` | COAP |
| `ws` | WS |
| `wss` | WSS |
| `mqtt` | MQTT |
| `ftp` | FTP |
| `sftp` | SFTP |
| `quic` | QUIC (SDK convention — raw QUIC channel) |

Notes:

- **HTTP/3 has no scheme.** `https://` maps to HTTPS (HTTP/1.1). HTTP/3 is
  reachable only via the expert override `ClientBuilder::new().set_protocol(
  PROTOCOLS::HTTP3)` until real ALPN negotiation exists.
- **Ports default per scheme** when omitted (an explicit `:port` always
  overrides): `http`/`ws` → 80, `https`/`wss` → 443, `ftp` → 21, `sftp` → 22,
  `mqtt` → 1883, `coap` → 5683, `quic` → 443. Unknown or scheme-less inputs
  fall back to 80. So `ftp://user:pass@host/file` and `mqtt://broker/topic`
  work without a port.
- Unrecognized schemes → `NetError::UnrecognizedScheme`.

---

## 12. Capability matrix

| Protocol | request | dispatch | listen | transfer | open (session) |
| --- | :-: | :-: | :-: | :-: | :-: |
| HTTP / HTTPS | ✅ | — | — | — | — |
| FILE | ✅ (byte GET) | — | — | — | — |
| CoAP | ✅ | — | — | — | — |
| HTTP/3 | ✅ (expert only) | — | — | — | — |
| WS / WSS | — | ✅ (send) | ✅ (recv loop) | — | ✅² |
| QUIC (raw) | — | ✅ (send) | ✅ (recv loop) | — | ✅ |
| MQTT | ⚠️ opt-in¹ | ✅ (publish) | ✅ (subscribe) | — | — |
| FTP / SFTP | — | — | — | ✅ | — |
| WebRTC | — | — | — | — | — (separate API, §13) |

¹ MQTT `request` requires broker/version support for request/response
correlation (e.g. MQTT 5 Response Topic / Correlation Data). It is a **declared
opt-in**, not a blanket promise: unsupported request-over-MQTT returns
`NetError::Capability` — a clear "no", never a silent hang. The correlation
mechanism itself is not implemented in this pass.

² `open` (bidirectional session, §4) is implemented for `quic://`, `ws://`,
and `wss://` (WS via a `tokio-tungstenite` backend; `wss` over rustls, which
validates against the bundled webpki-roots CA set).

Anything outside this matrix returns `NetError::Capability`.

---

## 13. WebRTC and media

WebRTC is **not** part of the intent verbs. It uses its own standalone
`WebRTCClient` API (session/offer/answer, `on_video_track`/`on_audio_track`
callbacks over raw RTP), a different threading model (its own async runtime and
jitter buffer). It is exported (`xrds_net::WebRTCClient`) and has its own guide:
**[MANUAL_WEBRTC.md](./MANUAL_WEBRTC.md)** (session/signaling/ICE flow,
publisher & subscriber usage, API reference). Wiring WebRTC into the DeviceSDK
is a future effort.

`xrds-net` performs **no codec work**. Media *sources* injected into WebRTC
(`AudioSource` / `VideoSource`, already encoded) are produced by `xrds-media`
(capture + `ffmpeg`/`opus` encoding under its `transcoding` feature). This
separation — device capture and codec encoding out of `xrds-net` — is described
in
[`docs/done/xrds-net-capture-decoupling.md`](../../docs/done/xrds-net-capture-decoupling.md).
Media *decode and playback* (encoded bytes → frames → a scene texture) are not
provided by any crate yet; that is `xrds-media` + `XrdsAPI` future work.

---

## 14. Platform support

| | Windows | Linux | macOS | Android |
| --- | :-: | :-: | :-: | :-: |
| Build & run | ✅ | ✅ (target) | planned | ✅ (Quest 3/Pro) |

**Android** (Quest 3/Pro, arm64, min API 32) is supported — HTTPS and `wss://`
are verified working on-device. Two caveats: the FTP **server** (`libunftp`) is
excluded there (an XR client never hosts FTP — dropped via
`--no-default-features`, see `docs/done/xrds-net-android-shipping.md`), and WebRTC
remains desktop-only. There is no Bevy dependency (removed) and no async
runtime is required of the consumer.

**TLS.** Everything except QUIC uses **rustls** (with the `ring` crypto
provider): HTTP/HTTPS (`reqwest`), `wss://` (`tokio-tungstenite`), FTPS
(`suppaftp`), MQTT (`rumqttc`), CoAP, WebRTC. QUIC/HTTP3 uses **BoringSSL**,
vendored inside `quiche` — it has no rustls option, so it's the one exception.
That's the whole story: two crypto backends, no OpenSSL, no libcurl, no
`native-tls`, nothing to provision on any platform. See
`docs/done/xrds-net-crypto-consolidation.md` for why.

Certificate validation uses the **webpki-roots** bundled Mozilla CA set (not
the OS trust store — more portable, and works identically on Android).
Connecting to a public TLS server needs no developer setup; a self-signed /
private-CA server is not currently supported for these protocols (there is no
`insecure`/custom-CA bypass — the `ClientContext::insecure` flag affects QUIC
only). rustls's process-wide crypto provider is installed automatically by the
crate on first use; callers don't need to do anything.

---

## 15. Non-goals / boundaries

- **Servers / high concurrency.** Thread-per-operation suits a client with a
  few streams, not thousands of connections. Use an async stack for that.
- **Replication / entity sync / prediction.** Out of scope — build atop this,
  or use a dedicated replication crate.
- **Media decode / playback / scene presentation.** `xrds-media` + `XrdsAPI`
  future work; this crate stops at delivering encoded bytes.
- **Connection pooling / reuse.** Each `XrdsNet` call is a fresh
  connect-operate-drop. Relevant if HLS/DASH-style segment pulling is ever
  targeted (a handshake per segment otherwise) — noted, not implemented.
- **WebRTC in the intent model.** Deliberately separate (§13).

See
[`docs/done/xrds-net-devicesdk-integration.md`](../../docs/done/xrds-net-devicesdk-integration.md)
for the integration design and its "Boundaries / non-goals" section.

---

## 16. Testing

```bash
cargo test -p xrds-net
```

The unit tests (buffer/backpressure, task/feed state machines, capability and
guided-error paths, scheme inference) are deterministic. The QUIC **session**
(`open`/`NetChannel`) also has a deterministic *live* round-trip: a `#[cfg(test)]`
raw-QUIC echo server (`server/quic_server.rs`, self-signed cert via the `rcgen`
dev-dependency) started in-process, with the client accepting the self-signed
cert via `ClientContext::insecure`.

A subset of integration tests hit **live public servers** —
`www.rust-lang.org`, `test.mosquitto.org`, `test.rebex.net`, `coap.me`,
`echo.websocket.org`, and public QUIC/WebRTC endpoints — and are inherently
**flaky** under network conditions; that flaky set (WebRTC, file-download, and
the public MQTT/WS/CoAP round-trips) is the known baseline, not a regression.
Compare against it rather than expecting an all-green run.

WebRTC's tests are genuine end-to-end integration tests (real signaling
server, real ICE/DTLS handshake) and live separately in
[`tests/webrtc_integration.rs`](../tests/webrtc_integration.rs) — see
`docs/done/xrds-net-webrtc-test-restructure.md` for why and how they're
structured (OS-assigned ports, polling instead of fixed sleeps, `#[serial]`
within that file since Cargo only isolates *files* into separate processes,
not functions within one). Pure logic that used to only be exercised
indirectly through those E2E round-trips — ICE server URL construction,
`WebRTCMessage` (de)serialization, session create/join/leave/list/close
bookkeeping, H.264 start-code validation — now has direct, real, sub-second
unit tests in the crate's own `#[cfg(test)]` modules instead. The slowest
WebRTC test (`test_client_webrtc_send_video_file`, a full real-time file
transfer, 60-120s) is `#[ignore]`d by default; run it explicitly with
`cargo test -p xrds-net --test webrtc_integration -- --ignored`.

---

## 17. Design documents

- [`docs/done/xrds-net-protocol-handler.md`](../../docs/done/xrds-net-protocol-handler.md)
  — the protocol-agnostic API + `ProtocolHandler` mechanism (intent verbs,
  `NetError`, capability matrix, expert-API refactor).
- [`docs/done/xrds-net-devicesdk-integration.md`](../../docs/done/xrds-net-devicesdk-integration.md)
  — the DeviceSDK integration (`XrdsNetTask`/`NetFeed`, bounded `EventStream`,
  `xrds::net` re-export), phase-by-phase.
- [`docs/done/xrds-net-capture-decoupling.md`](../../docs/done/xrds-net-capture-decoupling.md)
  — device capture + codec encoding moved to `xrds-media`.
