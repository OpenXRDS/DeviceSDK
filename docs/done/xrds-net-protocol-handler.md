# xrds-net: protocol-agnostic API (intent verbs) + ProtocolHandler mechanism

**Status: implemented (Phases 0-5 complete).** `XrdsNet`'s four intent verbs
are the primary networking path; `ClientBuilder`/`Client` remain as the
expert/session API both layers share the same `ProtocolHandler` mechanism
underneath. See the Implementation checklist below for what shipped in each
phase, and "Risks/notes" for gaps found along the way (some deliberately
deferred — see "History of this doc" for what changed and why the rewrite
happened in the first place).

## What "protocol agnostic" means for this SDK

**An XR application developer using this SDK should not have to think about
which wire protocol is in use at development time.** They express *intent*
("get me a reply" / "send this, don't wait" / "give me an ongoing feed" /
"move this file") and the SDK picks the mechanism. The developer should never
write `.set_protocol(PROTOCOLS::MQTT)`, and — this is the point this rewrite
adds — **they should not have to write `.connect()` vs `.request()` either**,
because that choice is still "which transport shape does this protocol have,"
not "what do I want."

## Two layers: intent verbs (primary) vs session API (expert)

This SDK now has two public surfaces, not one:

1. **`XrdsNet` — four intent verbs.** `request`, `dispatch`, `listen`,
   `transfer`. This is what the vast majority of XR application code should
   use, and it's the layer that actually satisfies the definition above.
2. **`ClientBuilder`/`Client` — the existing session API**, unchanged in
   public shape (`set_protocol`, `.connect()`, `.send()`, `.rcv()`, `.close()`,
   `.request()`, `run_ftp_command`, `mqtt_subscribe`, `get_mqtt_connection`).
   Kept for developers who deliberately want protocol-level control (same
   "expert escape hatch" role `RuntimeHandler` plays next to `XrdsAPI`
   elsewhere in this project). `XrdsNet` is built *on top of* this layer
   internally — none of the previously-planned `ProtocolHandler` mechanism
   work is wasted, it's the foundation both layers share.

## Why this rewrite happened: comparing against an independent design

A second design (`docs/PROTOCOL_AGNOSTIC_OPINION.md`) proposed categorizing
protocols by **developer intent** (`request`/`dispatch`/`listen`) rather than
by **transport mechanics** (connection-ful vs connection-less, the previous
draft of this doc). Comparing the two directly:

- **Their model is more agnostic, and that's the correct call.** In the
  previous draft, calling `.request()` on an MQTT-inferred `Client` failed
  with "connectionless-only, use `.connect()`" — the developer still had to
  know MQTT needs a connection to write correct code. `listen("mqtt://...")`
  and `listen("wss://...")` (and later `listen("moq://...")`) being the
  *literal same call* is a strictly stronger claim on "developer never thinks
  about protocol," and it's the actual goal, so it wins on that test.
- **Their model assumed an async-first API** (`Promise`/`AsyncIterable`),
  which looked like a much bigger lift than reorganizing internal dispatch.
  **This turned out to be avoidable**: this exact codebase already has a
  proven synchronous pattern for "ongoing stream of data" —
  `xrds-media`'s `Webcam::open()`/`Microphone::open_default()` both return a
  plain `std::sync::mpsc::Receiver`, consumed with blocking `.recv()` or a
  `for` loop. `listen()` reuses that pattern (see "`EventStream`" below)
  instead of requiring async. No async-first rewrite needed.
- **Their model had a real soundness gap**, adopted here as a fix rather than
  copied as-is: their capability matrix marked "request over `mqtt://`" as
  unconditionally supported via a "Correlation Engine" (embed a correlation ID,
  subscribe to a reply topic, wait for the match). That only works if the
  *other end* also implements the same convention — plain MQTT 3.1.1 has no
  such contract, so an uncooperative broker makes "simulated request"
  indistinguishable from a hang. (MQTT 5 does standardize this via Response
  Topic/Correlation Data properties, so it's not fabricated — just
  protocol-version/broker-dependent.) **Fix adopted here**: `request()` over a
  stream-shaped protocol is an explicit, declared, opt-in capability per
  protocol — never a blanket promise. See the capability matrix below, where
  MQTT's `request` column is marked "opt-in" not "yes."
- **Their structured `CapabilityError` idea is adopted directly** — see
  `NetError` below, replacing `Result<_, String>` throughout this refactor.

## The four intent verbs

```rust
pub struct XrdsNet; // namespacing only, matches XrdsAPI-style typed surface

impl XrdsNet {
    /// One-shot, wants a reply. HTTP/HTTPS/FILE/CoAP/HTTP3 always; MQTT only
    /// if the broker/protocol version declares request/response support
    /// (opt-in — see capability matrix).
    pub fn request(url: &str, opts: RequestOptions) -> Result<NetResponse, NetError>;

    /// One-shot, fire-and-forget — no reply awaited. MQTT publish, QUIC/WS
    /// send. The URL path is the topic (see "Topic addressing" below).
    pub fn dispatch(url: &str, payload: Vec<u8>) -> Result<(), NetError>;

    /// Ongoing feed. WS, MQTT subscribe, (future) MoQ subscribe. Returns a
    /// live handle — see `EventStream` below.
    pub fn listen(url: &str) -> Result<EventStream, NetError>;

    /// Bulk file operation. FTP/SFTP today. `TransferOp` is Upload/Download/
    /// List/Delete.
    pub fn transfer(url: &str, op: TransferOp) -> Result<TransferResult, NetError>;
}
```

```rust
// XR app developer's actual code:
let manifest = XrdsNet::request("https://api.example.com/manifest", RequestOptions::get())?;

XrdsNet::dispatch("mqtt://broker.local/sensors/temp", reading_bytes)?;

for event in XrdsNet::listen("mqtt://broker.local/commands")? {
    handle_command(event.payload);
}
// Zero code changes needed to move this feed to MoQ later — just the URL scheme.
for event in XrdsNet::listen("moq://relay.example.com/commands")? { ... }

XrdsNet::transfer("ftp://files.example.com/scans/room.glb",
    TransferOp::Upload(scan_bytes))?;
```

### Topic addressing: the URL path *is* the topic/file-path

This is what makes `dispatch`/`listen` genuinely uniform across MQTT/WS/QUIC/
(future MoQ) without a separate "give me a topic" parameter: the URL's path
component is the topic (`mqtt://broker/sensors/temp` → topic `sensors/temp`)
or the remote file path (`ftp://host/reports/q3.csv` → path `reports/q3.csv`).
Topic-less transports (WS, raw QUIC) just always report `topic: None` on
`Event`. One addressing scheme, no protocol-specific parameter shapes.

### `RequestOptions` / `TransferOp` / `TransferResult` / `Event`

```rust
pub struct RequestOptions {
    pub method: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout: Option<u64>,
}
impl RequestOptions {
    pub fn get() -> Self { .. } // convenience constructor for the common case
}

pub enum TransferOp {
    Upload(Vec<u8>),
    Download,
    List,
    Delete,
}
pub enum TransferResult {
    Uploaded,
    Downloaded(Vec<u8>),
    Listed(Vec<String>),
    Deleted,
}

pub struct Event {
    pub topic: Option<String>,
    pub payload: Vec<u8>,
}
```

## `NetError`: structured, not stringly-typed

Adopted from the comparison doc's `CapabilityError` idea, generalized to cover
both failure sources identified in the earlier "guided-error validation" plan
(kept, see below):

```rust
pub enum NetError {
    /// fetch/listen/etc. given a scheme with no known protocol mapping.
    UnrecognizedScheme(String),
    /// The protocol fundamentally does not support the requested verb
    /// (e.g. `request()` on FTP), or supports it only as a declared opt-in
    /// that isn't met (e.g. `request()` on an MQTT broker with no
    /// request/response support declared).
    Capability { protocol: PROTOCOLS, verb: &'static str, detail: String },
    /// Required input is missing before the operation can even be attempted
    /// (e.g. FTP credentials) — this is the "guided error" case: the message
    /// must say what to fill in.
    MissingInput { protocol: PROTOCOLS, field: &'static str, hint: String },
    /// Underlying I/O/transport failure.
    Network(String),
    /// Server/protocol-level rejection (auth failed, broker refused, ...).
    Protocol(String),
}
impl std::fmt::Display for NetError { /* .. */ }
impl std::error::Error for NetError {}
```

Used consistently top-to-bottom: `ProtocolHandler` trait methods, the category
traits, and `Client`'s public methods all return `Result<_, NetError>` now
(previously sketched as `Result<_, String>` — upgraded here, since this is
exactly where the capability/missing-input distinction needs to be
programmatically checkable, not just human-readable).

## Capability matrix

Documents which of the four verbs each protocol actually supports — and,
importantly, makes the MQTT `request` opt-in explicit instead of an implicit
blanket promise (the gap identified in the comparison above).

| Protocol | `request` | `dispatch` | `listen` | `transfer` |
| --- | --- | --- | --- | --- |
| HTTP / HTTPS | ✅ | — | — | — |
| FILE | ✅ (byte-blob GET) | — | — | — |
| CoAP | ✅ | — | — | — |
| HTTP3 | ✅ | — | — | — |
| WS / WSS | — | ✅ (send) | ✅ (recv loop) | — |
| QUIC (raw channel) | — | ✅ (send) | ✅ (recv loop) | — |
| MQTT | ⚠️ opt-in — only if broker/version declares request/response support (e.g. MQTT 5 Response Topic/Correlation Data); otherwise `Capability` error, not a hang | ✅ (publish) | ✅ (subscribe) | — |
| FTP / SFTP | — | — | — | ✅ |
| WebRTC | — | — | — | — |

Everything else returns `NetError::Capability`. Implementing MQTT 5's actual
opt-in request/response support is **out of scope for this pass** — the
capability *declaration mechanism* (so it fails clearly instead of hanging) is
in scope; the correlation engine itself is future work.

## `EventStream`: `listen()`'s return type — reuses an established project pattern

```rust
pub struct EventStream {
    rx: std::sync::mpsc::Receiver<Event>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Iterator for EventStream {
    type Item = Event;
    fn next(&mut self) -> Option<Event> { self.rx.recv().ok() }
}

impl EventStream {
    pub fn recv_timeout(&self, d: std::time::Duration) -> Result<Event, NetError> { .. }
    pub fn close(mut self) { /* signal shutdown, join worker — no arbitrary sleep */ }
}
```

This is a **blocking, synchronous** stream (`for event in stream { .. }` or
`.recv_timeout(..)`) — no async runtime required. It's the same
background-thread-plus-shutdown-flag-plus-channel shape already proven twice
in this codebase: `xrds-media`'s `Webcam`/`Microphone` (device capture) and
`xrds-net`'s own `AudioTrackWriter` bridge (capture/encoding decoupling work).
Third use of the same idiom, not a new pattern.

## `ProtocolHandler` mechanism (shared by both layers)

Simpler than the previous draft's design (which had a flat trait with five
default-erroring methods, later complicated further by a connection-ful/
connection-less enum split). Once verbs are intent-shaped, most protocols
support **exactly one** of three fundamental shapes — request, stream
(send/recv, optionally topic-addressed), or file-transfer — so capability
queries replace default-erroring stubs, and the connection-ful/connection-less
distinction dissolves into "how a `StreamHandler`/`FileTransferHandler`
implementation privately manages its own connection," which callers never see:

```rust
// client/handler.rs
pub trait ProtocolHandler: Send {
    fn validate(&self, ctx: &ClientContext) -> Result<(), NetError> { Ok(()) }

    /// Request-shaped protocols (HTTP/HTTPS/FILE/CoAP/HTTP3) implement this
    /// directly. Everyone else's default declares the capability absent.
    fn request(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        Err(NetError::Capability {
            protocol: ctx.protocol, verb: "request",
            detail: "protocol does not support request/response".into(),
        })
    }

    /// Capability queries — the mechanism `XrdsNet::dispatch/listen/transfer`
    /// use internally to realize an intent verb without matching on
    /// `PROTOCOLS` themselves. `None` means "this protocol doesn't have this
    /// shape," produced by `Client`/`XrdsNet` as a `NetError::Capability`.
    fn as_stream(&mut self) -> Option<&mut dyn StreamHandler> { None }
    fn as_file_transfer(&mut self) -> Option<&mut dyn FileTransferHandler> { None }

    /// Escape hatch for concrete-type-only extras (FTP raw commands, MQTT's
    /// raw connection handle) — see "Expert-only extras" below.
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// WS, QUIC, MQTT today; MoQ later. Topic is `None` for topic-less
/// transports (WS, raw QUIC) — one trait covers both shapes.
pub trait StreamHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
    fn send(&mut self, ctx: &ClientContext, topic: Option<&str>, data: Vec<u8>) -> Result<(), NetError>;
    fn recv(&mut self, ctx: &ClientContext) -> Result<Event, NetError>;
    fn close(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
}

/// FTP/SFTP today.
pub trait FileTransferHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError>;
    fn upload(&mut self, ctx: &ClientContext, path: &str, data: Vec<u8>) -> Result<(), NetError>;
    fn download(&mut self, ctx: &ClientContext, path: &str) -> Result<Vec<u8>, NetError>;
    fn list(&mut self, ctx: &ClientContext, path: &str) -> Result<Vec<String>, NetError>;
    fn delete(&mut self, ctx: &ClientContext, path: &str) -> Result<(), NetError>;
}
```

**`ClientContext`** bundles the request/config state that currently lives on
`Client` and that handlers read: `protocol`, `raw_url`, parsed `url`/`host`/
`port`/`path`, `method`, `req_headers`, `req_body`, `timeout`, `redirection`,
`user`, `password`, `id`. Handlers stay stateless w.r.t. *config*; they only
own their *connection* state.

**The factory is the only place left that matches over all protocols:**

```rust
// client/handler.rs
pub(crate) fn create_handler(protocol: PROTOCOLS) -> Box<dyn ProtocolHandler> {
    match protocol {
        PROTOCOLS::HTTP | PROTOCOLS::HTTPS | PROTOCOLS::FILE => Box::new(HttpHandler::new()),
        PROTOCOLS::COAP => Box::new(CoapHandler::new()),
        PROTOCOLS::HTTP3 => Box::new(Http3Handler::new()),
        PROTOCOLS::QUIC => Box::new(QuicHandler::new()),
        PROTOCOLS::WS | PROTOCOLS::WSS => Box::new(WsHandler::new()),
        PROTOCOLS::FTP | PROTOCOLS::SFTP => Box::new(FtpHandler::new()),
        PROTOCOLS::MQTT => Box::new(MqttHandler::new()),
        PROTOCOLS::WEBRTC => Box::new(UnsupportedHandler::new(protocol)),
    }
}
```

This single `match` is exactly where a future Cargo-feature gate slots in
(one `#[cfg(feature = "…")]` per arm, falling back to `UnsupportedHandler`) —
without touching `Client`, `XrdsNet`, `PROTOCOLS`, or any consumer. See
"Cargo features are a deployment concern, not a philosophy" below.

## `XrdsNet`'s intent verbs, implemented on the mechanism above

```rust
impl XrdsNet {
    pub fn listen(url: &str) -> Result<EventStream, NetError> {
        let mut client = ClientBuilder::from_url(url)?;
        match client.handler_mut().as_stream() {
            Some(stream) => EventStream::spawn(stream, client.ctx().clone()), // background recv() loop, see EventStream above
            None => Err(NetError::Capability {
                protocol: client.protocol(), verb: "listen",
                detail: "protocol has no ongoing-stream capability".into(),
            }),
        }
    }

    pub fn dispatch(url: &str, payload: Vec<u8>) -> Result<(), NetError> {
        let mut client = ClientBuilder::from_url(url)?;
        let topic = topic_from_path(&client.ctx().path); // URL path -> topic, see above
        match client.handler_mut().as_stream() {
            Some(stream) => { stream.connect(client.ctx())?; stream.send(client.ctx(), topic.as_deref(), payload) }
            None => Err(NetError::Capability { protocol: client.protocol(), verb: "dispatch", detail: "..".into() }),
        }
    }

    pub fn transfer(url: &str, op: TransferOp) -> Result<TransferResult, NetError> {
        let mut client = ClientBuilder::from_url(url)?;
        let path = client.ctx().path.clone();
        match client.handler_mut().as_file_transfer() {
            Some(ft) => { ft.connect(client.ctx())?; run_transfer_op(ft, client.ctx(), &path, op) }
            None => Err(NetError::Capability { protocol: client.protocol(), verb: "transfer", detail: "..".into() }),
        }
    }

    pub fn request(url: &str, opts: RequestOptions) -> Result<NetResponse, NetError> {
        let client = ClientBuilder::from_url(url)?.apply(opts); // set_method/set_req_body/set_req_headers
        client.handler().request(client.ctx())
    }
}
```

No protocol match anywhere in `XrdsNet` — every verb is realized generically
via the capability queries. **v1 scope note**: each call is a fresh
connect-then-operate-then-(implicitly)-drop; no connection pooling/reuse
across repeated `dispatch`/`request`/`transfer` calls to the same endpoint.
That's a real, valuable future optimization (the comparison doc's "implicit
connection pool" idea) but out of scope here — see "Out of scope."

## `Client` (expert/session API) after the refactor

Public shape is **unchanged except the removed `Clone` derive** (see next
section) — same method names, signatures, and builder chaining developers
already rely on. Internals now delegate through the capability-query
mechanism instead of a flat five-method defaulted trait:

```rust
pub struct Client {
    ctx: ClientContext,
    handler: Box<dyn ProtocolHandler>,  // not Clone — see "Drop Client: Clone"
}

impl Client {
    pub fn connect(mut self) -> Result<Self, NetError> {
        self.ctx.parse_url_into_self()?;
        self.handler.validate(&self.ctx)?;
        match self.handler.as_stream().or(/* file-transfer also has connect */ None) {
            Some(stream) => { stream.connect(&self.ctx)?; Ok(self) }
            None => match self.handler.as_file_transfer() {
                Some(ft) => { ft.connect(&self.ctx)?; Ok(self) }
                None => Err(NetError::Capability {
                    protocol: self.ctx.protocol, verb: "connect",
                    detail: "protocol is request-only — use .request() instead".into(),
                }),
            }
        }
    }
    pub fn send(mut self, data: Vec<u8>, topic: Option<&str>) -> Result<Self, NetError> {
        match self.handler.as_stream() {
            Some(s) => { s.send(&self.ctx, topic, data)?; Ok(self) }
            None => Err(NetError::Capability { protocol: self.ctx.protocol, verb: "send", detail: "..".into() }),
        }
    }
    pub fn rcv(&mut self) -> Result<Vec<u8>, NetError> {
        match self.handler.as_stream() {
            Some(s) => s.recv(&self.ctx).map(|e| e.payload),
            None => Err(NetError::Capability { protocol: self.ctx.protocol, verb: "rcv", detail: "..".into() }),
        }
    }
    pub fn close(&mut self) -> Result<(), NetError> { /* as_stream().close(), or Ok(()) if not applicable */ }
    pub fn request(self) -> Result<NetResponse, NetError> {
        self.ctx.parse_url_into_self()?;
        self.handler.validate(&self.ctx)?;
        self.handler.request(&self.ctx)
    }
    // run_ftp_command / get_mqtt_connection: concrete-type downcast via
    // as_any() — deliberately protocol-aware expert-only escape hatches, see
    // "Expert-only extras" below.
}
```

Calling the wrong verb for a protocol's shape (e.g. `.connect()` on an
HTTP-built `Client`) now produces a specific, useful message ("protocol is
request-only — use `.request()` instead") as a natural side effect of the
capability query, not a generic "unsupported."

## Expert-only extras: FTP raw commands, MQTT raw connection handle

`run_ftp_command(FtpPayload)` (the full `FtpCommands` surface — `CWD`/`CDUP`/
`PWD`/`NOOP`/…) and `get_mqtt_connection()` (raw `rumqttc` handle access) stay
exactly as they are today: inherent methods on `Client` reached via
concrete-type downcast (`self.handler.as_any().downcast_ref::<FtpHandler>()`).
These are **deliberately** protocol-aware — a developer calling
`run_ftp_command` already knows they're using FTP specifically, matching the
framing that motivated keeping this a separate, narrower escape hatch rather
than folding it into `FileTransferHandler`'s generic four verbs.

## Guided-error validation (unchanged from previous draft, now returns `NetError`)

The chaining design means input arrives incrementally, and two distinct
failure sources need consistent, actionable reporting:

1. **Missing required input** — e.g. FTP's `connect()` needs `user`/
   `password`. Known *before* attempting the network operation.
2. **Server/protocol-level rejection** — e.g. FTP login rejected. Only known
   after attempting the operation.

`ProtocolHandler::validate(&self, ctx) -> Result<(), NetError>` (default
`Ok(())`) runs before the handler's actual verb, returning
`NetError::MissingInput` with a concrete hint:

```rust
// FtpHandler
fn validate(&self, ctx: &ClientContext) -> Result<(), NetError> {
    if ctx.user.is_none() || ctx.password.is_none() {
        return Err(NetError::MissingInput {
            protocol: PROTOCOLS::FTP, field: "user/password",
            hint: "call .set_user(...) and .set_password(...) before .connect()".into(),
        });
    }
    Ok(())
}
```

This also gives an opening to replace the current code's `.unwrap()`-on-locked-
connection-state pattern (`self.ftp_stream.unwrap().lock().unwrap()...`,
repeated throughout the FTP command methods) with real error returns during
the handler extraction — not a "while I'm here" cleanup, but directly what
this validation plan requires. Scope this per-handler as each one is
extracted, not as a separate pass.

## `ClientBuilder::from_url` and scheme inference (unchanged from previous draft)

```rust
impl ClientBuilder {
    pub fn from_url(url: &str) -> Result<Client, NetError> {
        let parsed = parse_url(url).map_err(NetError::Network)?;
        let protocol = scheme_to_protocol(&parsed.scheme)?; // NetError::UnrecognizedScheme on miss
        Ok(ClientBuilder::new().set_protocol(protocol).build().set_url(url))
    }
}
```

Used internally by every `XrdsNet` verb, and still available directly to
expert-layer developers who want a `Client` without naming a protocol.
`ClientBuilder::set_protocol()` stays as the explicit-override escape hatch
(e.g. forcing `HTTP3` on an `https://` URL once supported) — unchanged
reasoning from the previous draft.

### Scheme → protocol mapping

| Scheme(s) | Protocol | Notes |
| --- | --- | --- |
| `http://` | `HTTP` | |
| `https://` | `HTTPS` | HTTP/3 auto-upgrade (ALPN-style) is **future work** — see Out of scope. `set_protocol(HTTP3)` remains an explicit override. |
| `file://` | `FILE` | |
| `coap://` | `COAP` | |
| `ws://` | `WS` | |
| `wss://` | `WSS` | |
| `mqtt://` | `MQTT` | |
| `ftp://` | `FTP` | |
| `sftp://` | `SFTP` | |
| `quic://` | `QUIC` | Raw QUIC channel — an SDK-specific scheme convention, not a registered standard one. |
| anything else | — | `NetError::UnrecognizedScheme`, not a panic. |

`HTTP3` has no scheme of its own on purpose — reached only via the
`set_protocol` override until real ALPN-based negotiation exists.

## Drop `Client: Clone` (design cleanup, decided — unchanged from previous draft)

`Client` is `#[derive(Clone)]` today, but that derive exists **only to serve a
test-writing quirk**, not any real requirement — and it is actively harmful on
a type that owns a live connection (cloning silently shares the same
`Arc<Mutex<…>>` connection between two `Client` values).

The only external clone (`tests.rs:741`) is inside an MQTT receive loop and is
removable by unwrapping the `Result<Client>` once before the loop instead of
per-iteration (`rcv(&self)` only borrows — the clone was never needed). The
other clone (`run_ftp_command`'s internal `self.clone().run_ftp_*`) is an
artifact of by-value `Client` methods and disappears naturally in the handler
extraction (`FtpHandler` locks its own `Arc<Mutex<FtpStream>>`, never clones).

**Decision:** remove `#[derive(Clone)]` from `Client`. No `dyn-clone`
dependency needed. The only required change is rewriting that one MQTT test
loop to unwrap-before-loop.

### The by-value builder API stays (for now)

Builder methods still take `self` by value and return `Self`/
`Result<Self, NetError>`. Converting to `&mut self` would be cleaner but
touches every call site in `tests.rs`/`examples/net.rs` — separate, larger,
**out of scope** churn.

## Can WebRTC join this model? (investigated — partial yes, deferred)

Checked against the actual `WebRTCClient` API
(`crates/xrds-net/src/client/xrds_webrtc/webrtc_client.rs`). Short answer:
**the data channel could fit `StreamHandler`; media streaming should not.**

**Why media streaming doesn't fit:** WebRTC media is multiple simultaneous
tracks (video + audio), driven by the `VideoSource`/`AudioSource` injection
API purpose-built in the capture/encoding decoupling work
(`docs/xrds-net-capture-decoupling.md`). Forcing that through
`StreamHandler::send(Vec<u8>)` would throw away real capability for no
benefit. `WebRTCClient` stays the dedicated, richer, "I know I'm doing WebRTC
media" expert API — same relationship as `RuntimeHandler` to `XrdsAPI`.

**Why the data channel *could* fit `StreamHandler`, and the wrinkle that makes
it non-trivial anyway:** `send_data_channel_message`/receiving data-channel
messages really is just "send/receive bytes, optionally topic-addressed" —
plausible as a `WebRtcHandler: StreamHandler`. But today's connection
lifecycle is six sequential async steps — `connect_to_signaling_server` →
`create_session` **or** `join_session` → `publish` → `wait_for_subscriber` →
`exchange_ice_candidates` — and steps 2–4 are **asymmetric**: a publisher
creates a session, a subscriber joins one. Nothing else in this SDK has that
role split. `StreamHandler::connect()` would need to encode *which role to
take* somewhere (query param or scheme variant) — workable, but real design
work, not a mechanical wrap.

**Decision: out of scope for this pass.** `PROTOCOLS::WEBRTC` maps to
`UnsupportedHandler` (implements neither `as_stream` nor `as_file_transfer`,
nor `request`) — unchanged from today, `Client` doesn't support it now
either. Revisit data-channel-only integration once the publisher/subscriber
role question has an answer.

## Module layout

```text
crates/xrds-net/src/client/
  mod.rs              # module tree + re-exports (Client, ClientBuilder, XrdsNet, ...)
  net_intent.rs        # XrdsNet: request/dispatch/listen/transfer (the primary API)
  error.rs            # NetError
  event.rs            # Event, EventStream
  scheme.rs           # scheme_to_protocol(&str) -> Result<PROTOCOLS, NetError>
  client.rs           # Client + ClientBuilder (incl. ClientBuilder::from_url) — THIN, delegates to handler
  context.rs          # ClientContext (config/parsed-url state handlers read)
  handler.rs          # ProtocolHandler trait, create_handler factory, UnsupportedHandler
  categories.rs       # StreamHandler, FileTransferHandler traits
  protocols/
    mod.rs
    http.rs           # HttpHandler (HTTP/HTTPS/FILE) + ResponseCollector; curl
    coap.rs           # CoapHandler (COAP); coap/coap-lite
    quic.rs           # QuicHandler: StreamHandler + shared quic config; quiche/mio
    http3.rs          # Http3Handler: request only; quiche/mio (+ shared quic config)
    ws.rs             # WsHandler: StreamHandler; wraps existing xrds_websocket
    ftp.rs            # FtpHandler: FileTransferHandler + run_command extra; suppaftp
    mqtt.rs           # MqttHandler: StreamHandler + get_connection extra; rumqttc
  xrds_websocket.rs   # unchanged (WsHandler wraps it)
  xrds_webrtc/        # unchanged (WebRTCClient is a separate API, not a Client protocol)
```

`create_quic_config` is shared by `QuicHandler` and `Http3Handler` → factor
into a small shared helper in `protocols/quic.rs`. `parse_headers` (used by
HTTP + FILE) → move next to `HttpHandler`.

## Method → handler mapping (pure relocation, no behavior change)

| Handler | Shape | Current source methods | Expert-only extras (`as_any`) |
| --- | --- | --- | --- |
| `HttpHandler` | `request` | `request_http`, `request_file`, `parse_headers`, `ResponseCollector` | — |
| `CoapHandler` | `request` | `request_coap`, `run_coap` | — |
| `Http3Handler` | `request` | `request_http3` + `send_packet`/`receive_packets`/`send_http3_request`/`handle_http3_events` | — |
| `QuicHandler` | `StreamHandler` | `connect_quic`/`send_quic`/`rcv_quic` + `event_loop`/`handle_read`/`handle_write`/`send_initial_packet`/`start_event_loop` | — |
| `WsHandler` | `StreamHandler` | `connect_ws`/`send_ws`/`rcv_ws`/`close_ws` | — |
| `FtpHandler` | `FileTransferHandler` | `connect_ftp`/`connect_sftp` + all `run_ftp_*` | `run_command` (raw `FtpCommands`) |
| `MqttHandler` | `StreamHandler` | `connect_mqtt`/`send_mqtt`/`rcv_mqtt` | `get_connection` |
| `UnsupportedHandler` | none (all default errors) | — (covers `WEBRTC` + future feature-disabled protocols) | — |

Shared config helper `create_quic_config` lives with the QUIC code and is used
by both `QuicHandler` and `Http3Handler`.

## Verification strategy

- Compile checkpoint after each handler is extracted (move one protocol at a
  time).
- Rewrite the one MQTT test loop (`tests.rs` ~741) to unwrap-before-loop, the
  sole edit required by dropping `Client: Clone`.
- `cargo test -p xrds-net --lib` must show the **same** pass/flaky set as
  today (network-dependent tests are pre-existing flaky — compare against the
  baseline in `docs/xrds-net-capture-decoupling.md`, not against green).
- `examples/net.rs` (the only external `ClientBuilder` consumer) must still
  compile and run unchanged.
- Public-API diff on the `Client`/`ClientBuilder` surface is intentional and
  limited to: (a) `Client` no longer implements `Clone`, (b) error type is now
  `NetError` instead of `String` (a real, deliberate breaking change —
  `examples/net.rs` will need `.to_string()` or `From<NetError> for String`
  compat added if minimizing example churn matters).
- New: one `XrdsNet::{request,dispatch,listen,transfer}` test per applicable
  row of the capability matrix, plus one negative test per verb confirming an
  unsupported protocol returns `NetError::Capability`, not a panic or hang.
- New: `EventStream` clean-shutdown test (`.close()` joins the background
  thread without an arbitrary sleep, matching the `xrds-media`/
  `AudioTrackWriter` precedent).
- New: `validate()` tests per handler that implements it, confirming
  `NetError::MissingInput`'s hint text actually names what to fill in.
- New: one `examples/` entry demonstrating `XrdsNet` verbs as the primary,
  recommended usage — `examples/net.rs` predates this and only exercises the
  expert `ClientBuilder` path; keep it as the expert-path example, add a new
  one for `XrdsNet`.
- Note: new tests reuse the same real external endpoints the existing suite
  hits (`echo.websocket.org`, `test.mosquitto.org`, …) and inherit the same
  pre-existing network flakiness.

## Cargo features are a deployment concern, not a philosophy

Cargo features do not express, grant, or contribute to "protocol agnostic."
Agnosticism is about the SDK's call-site experience (`XrdsNet`'s four verbs,
no enum, no protocol name); features are about which protocols' code
physically ships in a binary. They're orthogonal, and defining `xrds-net`'s
`Cargo.toml` features means enumerating every protocol by name in the build
manifest — a hardcoded protocol list at a different layer, a mild pull
*against* the spirit of this design, not a step toward it.

**The only legitimate reason to add features is a concrete build-footprint
fact**: e.g. `curl`, `quiche`, or `webrtc` cannot cross-compile to
Android/Quest (unverified — see `docs/xrds-net-capture-decoupling.md`'s "Out
of scope"). If and when that's confirmed, per-protocol features become
mechanical once dispatch goes through `create_handler` (each `protocols/*.rs`
module gated, disabled protocols fall back to `UnsupportedHandler`, `default
= [all]` preserves today's behavior). Until then, **not planned work**.

## Risks / notes

- **Async boundary**: `request_coap` and `request_http3` build their own
  `tokio::runtime` / block internally today; keep that as-is inside the
  handlers. `EventStream` is sync (see above) — don't make the trait async in
  this pass.
- **`create_quic_config` duplication**: currently a `Client` method used by
  two paths — must become a shared free function.
- **`Send` bound**: the trait requires `Send`. Confirm all handler state stays
  `Send` (it is today, via `Arc<Mutex<…>>`).
- **`NetError` is a real public breaking change**, not purely additive —
  unlike the previous draft's "only `Clone` changes," every method that used
  to return `Result<_, String>` now returns `Result<_, NetError>`. Worth
  double-checking `examples/net.rs` and any other consumer compiles against
  the new error type before calling this "done."
- **Scope discipline**: extracting handlers is a *pure relocation* — no bug
  fixes beyond what `validate()`/`NetError` directly require, no incidental
  behavior changes. Resist "while I'm here" cleanups.
- **`ProtocolHandler: Send + Sync`, not just `Send`**: the `bevy_plugin`
  feature's `NetClientState` is a Bevy `Resource`, which requires `Sync`.
  Caught in Phase 4 by compiling with `--all-features` (the default `cargo
  test` run doesn't enable `bevy_plugin`, so this had gone unnoticed since
  `Client` started holding `Box<dyn ProtocolHandler>` in Phase 2) — every
  handler's state already satisfied `Sync` in practice, so this was a trait
  bound fix, not a handler rewrite.
- **`common::parse_url` now parses embedded userinfo** (`user:pass@host`) into
  `XrUrl::username`/`password` — added in Phase 4 because `XrdsNet::transfer`
  had no other way to supply FTP credentials (no protocol-specific builder
  step exists at that layer, by design). `ClientContext::parse_url_into_self`
  only fills `user`/`password` from this when they aren't already set via
  `set_user`/`set_password`, so the expert `Client` API's existing behavior is
  unchanged. `FtpHandler::connect` was also fixed to dial `ctx.host:ctx.port`
  instead of passing `ctx.raw_url` straight to `FtpStream::connect` (which
  can't handle a scheme, embedded userinfo, or a path).
- **Pre-existing (since fixed)**: `common::parse_url` used to default an
  unspecified port to `80` regardless of scheme — wrong for FTP (21),
  MQTT (1883), CoAP (5683), etc. **Resolved in a later pass**:
  `common::default_port_for_scheme` now supplies the well-known port per
  scheme (`https`/`wss` 443, `ftp` 21, `sftp` 22, `mqtt` 1883, `coap` 5683,
  `quic` 443, else 80), and an explicit `:port` still overrides. So
  `ftp://user:pass@host/file` and `mqtt://broker/topic` no longer need an
  explicit port.

## Out of scope

- **Per-protocol Cargo features** — not motivated by anything concrete yet.
- **HTTP/3 ALPN auto-negotiation** — `https://` always maps to `HTTPS` for now;
  `HTTP3` stays reachable only via the explicit `set_protocol` override.
- **MQTT 5 request/response correlation implementation** — the capability
  *declaration* (so unsupported request-over-MQTT fails clearly) is in scope;
  actually implementing the Response Topic/Correlation Data mechanism is not.
- **Connection pooling/reuse** across repeated `XrdsNet` calls to the same
  endpoint — v1 is fresh-connect-per-call; pooling is a valuable, separate
  future optimization.
- **Open runtime handler registration** — letting downstream code register
  new protocols/schemes without touching xrds-net's source. Not what was
  asked for: the goal is the *developer* not naming a protocol, which the
  fixed `create_handler` factory + `XrdsNet` already achieves. Reconsider only
  if real third-party protocol extension need shows up.
- **WebRTC data-channel-only integration** — plausible (see above), blocked on
  the publisher/subscriber role design question. Full WebRTC media stays out
  of this model permanently.
- **MoQ (Media over QUIC) handler** — cited only to validate that
  `StreamHandler` generalizes beyond MQTT/WS; not implemented now.
- Making the dispatch async / unifying the ad-hoc tokio runtimes.
- Any change to `WebRTCClient` (separate API, already decoupled from `Client`).
- Touching `xrds-media` or the transport-only surface finished in the
  capture/encoding decoupling work.

## Implementation checklist

Six phases, ordered so each one leaves the crate compiling and the existing
test suite green — nothing downstream is blocked on a half-finished phase, and
you can stop after any phase with a working (if not yet fully migrated) crate.

### Phase 0 — Scaffolding (new types only, zero wiring, zero behavior change)

Purely additive. `client.rs`/`Client` untouched and still works exactly as
today; nothing calls the new types yet.

- [x] `client/error.rs`: `NetError` enum + `Display`/`std::error::Error` impls
- [x] `client/context.rs`: `ClientContext` struct (fields listed in
      "`ProtocolHandler` mechanism" above), plus the `parse_url_into_self()`
      helper (dedupes the parse-and-fill-in logic currently duplicated in
      `connect()`/`request()`)
- [x] `client/event.rs`: `Event` struct
- [x] `client/handler.rs`: `ProtocolHandler` trait (with `validate`/`request`/
      `as_stream`/`as_file_transfer`/`as_any`/`as_any_mut`, all default bodies)
- [x] `client/categories.rs`: `StreamHandler`, `FileTransferHandler` traits
- [x] `client/scheme.rs`: `scheme_to_protocol(&str) -> Result<PROTOCOLS, NetError>`
      per the mapping table above
- [x] `cargo check -p xrds-net` — new modules compile standalone, nothing else
      changed

### Phase 1 — Handler extraction (one protocol at a time)

For each protocol: move its methods from `client.rs` into
`protocols/<name>.rs` as a `ProtocolHandler` impl, **but don't rewire
`Client` yet** — leave the old `match self.protocol { .. }` arms in
`client.rs` calling the *old* private methods, unchanged. This phase is pure
relocation + trait-shaping; it's low-risk exactly because `Client`'s actual
behavior doesn't move until Phase 2. Order chosen to front-load the simplest
(stateless, single-shape) protocols and save the trickiest for last:

- [x] `protocols/http.rs` — `HttpHandler` (`request` only): `request_http`,
      `request_file`, `parse_headers`, `ResponseCollector`
- [x] `protocols/coap.rs` — `CoapHandler` (`request` only): `request_coap`,
      `run_coap`
- [x] `protocols/http3.rs` — `Http3Handler` (`request` only): `request_http3`
      + its `send_packet`/`receive_packets`/`send_http3_request`/
      `handle_http3_events` helpers
- [x] `protocols/quic.rs` — `QuicHandler` (`StreamHandler`): `connect_quic`/
      `send_quic`/`rcv_quic` + `event_loop`/`handle_read`/`handle_write`/
      `send_initial_packet`/`start_event_loop`; factor `create_quic_config`
      into a shared free function used by both this and `Http3Handler`
- [x] `protocols/ws.rs` — `WsHandler` (`StreamHandler`): `connect_ws`/
      `send_ws`/`rcv_ws`/`close_ws` (wraps existing `xrds_websocket.rs`
      unchanged)
- [x] `protocols/mqtt.rs` — `MqttHandler` (`StreamHandler` + `get_connection`
      expert extra): `connect_mqtt`/`send_mqtt`/`rcv_mqtt`
- [x] `protocols/ftp.rs` — `FtpHandler` (`FileTransferHandler` + `run_command`
      expert extra): `connect_ftp`/`connect_sftp` + all `run_ftp_*`; add the
      `validate()` precondition check here (missing user/password) as part of
      this extraction, per "Guided-error validation"
- [x] `handler.rs`: `UnsupportedHandler` (covers `WEBRTC`) + `create_handler`
      factory wiring all of the above
- [x] `cargo check -p xrds-net` after **each** protocol above — don't batch;
      a broken handler extraction should be bisectable to one commit

### Phase 2 — `Client`/`ClientBuilder` cut over to the mechanism

This is the phase where behavior actually moves. Do this only once every
handler from Phase 1 exists and compiles standalone.

- [x] Rewrite `Client` to hold `ctx: ClientContext` + `handler: Box<dyn
      ProtocolHandler>` (delete the old per-protocol fields: `ws_client`,
      `ftp_stream`, `mqtt_client`, `mqtt_connection`, `quic_connection`,
      `udp_socket`, `event_poll`)
- [x] Rewrite `connect`/`send`/`rcv`/`close`/`request` to delegate via the
      capability queries (see "`Client` (expert/session API) after the
      refactor" above)
- [x] `run_ftp_command`/`get_mqtt_connection` delegate via `as_any()` downcast
- [x] Remove `#[derive(Clone)]` from `Client`
- [x] Rewrite the one MQTT test loop (`tests.rs` ~741) to unwrap-before-loop
- [x] `ClientBuilder::from_url(url) -> Result<Client, NetError>`
- [x] Update every `Result<_, String>` call site touched by this phase to
      `Result<_, NetError>` (this is the real breaking-change surface flagged
      in Risks — grep for `.request()`/`.connect()`/`.send()`/`.rcv()` callers)
- [x] `cargo test -p xrds-net --lib` — compare pass/flaky set against the
      pre-refactor baseline (see Verification strategy); must match, not just
      "be green"
- [x] `examples/net.rs` compiles and runs against the new `NetError` type

### Phase 3 — `XrdsNet` intent layer

- [x] `client/event.rs`: `EventStream` (background-thread + shutdown-flag +
      channel, per the `xrds-media`/`AudioTrackWriter` precedent)
- [x] `net_intent.rs`: `RequestOptions`, `TransferOp`, `TransferResult`,
      `topic_from_path()` helper
- [x] `net_intent.rs`: `XrdsNet::request`
- [x] `net_intent.rs`: `XrdsNet::dispatch`
- [x] `net_intent.rs`: `XrdsNet::listen` (wires up `EventStream`)
- [x] `net_intent.rs`: `XrdsNet::transfer`
- [x] `lib.rs`: re-export `XrdsNet`, `NetError`, `Event`, `EventStream`,
      `RequestOptions`, `TransferOp`, `TransferResult`

### Phase 4 — Verification

- [x] One `XrdsNet` test per capability-matrix row that's `✅` or `⚠️ opt-in`
      (HTTP3 `request` and QUIC `dispatch`/`listen` are exceptions, not
      exercised live through `XrdsNet` — see the note in
      `net_intent::tests` for why)
- [x] One negative test per verb confirming `NetError::Capability` (not a
      panic/hang) for a protocol that doesn't support it
- [x] `EventStream` clean-shutdown test (`.close()` joins without an arbitrary
      sleep)
- [x] `validate()` tests confirming `NetError::MissingInput` hint text
- [x] Full `cargo test -p xrds-net --lib` run, pass/flaky set compared against
      baseline one more time (post-`XrdsNet`, not just post-`Client` cutover)

### Phase 5 — Docs & examples

- [x] New `examples/` entry demonstrating `XrdsNet` verbs as the primary path
- [x] `examples/net.rs` kept as-is, re-labeled (in `examples/README.md`) as the
      expert `ClientBuilder`-path example
- [x] `examples/README.md` updated with the new example
- [x] This doc's `**Status:**` line at the top updated from "planned, not
      started" to reflect what's actually landed

## History of this doc

1. **First draft**: `ProtocolHandler` registry + `ClientBuilder::from_url`
   scheme inference, with `Client: Clone` dropped and category traits
   (`PubSubHandler`/`FileTransferHandler`) for MQTT/FTP extras. Public verbs
   were still `.connect()`/`.request()`/`.send()`/`.rcv()` — transport-shaped.
2. **This rewrite**: replaced the transport-shaped public verbs with
   intent-shaped ones (`request`/`dispatch`/`listen`/`transfer`) after
   comparing against `docs/PROTOCOL_AGNOSTIC_OPINION.md` and concluding its
   intent-based categorization is the more correct answer to "developer
   doesn't think about protocol" — while fixing its async-first assumption
   (unnecessary, given this codebase's existing sync `Receiver`-based
   streaming pattern) and its request-over-pub/sub soundness gap (made
   explicit/opt-in instead of a blanket promise). The previous draft's
   `ProtocolHandler` mechanism work carries forward as the shared internal
   foundation for both the new `XrdsNet` layer and the pre-existing `Client`
   expert layer — not discarded, repurposed.
