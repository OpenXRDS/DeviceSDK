/*
Copyright 2025 KETI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! `XrdsNet`: the four intent verbs (`request`/`dispatch`/`listen`/
//! `transfer`) that satisfy "protocol agnostic" as defined in
//! `docs/done/xrds-net-protocol-handler.md` — an XR app developer expresses what
//! they want, never which protocol or transport shape it needs. Built
//! entirely on the `ProtocolHandler` capability-query mechanism `Client`
//! itself uses; no protocol match anywhere in this file.

use crate::common::data_structure::NetResponse;

use super::client::ClientBuilder;
use super::error::NetError;
use super::event::{EventStream, ListenOptions};
use super::net_channel::NetChannel;
use super::net_feed::NetFeed;
use super::net_task::XrdsNetTask;

/// Options for `XrdsNet::request`. `RequestOptions::get()` covers the common
/// case; construct the struct directly for POST/custom headers/timeout.
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    pub method: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout: Option<u64>,
}

impl RequestOptions {
    pub fn get() -> Self {
        Self::default()
    }
}

/// A bulk file operation for `XrdsNet::transfer`. The URL path is the file
/// path (see "Topic addressing" in the plan doc) — `Upload`/`Download`
/// operate on that path; `List` uses it as the directory to list.
pub enum TransferOp {
    Upload(Vec<u8>),
    Download,
    List,
    Delete,
}

/// `XrdsNet::transfer`'s result — one variant per `TransferOp`.
#[derive(Debug, Clone)]
pub enum TransferResult {
    Uploaded,
    Downloaded(Vec<u8>),
    Listed(Vec<String>),
    Deleted,
}

/// The URL path is the topic for topic-addressed transports (MQTT, future
/// MoQ) — `mqtt://broker/sensors/temp` -> `sensors/temp`. Root/empty paths
/// (topic-less transports, or a bare `mqtt://broker/`) report `None`; query
/// strings (folded into `ClientContext::path` by `parse_url_into_self`)
/// aren't part of the topic.
pub(crate) fn topic_from_path(path: &Option<String>) -> Option<String> {
    let raw = path.as_deref()?;
    let without_query = raw.split('?').next().unwrap_or("");
    let trimmed = without_query.trim_start_matches('/');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Namespacing only — matches the `XrdsAPI`-style typed surface used
/// elsewhere in the SDK. Every method is a one-shot: connect, operate,
/// (implicitly) drop. No connection pooling/reuse across calls — see "Out of
/// scope" in the plan doc.
pub struct XrdsNet;

impl XrdsNet {
    /// One-shot, wants a reply. HTTP/HTTPS/FILE/CoAP/HTTP3 always; MQTT only
    /// if the broker/protocol version declares request/response support
    /// (opt-in — see the capability matrix in the plan doc).
    ///
    /// **Blocking.** Inside `XrdsApp` (`setup`/`update`) use
    /// [`request_async`](Self::request_async) instead — this call runs on the
    /// calling thread and would freeze the frame. The sync form is for
    /// scripts, tests, and non-Bevy tooling where blocking is fine.
    pub fn request(url: &str, opts: RequestOptions) -> Result<NetResponse, NetError> {
        let mut client = ClientBuilder::from_url(url)?;

        if let Some(method) = &opts.method {
            client = client.set_method(method);
        }
        if !opts.headers.is_empty() {
            let headers: Vec<(&str, &str)> =
                opts.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            client = client.set_req_headers(headers);
        }
        if let Some(body) = opts.body {
            let body_str = String::from_utf8(body)
                .map_err(|e| NetError::Network(format!("request body is not valid UTF-8: {e}")))?;
            client = client.set_req_body(&body_str);
        }
        if let Some(timeout) = opts.timeout {
            client = client.set_timeout(timeout);
        }

        client.request()
    }

    /// Frame-safe [`request`](Self::request): runs the blocking call on a
    /// background thread and hands back an [`XrdsNetTask`] to poll each frame
    /// with `try_take()` (or `Option::take_ready()`). This is the form to use
    /// inside `XrdsApp`.
    pub fn request_async(url: &str, opts: RequestOptions) -> XrdsNetTask<NetResponse> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::request(&url, opts))
    }

    /// One-shot, fire-and-forget — no reply awaited. MQTT publish, QUIC/WS
    /// send. The URL path is the topic.
    ///
    /// **Blocking.** Inside `XrdsApp` use [`dispatch_async`](Self::dispatch_async)
    /// — the sync form runs on the calling thread and would freeze the frame.
    pub fn dispatch(url: &str, payload: Vec<u8>) -> Result<(), NetError> {
        let client = ClientBuilder::from_url(url)?;
        let (ctx, mut handler) = client.into_parts();
        handler.validate(&ctx)?;
        let topic = topic_from_path(&ctx.path);

        match handler.as_stream() {
            Some(stream) => {
                stream.connect(&ctx)?;
                stream.send(&ctx, topic.as_deref(), payload)
            }
            None => Err(NetError::capability(
                ctx.protocol,
                "dispatch",
                "protocol has no send/dispatch capability",
            )),
        }
    }

    /// Frame-safe [`dispatch`](Self::dispatch): runs on a background thread,
    /// returns an [`XrdsNetTask`] to poll. Note the task must be polled (or
    /// deliberately dropped) — dropping it is non-blocking, but you won't
    /// learn of a send failure unless you `try_take()` it.
    pub fn dispatch_async(url: &str, payload: Vec<u8>) -> XrdsNetTask<()> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::dispatch(&url, payload))
    }

    /// Ongoing feed. WS, MQTT subscribe, (future) MoQ subscribe. Returns a
    /// live handle — see `EventStream`. Uses `ListenOptions::default()`
    /// (bounded + lossless); use `listen_with` to tune the buffer / overflow
    /// policy (e.g. shallow + drop-oldest for live video).
    ///
    /// **Blocking during connect/subscribe.** Inside `XrdsApp`, use the
    /// frame-safe streaming path (`listen_async`/`NetFeed`, Phase A3) rather
    /// than calling this on the frame thread.
    pub fn listen(url: &str) -> Result<EventStream, NetError> {
        Self::listen_with(url, ListenOptions::default())
    }

    /// As `listen`, but with an explicit buffer size and overflow policy.
    pub fn listen_with(url: &str, opts: ListenOptions) -> Result<EventStream, NetError> {
        let client = ClientBuilder::from_url(url)?;
        let (ctx, mut handler) = client.into_parts();
        handler.validate(&ctx)?;

        if handler.as_stream().is_none() {
            return Err(NetError::capability(
                ctx.protocol,
                "listen",
                "protocol has no ongoing-stream capability",
            ));
        }

        EventStream::spawn(handler, ctx, opts)
    }

    /// Frame-safe streaming: runs the blocking connect/subscribe on a
    /// background thread and hands back an [`XrdsNetTask`] that resolves to a
    /// live [`EventStream`]. Most app code should prefer
    /// [`listen_feed`](Self::listen_feed), which wraps this handshake in a
    /// single value; reach for this when you want to observe the
    /// connect→stream transition yourself. Uses `ListenOptions::default()`.
    pub fn listen_async(url: &str) -> XrdsNetTask<EventStream> {
        Self::listen_with_async(url, ListenOptions::default())
    }

    /// As `listen_async`, with an explicit buffer / overflow policy.
    pub fn listen_with_async(url: &str, opts: ListenOptions) -> XrdsNetTask<EventStream> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::listen_with(&url, opts))
    }

    /// The recommended in-app streaming surface: connects on a background
    /// thread and returns a [`NetFeed`] you hold and drain each frame with
    /// `try_recv()`. A connect/subscribe failure surfaces via
    /// `NetFeed::take_error()` rather than a `Result` here. Uses
    /// `ListenOptions::default()`.
    pub fn listen_feed(url: &str) -> NetFeed {
        Self::listen_feed_with(url, ListenOptions::default())
    }

    /// As `listen_feed`, with an explicit buffer / overflow policy (e.g.
    /// shallow buffer + `Overflow::DropOldest` for live video).
    pub fn listen_feed_with(url: &str, opts: ListenOptions) -> NetFeed {
        NetFeed::new(Self::listen_with_async(url, opts))
    }

    /// Bulk file operation. FTP/SFTP today.
    ///
    /// **Blocking** (a download blocks for the whole transfer). Inside
    /// `XrdsApp` use [`transfer_async`](Self::transfer_async).
    pub fn transfer(url: &str, op: TransferOp) -> Result<TransferResult, NetError> {
        let client = ClientBuilder::from_url(url)?;
        let (ctx, mut handler) = client.into_parts();
        handler.validate(&ctx)?;
        let path = ctx.path.clone().unwrap_or_default();
        let path = path.trim_start_matches('/');

        match handler.as_file_transfer() {
            Some(ft) => {
                ft.connect(&ctx)?;
                match op {
                    TransferOp::Upload(data) => {
                        ft.upload(&ctx, path, data)?;
                        Ok(TransferResult::Uploaded)
                    }
                    TransferOp::Download => Ok(TransferResult::Downloaded(ft.download(&ctx, path)?)),
                    TransferOp::List => Ok(TransferResult::Listed(ft.list(&ctx, path)?)),
                    TransferOp::Delete => {
                        ft.delete(&ctx, path)?;
                        Ok(TransferResult::Deleted)
                    }
                }
            }
            None => Err(NetError::capability(
                ctx.protocol,
                "transfer",
                "protocol has no file-transfer capability",
            )),
        }
    }

    /// Frame-safe [`transfer`](Self::transfer): runs the blocking transfer on
    /// a background thread, returns an [`XrdsNetTask`] to poll each frame.
    pub fn transfer_async(url: &str, op: TransferOp) -> XrdsNetTask<TransferResult> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::transfer(&url, op))
    }

    /// Open a **bidirectional session** — one connection you both `send` on
    /// and `try_recv` from (a persistent control/data channel). Returns a
    /// [`NetChannel`]. This is the *session* shape, distinct from the pub/sub
    /// `dispatch`/`listen` verbs; use it for a point-to-point socket.
    ///
    /// Supported by session-capable protocols only — **`quic://` today**
    /// (WS/`wss` once its client is reworked for duplex; until then a WS
    /// request/response uses the expert `Client`). Anything else returns
    /// `NetError::Capability`.
    ///
    /// **Blocking during connect.** Inside `XrdsApp` use
    /// [`open_async`](Self::open_async).
    pub fn open(url: &str) -> Result<NetChannel, NetError> {
        let client = ClientBuilder::from_url(url)?;
        let (ctx, mut handler) = client.into_parts();
        handler.validate(&ctx)?;

        {
            let session = handler.as_session().ok_or_else(|| {
                NetError::capability(
                    ctx.protocol,
                    "open",
                    "protocol has no bidirectional-session capability — use \
                     request/dispatch/listen/transfer, or the expert Client",
                )
            })?;
            session.connect(&ctx)?;
        }

        Ok(NetChannel::new(ctx, handler))
    }

    /// Frame-safe [`open`](Self::open): runs the blocking connect on a
    /// background thread and hands back an [`XrdsNetTask`] that resolves to a
    /// live `NetChannel`.
    pub fn open_async(url: &str) -> XrdsNetTask<NetChannel> {
        let url = url.to_string();
        XrdsNetTask::spawn(move || XrdsNet::open(&url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_options_get_has_no_method_headers_or_body() {
        let opts = RequestOptions::get();
        assert!(opts.method.is_none());
        assert!(opts.headers.is_empty());
        assert!(opts.body.is_none());
        assert!(opts.timeout.is_none());
    }

    #[test]
    fn topic_from_path_strips_leading_slash_and_query() {
        assert_eq!(
            topic_from_path(&Some("/sensors/temp?unit=c".to_string())),
            Some("sensors/temp".to_string())
        );
    }

    #[test]
    fn topic_from_path_is_none_for_root_or_missing_path() {
        assert_eq!(topic_from_path(&Some("/".to_string())), None);
        assert_eq!(topic_from_path(&Some(String::new())), None);
        assert_eq!(topic_from_path(&None), None);
    }

    #[test]
    fn dispatch_on_a_request_only_protocol_is_a_capability_error() {
        let err = XrdsNet::dispatch("https://example.com/x", vec![1, 2, 3])
            .expect_err("HTTP has no dispatch capability");
        assert!(matches!(err, NetError::Capability { verb: "dispatch", .. }));
    }

    #[test]
    fn request_async_resolves_to_the_same_error_as_the_sync_verb() {
        // A bad scheme fails in `from_url` before any network I/O, so this
        // exercises the full async path (spawn → sync verb → result delivered
        // through the task) deterministically, no network needed.
        let mut task = XrdsNet::request_async("gopher://example.com/x", RequestOptions::get());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let result = loop {
            if let Some(r) = task.try_take() {
                break r;
            }
            assert!(std::time::Instant::now() < deadline, "async task never completed");
            std::thread::yield_now();
        };
        assert!(matches!(result, Err(NetError::UnrecognizedScheme(s)) if s == "gopher"));
    }

    #[test]
    fn listen_on_a_request_only_protocol_is_a_capability_error() {
        let err = XrdsNet::listen("https://example.com/x").expect_err("HTTP has no listen capability");
        assert!(matches!(err, NetError::Capability { verb: "listen", .. }));
    }

    #[test]
    fn transfer_on_a_request_only_protocol_is_a_capability_error() {
        let err = XrdsNet::transfer("https://example.com/x", TransferOp::Download)
            .expect_err("HTTP has no transfer capability");
        assert!(matches!(err, NetError::Capability { verb: "transfer", .. }));
    }

    #[test]
    fn request_on_a_stream_only_protocol_is_a_capability_error() {
        let err = XrdsNet::request("ws://example.com/x", RequestOptions::get())
            .expect_err("WS has no request capability");
        assert!(matches!(err, NetError::Capability { verb: "request", .. }));
    }

    #[test]
    fn open_on_a_non_session_protocol_is_a_capability_error() {
        // No network: the session-capability check happens before connect.
        // (QUIC is the only session-capable protocol today.)
        let err = XrdsNet::open("https://example.com/x")
            .expect_err("HTTP has no bidirectional-session capability");
        assert!(matches!(err, NetError::Capability { verb: "open", .. }));
    }

    #[test]
    fn unrecognized_scheme_is_a_clear_error_not_a_panic() {
        let err = XrdsNet::request("gopher://example.com/x", RequestOptions::get())
            .expect_err("gopher is not a mapped scheme");
        assert!(matches!(err, NetError::UnrecognizedScheme(s) if s == "gopher"));
    }

    // --- Capability-matrix positive tests (live network) ---------------------
    //
    // One test per matrix row that's ✅ or ⚠️ opt-in, exercised through the
    // public `XrdsNet` verbs (not `Client` directly) — these are the rows the
    // developer-facing intent API actually needs to satisfy. Real network
    // I/O, same accepted flakiness as the rest of this suite (see
    // `docs/done/xrds-net-protocol-handler.md`'s Verification strategy).
    //
    // Not covered here (documented gap, not an oversight):
    // - HTTP3 `request`: HTTP3 has no scheme of its own (see `scheme.rs`) —
    //   it's only reachable via `ClientBuilder`'s expert `set_protocol`
    //   override, never through `XrdsNet`'s URL-scheme inference. Already
    //   covered at the `Client`/handler level in `client::tests` and
    //   `protocols::http3`.
    // - QUIC `dispatch`/`listen`: reachable in principle via the `quic://`
    //   SDK-convention scheme, but not exercised live here. Already covered
    //   structurally (unit-level) in `protocols::quic`; `Client`-level live
    //   QUIC connectivity is covered by `client::tests::test_client_quic_connect`.

    #[test]
    fn request_http_reaches_a_real_server() {
        let response = XrdsNet::request("http://www.rust-lang.org:80/", RequestOptions::get())
            .expect("HTTP request should succeed");
        assert!(response.error.is_none());
        assert!(!response.body.is_empty());
    }

    #[test]
    fn request_file_returns_bytes() {
        let response = XrdsNet::request(
            "https://files.keti-xr.duckdns.org/api/public/dl/afeLp4YK/Box.glb",
            RequestOptions::get(),
        )
        .expect("FILE request should succeed");
        assert!(!response.body.is_empty());
    }

    #[test]
    fn request_coap_reaches_a_real_server() {
        let response = XrdsNet::request("coap://coap.me:5683/test", RequestOptions::get())
            .expect("CoAP request should succeed");
        assert_eq!(response.status_code, 69);
    }

    #[test]
    fn request_mqtt_is_opt_in_and_reports_capability_not_a_hang() {
        // No MQTT 5 request/response correlation is implemented (out of
        // scope, see the plan doc) — this proves the *declaration* works:
        // a clear, immediate Capability error, never a hang.
        let err = XrdsNet::request("mqtt://test.mosquitto.org:1883/hello/keti", RequestOptions::get())
            .expect_err("MQTT request/response is opt-in and undeclared here");
        assert!(matches!(err, NetError::Capability { verb: "request", .. }));
    }

    #[test]
    fn dispatch_ws_reaches_a_real_server() {
        XrdsNet::dispatch("wss://echo.websocket.org/", b"hello from XrdsNet".to_vec())
            .expect("WS dispatch should succeed");
    }

    #[test]
    fn dispatch_mqtt_publishes_to_a_real_broker() {
        XrdsNet::dispatch("mqtt://test.mosquitto.org:1883/hello/keti", b"ping".to_vec())
            .expect("MQTT dispatch (publish) should succeed");
    }

    #[test]
    fn listen_mqtt_receives_a_dispatched_message() {
        // A topic distinct from `dispatch_mqtt_publishes_to_a_real_broker`'s
        // `hello/keti` — that test's publish is `retain: true`, so
        // subscribing to the same topic here could immediately receive its
        // stale retained message instead of the one this test dispatches.
        let topic_url = "mqtt://test.mosquitto.org:1883/hello/keti/xrdsnet_listen_test";

        // listen() connects AND subscribes (confirmed via SUBACK) before
        // returning, so it's safe to dispatch() immediately after.
        let stream = XrdsNet::listen(topic_url).expect("MQTT listen (subscribe) should succeed");

        XrdsNet::dispatch(topic_url, b"listen-test".to_vec())
            .expect("MQTT dispatch (publish) should succeed");

        let event = stream
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("should receive the dispatched message");
        assert_eq!(event.payload, b"listen-test".to_vec());
        stream.close();
    }

    #[test]
    fn transfer_ftp_downloads_a_real_file() {
        // FTP credentials travel as URL userinfo (`user:pass@host`) since
        // `XrdsNet`'s verbs take a bare URL — no protocol-specific builder
        // step the developer would need to know is FTP-only. No explicit port
        // needed: the `ftp` scheme defaults to 21 (see
        // `common::default_port_for_scheme`).
        let result = XrdsNet::transfer(
            "ftp://demo:password@test.rebex.net/readme.txt",
            TransferOp::Download,
        );
        match result.expect("FTP download should succeed") {
            TransferResult::Downloaded(bytes) => assert!(!bytes.is_empty()),
            other => panic!("expected TransferResult::Downloaded, got {other:?}"),
        }
    }

    #[test]
    fn transfer_ftp_without_credentials_is_a_missing_input_error() {
        let err = XrdsNet::transfer("ftp://test.rebex.net:21/readme.txt", TransferOp::Download)
            .expect_err("no credentials in the URL should fail guided validation");
        assert!(matches!(err, NetError::MissingInput { .. }));
    }
}
