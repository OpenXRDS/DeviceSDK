# xrds-net — WebRTC Manual

WebRTC in `xrds-net` is a **separate subsystem** from the rest of the crate,
with its own module (`client::xrds_webrtc`), its own type (`WebRTCClient`), and
a working model that looks nothing like the intent verbs / sessions in the main
[MANUAL.md](./MANUAL.md):

- It is **async** (Tokio) — the main crate is synchronous/poll-based.
- It is **signaling-mediated pub/sub** — a publisher creates a *session*, one
  or more subscribers *join* it, and media flows peer-to-peer after an
  **offer/answer + ICE** handshake brokered by a signaling server. There is no
  `XrdsNet::open`/`NetChannel` here.
- It carries **negotiated real-time media** (H264 video / Opus audio over RTP),
  not opaque byte messages.
- It is **not part of the intent-verb model** and **not wired into the
  DeviceSDK** (`xrds::net` / `XrdsApp`). It is exported as
  `xrds_net::WebRTCClient` and used directly.

Because the handshake and lifecycle are this different, it gets its own manual.
For everything else (request/dispatch/listen/transfer/open), see
[MANUAL.md](./MANUAL.md).

---

## 1. The pieces

| Piece | Where | Role |
| --- | --- | --- |
| `WebRTCClient` | `xrds_net::WebRTCClient` | The peer — publisher or subscriber. |
| `VideoSource` / `AudioSource` | `xrds_net::{VideoSource, AudioSource}` | Injected, **already-encoded** media the publisher streams. |
| `VideoTrackHandler` / `AudioTrackHandler` / `MediaTrackHandler` | `client::media::*` | Subscriber callbacks that receive the remote track (raw RTP). |
| Signaling server | `XRNetServer::new(vec![PROTOCOLS::WEBRTC], vec![port])` | Brokers session discovery + offer/answer + ICE over WebSocket. |

`xrds-net` does **no codec work** and **no device access**. The publisher hands
it media already encoded in the negotiated codec (H264 Annex-B / Opus), and the
subscriber receives raw RTP to decode itself. Producing the encoded media
(capture + encode) is `xrds-media`'s job — see
[`docs/done/xrds-net-capture-decoupling.md`](../../docs/done/xrds-net-capture-decoupling.md).

---

## 2. The working process (handshake)

Media is peer-to-peer, but getting there needs a signaling server to relay SDP
and ICE. The flow (publisher **P**, subscriber **S**, signaling server **SS**):

```text
P: connect_to_signaling_server(ws://…)      S: connect_to_signaling_server(ws://…)
P: create_session()  ── session_id ──▶ SS
                                        SS ◀── list_sessions() / join_session(id) :S
P: publish(session_id)  ─ OFFER ─▶ SS ─ OFFER ─▶ S
                          S ─ ANSWER ─▶ SS ─ ANSWER ─▶ P
P: wait_for_subscriber(timeout)             (S has joined)
P: exchange_ice_candidates(false) ⇄ SS ⇄ exchange_ice_candidates(true) :S   (ICE)
   ── DTLS/SRTP established, media flows P ▶ S directly ──
P: start_stream(video, audio)               S: on_video_track / on_audio_track fire
P: stop_stream()                            S: (track ends)
```

The signaling messages (relayed by `SS` over WebSocket) are typed:
`create_session` · `list_sessions` · `join_session` · `leave_session` ·
`close_session` · `list_participants` · `offer` · `answer` · `welcome` ·
`ice_candidate` · `ice_candidate_ack`. You don't construct these yourself —
the `WebRTCClient` methods below do — but the message shape is `WebRTCMessage`
(`client_id`, `session_id`, `message_type`, `ice_candidates`, `sdp`, `error`)
if you inspect the wire.

---

## 3. External requirements (read this first)

WebRTC needs setup the rest of the crate doesn't:

1. **A Tokio runtime.** Every `WebRTCClient` method past `new()` is `async`.
   Use `#[tokio::main]` or an explicit runtime. (This is the main reason WebRTC
   isn't in the synchronous `XrdsNet` surface.)
2. **A rustls crypto provider installed once at startup**, before any client is
   used — WebRTC's DTLS handshake needs it:

   ```rust
   use rustls::crypto::{ring, CryptoProvider};
   CryptoProvider::install_default(ring::default_provider())
       .expect("install crypto provider");
   ```

   Omitting this makes the DTLS handshake fail at runtime.
3. **A reachable signaling server.** Either run the built-in one
   (`XRNetServer` with `PROTOCOLS::WEBRTC`, §6) or point at an existing
   compatible one.
4. **Desktop only.** Like the rest of `xrds-net`, the WebRTC path is excluded
   on Android (its native deps aren't available there).

---

## 4. Publisher

Create a session, publish an offer, wait for a subscriber, exchange ICE, then
stream already-encoded media. (Producing `VideoSource`/`AudioSource` — capture
and encode — is `xrds-media`'s job; shown fully in
[`examples/webrtc_webcam_stream.rs`](../../examples/webrtc_webcam_stream.rs).)

```rust
use xrds_net::{WebRTCClient, VideoSource, AudioSource};

let mut publisher = WebRTCClient::new();
publisher.connect_to_signaling_server("ws://127.0.0.1:18080/").await?;

let session_id = publisher.create_session().await?;
publisher.publish(&session_id).await?;           // sends the OFFER
publisher.wait_for_subscriber(10).await?;         // wait up to 10s for a joiner
publisher.exchange_ice_candidates(false).await?;  // publisher side (is_ack = false)

// Media already encoded elsewhere (H264 Annex-B reader + Opus frame channel).
let video = VideoSource::new(Box::new(h264_reader)); // impl Read + Send
let audio = AudioSource::new(opus_frame_rx);          // Receiver<Vec<u8>>, 20ms Opus frames
publisher.start_stream(video, Some(audio)).await?;    // video + optional audio

// … stream runs …
publisher.stop_stream().await?;
```

- **Media contract:** `VideoSource(Box<dyn Read + Send>)` = an H264 Annex-B
  byte stream (SPS/PPS + start-code-prefixed NAL units); `AudioSource { rx:
  Receiver<Vec<u8>> }` = Opus frames, one per message (~20ms each). `xrds-net`
  RTP-packetizes and writes them to the tracks; it never transcodes.
- `start_audio_stream(audio)` streams audio standalone (no video).
- ICE: publisher passes `false`, subscriber passes `true` — run them
  concurrently (`tokio::try_join!`).

---

## 5. Subscriber

Join a session, exchange ICE, then receive the remote track(s). A track arrives
as `Arc<TrackRemote>` carrying **raw RTP** — you read packets and decode them
yourself (`xrds-net` does not decode).

```rust
use std::sync::Arc;
use webrtc::track::track_remote::TrackRemote;
use xrds_net::WebRTCClient;

let mut subscriber = WebRTCClient::new();
subscriber.connect_to_signaling_server("ws://127.0.0.1:18080/").await?;

// Register a per-track callback BEFORE joining.
subscriber.on_video_track(|track: Arc<TrackRemote>| {
    Box::pin(async move {
        while let Ok((rtp, _attrs)) = track.read_rtp().await {
            // rtp.payload is an H264 RTP payload — depacketize / decode here.
        }
        Ok(())
    })
});

subscriber.join_session(&session_id).await?;       // discover via list_sessions() if needed
subscriber.exchange_ice_candidates(true).await?;   // subscriber side (is_ack = true)
// on_video_track fires once media flows.
```

Two callback styles (equivalent):

- **Closures:** `on_video_track(f)` / `on_audio_track(f)` / `on_media_tracks(f)`
  — `f: Fn(Arc<TrackRemote>) -> Pin<Box<dyn Future<Output=Result<(), …>> + Send>>`.
- **Traits:** `register_video_handler(Arc<dyn VideoTrackHandler>)` /
  `register_audio_handler` / `register_media_handler` — implement
  `handle_video_track(&self, track) -> HandlerFuture` (see `client::media`).
- `clear_handlers()` removes them.

**Debug capture (no decoder needed):** `set_debug_dir_path(dir).await` makes the
subscriber save the received video/audio to files; retrieve the paths with
`get_debug_video_file_path()` / `get_debug_audio_file_path()`. Useful to confirm
a stream actually arrived (the example decodes the saved files with `ffplay`).

---

## 6. Signaling server

Run the built-in signaling server via `XRNetServer` with the `WEBRTC` protocol
(it brokers session discovery + SDP/ICE relay over WebSocket):

```rust
use xrds_net::{server::XRNetServer, PROTOCOLS};

// XRNetServer::start() requires its root_dir to exist up front.
std::fs::create_dir_all("test_output").unwrap();
let server = XRNetServer::new(vec![PROTOCOLS::WEBRTC], vec![18080]);
tokio::spawn(async move { server.set_root_dir("test_output").start().await; });
```

Clients then `connect_to_signaling_server("ws://<host>:18080/")`.

---

## 7. `WebRTCClient` API reference

Constructor is sync; everything else is `async` (needs a Tokio runtime).

```rust
WebRTCClient::new() -> Self

// Signaling connection
.connect_to_signaling_server(addr: &str) -> Result<(), Box<dyn Error>>
.send_message(message: &str) -> Result<(), Box<dyn Error>>
.close_connection() -> Result<(), Box<dyn Error>>

// Sessions
.create_session() -> Result<String, String>            // publisher; returns session_id
.list_sessions() -> Result<WebRTCMessage, String>
.join_session(session_id: &str) -> Result<(), String>  // subscriber
.leave_session(session_id: &str) -> Result<(), String>
.close_session(session_id: &str) -> Result<(), String>
.list_participants(session_id: &str) -> Result<WebRTCMessage, String>

// Offer / subscriber wait / ICE
.publish(session_id: &str) -> Result<WebRTCMessage, String>   // publisher sends OFFER
.wait_for_subscriber(timeout_secs: u64) -> Result<(), String>
.exchange_ice_candidates(is_ack: bool) -> Result<(), String>  // pub=false, sub=true

// Streaming (publisher)
.start_stream(video: VideoSource, audio: Option<AudioSource>) -> Result<(), String>
.start_audio_stream(audio: AudioSource) -> Result<(), String>
.stop_stream() -> Result<(), String>

// Receiving (subscriber) — see §5
.on_video_track(F) / .on_audio_track(F) / .on_media_tracks(F)
.register_video_handler(Arc<dyn VideoTrackHandler>) / _audio_ / _media_
.clear_handlers()

// Data channel
.send_data_channel_message(message: &str) -> Result<(), String>

// Introspection / debug
.get_client_id() / .get_session_id() -> Option<&String>
.get_offer() / .get_answer() -> Option<&RTCSessionDescription>
.set_debug_dir_path(path: &str) -> Result<(), String>
.get_debug_video_file_path() / .get_debug_audio_file_path() -> Option<&String>
```

---

## 8. Constraints & non-goals

- **Not in the DeviceSDK yet.** `WebRTCClient` is standalone async; it is not
  reachable through `xrds::net` intent verbs and has no frame-loop bridge. Using
  it inside an `XrdsApp` means the app owns/drives a Tokio runtime and bridges
  results itself — not provided. Wiring WebRTC into the DeviceSDK is future
  work (noted in the integration doc).
- **No decode.** Subscribers get raw RTP (`TrackRemote`); depacketizing and
  decoding H264/Opus is the app's responsibility (or use the debug file save).
- **No encode / no capture.** Publishers inject already-encoded
  `VideoSource`/`AudioSource`; produce them with `xrds-media`.
- **Desktop only** (excluded on Android, like the rest of `xrds-net`).

---

## 9. Complete example & related docs

- [`examples/webrtc_webcam_stream.rs`](../../examples/webrtc_webcam_stream.rs)
  — end-to-end: in-process signaling server + publisher (real webcam/mic,
  captured & encoded via `xrds-media`) + subscriber (saves the received stream).
  The canonical, runnable reference for everything above.
- [`docs/done/xrds-net-capture-decoupling.md`](../../docs/done/xrds-net-capture-decoupling.md)
  — why capture + encoding live in `xrds-media`, and the exact `VideoSource`/
  `AudioSource` contract.
- [MANUAL.md](./MANUAL.md) — everything that is *not* WebRTC (the intent verbs,
  sessions, expert `Client`, capability matrix).
