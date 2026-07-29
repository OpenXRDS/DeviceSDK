# xrds-net

Protocol-agnostic **client** networking for XR applications — drops into a
synchronous app/frame loop without an async runtime.

You express *intent* (get a reply, fire-and-forget, subscribe to a feed, move
a file) with a URL, and the crate picks the protocol from the URL scheme. No
protocol enum to choose, no `.await`, no engine plugin to register.

> For the full reference (every type, the expert session API, the capability
> matrix, design rationale), see [MANUAL.md](./MANUAL.md).

## What it's for (and not for)

- **For:** a client app — an XR app, a tool, an editor — making a handful of
  network calls, from a synchronous loop, without wanting to learn `tokio`.
- **Not for:** high-concurrency servers (it uses a background thread per
  operation), game-state/entity replication, or media decode/playback (that's
  `xrds-media` + the runtime). This crate moves bytes; that's it.

## The four verbs

```rust
use xrds_net::{XrdsNet, RequestOptions, TransferOp};

// request  — one-shot, want a reply
let resp = XrdsNet::request("https://api.example.com/manifest", RequestOptions::get())?;

// dispatch — one-shot, fire-and-forget (URL path is the topic)
XrdsNet::dispatch("mqtt://broker.local/sensors/temp", reading_bytes)?;

// listen   — ongoing feed (returns a drainable stream)
for event in XrdsNet::listen("mqtt://broker.local/commands")? {
    handle(event.payload);
}

// transfer — bulk file op (URL path is the remote path; ftp defaults to :21)
XrdsNet::transfer("ftp://user:pass@host/scans/room.glb", TransferOp::Upload(bytes))?;
```

The protocol is inferred from the scheme — moving `listen("mqtt://…")` to
`listen("wss://…")` (or a future `moq://…`) is a URL change, not a code change.

### Plus: `open` — a bidirectional session

For "one connection I both send on and read replies from" (a persistent
control/data channel — WS/QUIC's native shape), `XrdsNet::open` returns a
`NetChannel` you `send` on and `try_recv` from. Supported for **`quic://`**,
**`ws://`**, and **`wss://`**:

```rust
let mut chan = XrdsNet::open("wss://echo.example.com/")?;
chan.send(b"hello".to_vec())?;
while let Some(ev) = chan.try_recv() { /* ev.payload */ }
```

## Inside an XR app (the recommended path)

Those four verbs **block**, so in a frame loop use the `_async` forms and poll
them — the blocking work runs on a background thread, `update()` never stalls.

```rust
use xrds::net::{XrdsNet, XrdsNetTask, NetTaskSlot, NetFeed, RequestOptions, NetResponse};

struct MyApp {
    manifest: Option<XrdsNetTask<NetResponse>>, // a one-shot in flight
    feed: Option<NetFeed>,                       // an ongoing stream
}

impl XrdsApp for MyApp {
    fn setup(&mut self, _api: &mut XrdsAPI) {
        self.manifest = Some(XrdsNet::request_async("https://…/manifest", RequestOptions::get()));
        self.feed = Some(XrdsNet::listen_feed("mqtt://broker.local/telemetry"));
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext) {
        // one-shot: hands back the result once, clears the slot for you
        if let Some(result) = self.manifest.take_ready() {
            match result { Ok(r) => { /* r.body */ }, Err(e) => log::error!("{e}") }
        }
        // stream: drain whatever arrived this frame (never blocks)
        if let Some(feed) = &mut self.feed {
            while let Some(ev) = feed.try_recv() { /* ev.payload */ }
            if let Some(e) = feed.take_error() { log::error!("{e}"); self.feed = None; }
        }
    }
}
```

Within the DeviceSDK the crate is reached as `xrds::net::…` (no extra Cargo
dependency; works on desktop **and** Android/Quest — only the FTP *server* and
WebRTC are desktop-only). Standalone, use
`xrds_net::…`. See [`examples/net_app.rs`](../../examples/net_app.rs) (in-app),
[`examples/net_intent.rs`](../../examples/net_intent.rs) (standalone, sync),
and [`examples/net.rs`](../../examples/net.rs) (expert API).

## Protocols

| Scheme(s) | Protocol | request | dispatch | listen | transfer | open |
| --- | --- | :-: | :-: | :-: | :-: | :-: |
| `http` / `https` | HTTP/1.1 | ✅ | — | — | — | — |
| `file` | FILE (byte GET) | ✅ | — | — | — | — |
| `coap` | CoAP | ✅ | — | — | — | — |
| `ws` / `wss` | WebSocket | — | ✅ | ✅ | — | ✅ |
| `quic` | raw QUIC | — | ✅ | ✅ | — | ✅ |
| `mqtt` | MQTT | ⚠️ opt-in | ✅ | ✅ | — | — |
| `ftp` / `sftp` | FTP | — | — | — | ✅ | — |
| (via expert API) | HTTP/3 | ✅ | — | — | — | — |

`open` is the bidirectional-session shape (one connection you both send on and
read replies from). An unsupported verb returns a clear `NetError::Capability`
(never a hang or a silent no-op). Full matrix + footnotes in
[MANUAL.md](./MANUAL.md) §12.

**WebRTC** is a **separate** subsystem (`WebRTCClient`) — async, signaling-based
publish/subscribe with an offer/answer + ICE handshake, not part of the intent
verbs. It has its own guide: **[MANUAL_WEBRTC.md](./MANUAL_WEBRTC.md)**.

## Errors

Every call returns `Result<_, NetError>`, a structured enum:
`UnrecognizedScheme` · `Capability { protocol, verb, detail }` ·
`MissingInput { protocol, field, hint }` · `Network` · `Protocol`. The
`MissingInput` hint tells you what to fill in (e.g. FTP credentials).

## Test

```bash
cargo test -p xrds-net
```

Some tests hit live public servers (rust-lang.org, test.mosquitto.org,
test.rebex.net, coap.me) and are flaky under network conditions; that flaky set
is the known baseline. See [MANUAL.md](./MANUAL.md) for the full detail.
