# xrds-net: decouple device capture and encoding, add media source injection

**Status: done, verified live end-to-end.** `xrds-net` no longer touches
hardware or encodes media. Device capture and codec encoding both live in
`xrds-media`. See "Final architecture" below for the shape that shipped,
"History" for how it got there (the design changed twice mid-implementation as
real constraints surfaced), and "Fixed: h264_mf / COM conflict" for the one
real bug hit along the way — root-caused and fixed, with the full pipeline
(real webcam + mic → real encode → real WebRTC → saved file) confirmed by
decoding the output with ffmpeg.

## Why

`xrds-net` is a **networking** crate — its job is to move bytes over protocols
(HTTP, CoAP, MQTT, FTP, WS, QUIC, WebRTC transport). It used to reach directly
into local hardware to acquire media (`nokhwa` webcam, `cpal` mic) **and**
encode that media for the wire (`ffmpeg-next` JPEG→H264, `opus` PCM→Opus).

Both are the wrong job for a networking crate, for the same underlying reason:

- **Device access is platform I/O, not networking**, and the wrong model for
  XR specifically — on Quest the thing you stream isn't a USB webcam enumerated
  by nokhwa, it's the passthrough camera or a rendered eye buffer.
- **Codec encoding is a media concern, not a networking one.** WebRTC transport
  requires a negotiated codec's bitstream, but *producing* that bitstream from
  raw pixels/samples pulls in an entirely separate failure surface — encoder
  selection, hardware acceleration, platform codec-library interop — that has
  nothing to do with sockets, ICE, or RTP. This was confirmed empirically: the
  one real bug hit during this work (see "Known issue" below) was 100% inside
  ffmpeg's encoder/COM interaction, zero involvement from anything transport-related.

## Final architecture

| Concern | Owner | Deps |
| --- | --- | --- |
| Acquire raw frames from camera/mic hardware | `xrds-media` (PC-only) | `nokhwa`, `cpal` |
| Encode for transport (JPEG→H264, PCM→Opus) | `xrds-media`'s `transcoding` feature | `ffmpeg-next`, `opus` |
| RTP packetize, transport, track write | `xrds-net` (its actual job) | `webrtc` |
| Decide *what* to stream, wire capture → transcode → transport | app / example / editor | — |

**`xrds-media`'s mandate is broader than capture alone.** It's the crate for
desktop media I/O generally: capture (always available), encoding-for-transport
(the `transcoding` feature), and — planned, not yet implemented — media import
and playback for both `xrds-runtime` and the GUI editor. It was named
`xrds-media`, not `xrds-media-capture`, for exactly this reason: avoid
reshuffling dependents again as each capability lands. See
`crates/xrds-media/src/lib.rs` for the current module map.

**Dependency direction is unchanged and still load-bearing**: `xrds-media` does
not depend on `xrds-net`, and `xrds-net` does not depend on `xrds-media` or any
codec library. `xrds-media` exposes plain outputs (a `Receiver<Vec<u8>>` of JPEG
frames, a `(Receiver<Vec<i16>>, AudioFormat)` for PCM) and, behind
`transcoding`, ergonomic encode functions that turn those into exactly what
`xrds-net`'s `VideoSource`/`AudioSource` expect (an Annex-B H264 `Read`, a
`Receiver<Vec<u8>>` of Opus frames). The **caller** (example, app, editor) owns
both dependencies and does the wiring — capture → transcode → transport is
nobody's job but the integrator's.

### The injected-source contract (xrds-net)

- `VideoSource(Box<dyn Read + Send>)` — an H264 Annex-B byte stream, already
  encoded. Parsed via `H264Reader` and written to the track as-is. xrds-net
  never transcodes.
- `AudioSource { rx: Receiver<Vec<u8>> }` — Opus frames, one per message
  (conventionally 20ms each). Written to the track via a small bridge
  (`AudioTrackWriter`) that exists only to cross from the caller's plain
  `std::sync::mpsc` channel onto the async `write_sample` call — that bridge is
  transport plumbing, not encoding, so it stays in xrds-net.

### xrds-media's transcoding feature

`crates/xrds-media/src/transcoding/` (gated behind the `transcoding` Cargo
feature, adding `ffmpeg-next` + `opus` as optional deps):

- `jpeg2h264.rs`, `pcm2opus.rs`, `img2vid_encoder.rs`, `streaming_mp4_writer.rs`
  — moved here verbatim from xrds-net's old `media/transcoding/`.
- `stream.rs` — the ergonomic entry points a caller actually uses:
  `encode_jpeg_stream_to_h264(frame_rx, w, h, fps) -> (H264StreamEncoder, FrameReader)`
  and `encode_pcm_stream_to_opus(pcm_rx, format) -> (OpusStreamEncoder, Receiver<Vec<u8>>)`.
  Also owns the corrected `resample_and_convert` (frame-based, not flat-sample,
  indexing — see "History" for the bug this fixes).

## Fixed: h264_mf / COM conflict

`Jpeg2H264Transcoder::new` calls `encoder::find(codec::Id::H264)` — "give me
*a* H264 encoder" — which resolves to the hardware `h264_mf` (Windows Media
Foundation) encoder on this machine's ffmpeg build (this build has no libx264
compiled in — confirmed via `encoder::find_by_name("libx264")` returning
`None` — so preferring libx264 by name, tried first, was not a viable fix here).

The actual root cause was **not** which encoder got selected — it was **which
thread created it**. `encode_jpeg_stream_to_h264` used to call
`Jpeg2H264Transcoder::new(...)` synchronously, on whatever thread called the
function (a shared/pooled tokio worker thread in the example). `h264_mf`
requires Windows COM in MTA mode; a pooled thread can have COM already set to
STA by unrelated prior work elsewhere in the process, which makes `h264_mf`'s
own internal `CoInitializeEx(MTA)` fail with exactly the observed
`COM must not be in STA mode` / `Invalid argument`. **Fix**: move encoder
construction inside the dedicated `std::thread::spawn` worker (a brand new OS
thread with no prior COM state), reporting success/failure back to the caller
over a small handshake channel so the public API stays synchronous. See
`crates/xrds-media/src/transcoding/stream.rs`'s `encode_jpeg_stream_to_h264`.

**Confirmed fixed** by running `examples/webrtc_webcam_stream.rs` live and
validating the subscriber's saved output with `ffmpeg -i <file> -f null -`:
the received `.h264` decoded as 200 real frames at 1920x1080 (~6.66s) and the
`.opus` decoded as 48kHz stereo PCM (~14.6s) — genuine end-to-end proof, not
just "no panic." To watch it yourself: `ffplay test_output/<timestamp>.h264`
(the example prints the exact path and command at the end of its run).

## History

The design went through two real pivots while building this, both driven by
things discovered mid-implementation rather than planned upfront:

1. **Webcam/mic capture doesn't belong in xrds-net** (original scope). Moved to
   a new sibling crate. Discovered along the way: `VideoSource::MediaStream`
   (despite the name) always assumed pre-encoded H264, not raw JPEG — the
   webcam path had its own separate JPEG→H264→track pipeline. This meant the
   video source needed two genuinely different shapes, not one.
2. **Encoding doesn't belong in xrds-net either** (the bigger pivot). Triggered
   by hitting the h264_mf/COM bug above and asking "do we need transcoder code
   here at all, given capture is already separated?" The answer: no — encoding
   is exactly the same class of non-networking concern as capture, and the bug
   itself proved it (zero networking code involved in the failure). This
   removed `VideoSource::Jpeg` and `media/video_pipeline.rs` (JpegTranscoder)
   entirely, simplified `AudioSource` from raw-PCM-plus-format down to
   already-Opus-encoded frames, and moved all four transcoding files
   (`jpeg2h264.rs`, `pcm2opus.rs`, `img2vid_encoder.rs`,
   `streaming_mp4_writer.rs`) into the capture crate — which is also why that
   crate was renamed `xrds-media` rather than staying `xrds-media-capture`.

Two debug-only utilities (`capture_audio_encode_to_file`, an earlier
`realtime_webcam_to_mp4`/`record_jpeg_source_to_mp4`) existed purely to verify
encoding — once encoding left xrds-net, they had nothing left to verify there
and were deleted rather than migrated.

`xrds-audio` (an empty placeholder crate on this branch) was considered as a
merge target for `xrds-media` but explicitly **not** touched — it has real,
non-trivial content on another branch not yet visible here, and merging blind
would risk conflicting with that work. Revisit once that branch's content is
reconciled.

---

## Work #1 — `xrds-media` capture crate + standalone tests ✅

- [x] Crate created, later renamed `xrds-media-capture` → `xrds-media` (see
      "Why xrds-media" above). Runtime-agnostic — std threads +
      `std::sync::mpsc`, no tokio dependency.
- [x] `video::Webcam::open(device_id)` → `(Webcam, Receiver<Vec<u8>>)`, one
      complete JPEG frame per message. (Changed from an initial `FrameReader`
      return — see below.) Plus `video::list_available_devices()`.
- [x] `audio::Microphone::open_default()` → `(Microphone, Receiver<Vec<i16>>,
      AudioFormat)`.
- [x] Pure logic isolated from device I/O for hardware-free testing:
      `video::find_complete_jpeg` (JPEG boundary scan), `video::FrameReader`
      (buffering `Read` adapter — kept as a reusable building block even
      though `Webcam::open` no longer returns it directly), `audio::convert`
      (`f32`/`u16` → `i16`).
- [x] `test-util` feature: `mock::{mock_webcam, mock_mic, canned_jpeg}` —
      synthetic sources satisfying the same output contract as real devices.
- [x] Full Tier 1 (pure logic) + Tier 2 (mock) + Tier 3 (real hardware,
      `#[ignore]`) test suite — all green, including a confirmed manual
      hardware run on Windows (webcam + mic). See `crates/xrds-media/tests/`.
- [x] `Webcam::open`'s return type changed from `FrameReader` to the raw
      `Receiver<Vec<u8>>` once Work #2 clarified that xrds-net's video source
      wants discrete frames, not a byte stream (a camera naturally produces
      frames, not a continuous stream). `mock_webcam` updated to match.

## Work #2 — media source injection + transcoding relocation (xrds-net) ✅

- [x] `media/source.rs`: `VideoSource(Box<dyn Read + Send>)` (H264 Annex-B,
      always-encoded) + `AudioSource { rx: Receiver<Vec<u8>> }` (Opus frames).
      No `Jpeg` variant, no `AudioFormat` — both were removed once encoding
      left xrds-net (see History).
- [x] `media/audio_pipeline.rs`: `AudioTrackWriter` — bridges the caller's
      sync Opus-frame channel onto the async `write_sample` call. This is the
      only piece of the old `AudioCapturer`/`AudioEncoder` lineage that
      survived in xrds-net; everything encoding-related (resample, Opus
      encode) moved to `xrds-media::transcoding`.
- [x] `media/video_pipeline.rs` (`JpegTranscoder`/`VideoTrackWriter`) deleted
      entirely — the existing `stream_from_buf_read`/`H264Reader` path already
      covers writing pre-encoded H264 to the track.
- [x] `WebRTCClient::start_stream(video: VideoSource, audio: Option<AudioSource>)`
      / `start_audio_stream(audio: AudioSource)` — video-with-optional-audio,
      and standalone audio.
- [x] Removed `capture_audio_encode_to_file`, `stop_audio_capture`, and the
      renamed `record_jpeg_source_to_mp4` (formerly `realtime_webcam_to_mp4`)
      — debug-only encode-verification utilities with nothing left to verify
      in xrds-net once encoding moved out.
- [x] `nokhwa`, `cpal`, `ffmpeg-next`, `opus`, `image` all removed from
      `crates/xrds-net/Cargo.toml` — confirmed absent via `cargo tree -i`.
      (`ogg`, `rand`, `chrono` **stay** — used by the unrelated, still-active
      receive-side `save_audio_to_disk` default handler, which muxes already-
      encoded RTP payloads into Ogg without touching a codec library.)
- [x] `tests.rs` updated: the dedicated webcam-stream test and the two
      "custom handler" tests (which only ever used a webcam source to kick off
      *some* stream, not to test webcam-ness specifically) now all use the
      existing sample H264 file via `VideoSource::new`.
- [x] `cargo check -p xrds-net --tests`: zero warnings. `cargo test -p
      xrds-net --lib`: consistently ~62-68/73 pass; the remainder are
      pre-existing network-flakiness failures (real STUN/TURN/HTTP3 servers
      under parallel load) — confirmed by re-running individually (all pass
      alone) and by the *set* of failing tests changing between runs, which
      network-independent bugs wouldn't do.
- [x] `examples/webrtc_webcam_stream.rs` rewired onto
      `xrds_media::transcoding::{encode_jpeg_stream_to_h264,
      encode_pcm_stream_to_opus}` → `xrds_net::{VideoSource, AudioSource}`.
      Compiles clean; run live end-to-end up to the h264_mf/COM bug above
      (signaling, ICE, real device open all confirmed working).

## Out of scope (tracked elsewhere / future work)

- The larger `ProtocolHandler` registry refactor for true protocol-agnostic
  dispatch (the original motivating discussion for this whole effort).
- Per-protocol Cargo features on `xrds-net` (`http`, `coap`, `mqtt`, `ftp`,
  `quic`, `webrtc`, …) — planned to come *after* this decoupling, unblocked now
  that device/codec deps are gone from xrds-net's default build.
- Whether `quiche`/`curl`/`webrtc` (the *remaining* xrds-net deps) can
  cross-compile to Android — untouched by this work.
- Media import/playback for the SDK and editor — `xrds-media`'s module
  structure leaves room for it, but nothing is implemented.
- Reconciling `xrds-audio`'s real content (on another branch) with `xrds-media`
  — deliberately not attempted here.

## Summary for report

- **Problem**: `xrds-net` (the networking crate) directly opened camera/mic
  hardware and ran its own JPEG→H264/PCM→Opus codec encoding for WebRTC — two
  concerns unrelated to "move bytes over a network," and the reason Quest/XR
  device capture couldn't be swapped in.
- **Change**: created a new crate, **`xrds-media`**, that owns both device
  capture (`nokhwa` webcam, `cpal` mic) and codec encoding (`ffmpeg-next`,
  `opus`, behind an opt-in `transcoding` feature). `xrds-net` no longer depends
  on either kind of library.
- **Result**: `xrds-net` now only accepts **already-encoded** media
  (`VideoSource` = H264 bytes, `AudioSource` = Opus frames) and handles pure
  transport — RTP packetization and track writes. It is a solid, hardware- and
  codec-agnostic networking crate; any future capture source (Quest passthrough
  camera, a video file, a synthetic test source) can feed it without touching
  xrds-net at all.
- **Crate scope was deliberately broadened**, not narrowed: `xrds-media` is
  named for general desktop media I/O (capture + encoding today; media
  import/playback for the SDK and editor is planned), so future capabilities
  land there without another reshuffle.
- **A real bug was found and fixed along the way**: a Windows COM
  apartment-threading conflict between the hardware H264 encoder and the
  webcam driver, when both ran on a shared thread. Root-caused (not just
  worked around) and fixed by moving encoder creation onto its own dedicated
  thread.
- **Verified, not just compiled**: a live example
  (`examples/webrtc_webcam_stream.rs`) runs the real webcam and microphone
  through real encoding, over a real local WebRTC connection, and the received
  stream was independently decoded with `ffmpeg` — 200 real video frames at
  1920x1080 and correct 48kHz stereo audio — confirming the full pipeline
  actually works end to end, not merely that it builds.
- **Test coverage**: `xrds-media` has a three-tier test suite (pure-logic unit
  tests, hardware-free mock-source tests, and real-hardware tests) all
  passing, including a confirmed manual run against real camera/mic hardware.
  `xrds-net`'s existing test suite continues to pass (excluding pre-existing,
  unrelated network flakiness in a handful of tests that depend on real
  external STUN/TURN servers).
- **Explicitly deferred, not forgotten**: reconciling this work with
  `xrds-audio` (which has unrelated in-progress content on another branch),
  building actual media import/playback, and the larger per-protocol Cargo
  feature split for `xrds-net` that motivated this effort originally.
