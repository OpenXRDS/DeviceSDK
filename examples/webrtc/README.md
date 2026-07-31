# WebRTC examples — launching manual

Five files, three ways to run them:

| File | What it needs | Run it with |
| --- | --- | --- |
| [`webrtc_file_stream.rs`](webrtc_file_stream.rs) | Nothing — no camera/mic, no real network | one command |
| [`webrtc_webcam_stream.rs`](webrtc_webcam_stream.rs) | A real webcam + microphone | one command |
| [`webrtc_realnet_signaling_server.rs`](webrtc_realnet_signaling_server.rs), [`webrtc_realnet_publisher.rs`](webrtc_realnet_publisher.rs), [`webrtc_realnet_subscriber.rs`](webrtc_realnet_subscriber.rs) | Two (or three) separate terminals — or two separate machines for a real test | three commands, in order |
| [`realnet_common.rs`](realnet_common.rs) | — | not runnable; shared code for the three above |

See the top-level [`examples/README.md`](../README.md) for how this folder
fits into the rest of the examples tree.

---

## `webrtc_file_stream.rs` — no hardware needed

```bash
cargo run --example webrtc_file_stream
```

Runs a signaling server, a publisher, and a subscriber all in one process
on `127.0.0.1`, streaming the crate's bundled sample video. Start here if
you just want to see the WebRTC API work without any setup. Takes about
10 seconds.

## `webrtc_webcam_stream.rs` — needs a real camera + mic

```bash
cargo run --example webrtc_webcam_stream
```

Same shape as above, but the publisher captures your actual webcam (device
0) and default microphone instead of reading a file. Streams for 15
seconds, then saves what the subscriber received under `test_output/`.

---

## The `webrtc_realnet_*` binaries — testing across a real network

Unlike the two examples above, these three are **separate, independently
runnable programs** — built specifically so a real second machine can
connect to the first. They also use the **default** production STUN/TURN
config (no loopback shortcut), so they're the right tool when you actually
need to confirm WebRTC works over a real network, not just within one
process.

Background: `docs/xrds-net-release-readiness.md` Phase 3 and
`docs/done/xrds-net-webrtc-realnet-binaries.md`.

### Build them once

```bash
cargo build --example webrtc_realnet_signaling_server \
            --example webrtc_realnet_publisher \
            --example webrtc_realnet_subscriber
```

(Or skip this and just `cargo run` each one below — it'll build on first
run. Building explicitly first just avoids three separate "Compiling..."
waits interleaved with the instructions below.)

### 1. Start the signaling server

```bash
cargo run --example webrtc_realnet_signaling_server -- --port 9443
```

It prints the port it actually bound (useful if you passed `--port 0` for
an OS-assigned one) and a reminder that it can't tell you which of this
machine's IP addresses the other side can actually reach — check
`ip addr` / `ifconfig` / your router's client list yourself. Leave this
running (Ctrl-C to stop).

**Flags:** `--port <u32>` (default `0`), `--root-dir <path>` (default
`test_output`; required to exist, but unused by the `WEBRTC` protocol
itself).

### 2. Start the publisher

In a second terminal (same machine for a local check, or a different one
for a real test):

```bash
cargo run --example webrtc_realnet_publisher -- \
  --signaling-addr ws://<signaling-server-ip>:9443/
```

It creates a session and prints the session id inside a bordered banner —
**copy that id**, you'll need it for step 3. Then it waits up to 120s for
a subscriber to join.

**Flags:**

| Flag | Required | Default |
| --- | --- | --- |
| `--signaling-addr <ws://host:port/>` | yes | — |
| `--file <path>` | no | none — streams the crate's sample video, which is **embedded into the binary at compile time**, so it works even if you copy the built binary to a different machine. Pass a real path to stream something else instead. |
| `--stream-seconds <u64>` | no | `30` |
| `--turn-username <user>` | no | — (or set `XRDS_TURN_USERNAME`) |
| `--turn-password <pass>` | no | — (or set `XRDS_TURN_PASSWORD`) |

TURN credentials — via either the flags or the env vars, both do the same
thing — are only needed if you want the TURN relay entry included in ICE
gathering. Without them, this still runs, just STUN-only (logged as a
warning, not an error). Provide both or neither; one alone is treated the
same as neither.

### 3. Start the subscriber

In a third terminal (or the second machine):

```bash
cargo run --example webrtc_realnet_subscriber -- \
  --signaling-addr ws://<signaling-server-ip>:9443/ \
  --session-id <id printed by the publisher>
```

**Flags:**

| Flag | Required | Default |
| --- | --- | --- |
| `--signaling-addr <ws://host:port/>` | yes | — |
| `--session-id <id>` | yes | — |
| `--output-dir <path>` | no | `test_output` |
| `--turn-username <user>` | no | — (or set `XRDS_TURN_USERNAME`) |
| `--turn-password <pass>` | no | — (or set `XRDS_TURN_PASSWORD`) |

### 4. Read the result

Both the publisher and subscriber print, once ICE connects:

```text
[publisher] ICE connected: Some(Connected)
[publisher] Active candidate pair: local=Host remote=PeerReflexive
```

The `Active candidate pair` line is the actual answer to "did this go over
TURN, or just direct/STUN?" — candidate types are `host` (direct),
`srflx`/`prflx` (STUN-assisted), or `relay` (TURN). A `relay` result on
either side is the strongest evidence the TURN path specifically worked.

The subscriber also reports the received file's path and size at the end:

```text
Received stream saved to test_output/20260731_100642.h264 (14154318 bytes).
To visually verify: ffplay "test_output/20260731_100642.h264"
```

### Logging

All three default to a quiet-ish `RUST_LOG=info,webrtc_ice=error,webrtc_sctp=error,webrtc_mdns=error`
if you don't set `RUST_LOG` yourself — this hides the very chatty (and
almost never actionable) per-interface bind-probe and STUN-resolution
warnings those three crates emit, while keeping `xrds_net`'s own logs and
`webrtc`'s higher-level connection state changes visible. If ICE actually
fails to connect, `log_ice_summary`'s `[publisher]`/`[subscriber] ICE did
not connect: ...` line still tells you why. Set `RUST_LOG` yourself (e.g.
`RUST_LOG=debug`) before running to see everything, including from those
three crates.

### Running a local dry run first (recommended before a real 2-machine test)

All three can run as separate processes on **one** machine first, over
`127.0.0.1`, purely to sanity-check the binaries (argument parsing, session
id hand-off, ICE exchange, streaming, teardown) before taking them to an
actual second machine. This does **not** prove the real network/TURN path
works — loopback ICE gathering still tends to pick a local-interface `host`
pair even with the full STUN/TURN list configured — but it catches typos
and connection-plumbing mistakes cheaply. Just point all three at
`ws://127.0.0.1:<port>/` and run them in order as above, each in its own
terminal.

### Troubleshooting

- **Subscriber gets "Connection refused."** The signaling server isn't
  actually running (check terminal 1 didn't exit), or the port/address is
  wrong. If you background these with a shell `&` instead of running each
  in its own terminal, make sure the shell that started it doesn't exit —
  killing the shell kills the backgrounded process too.
- **`join_session` fails / "did you copy the session id correctly?"** The
  publisher generates a fresh UUID every run — re-copy it from the
  publisher's current banner, not a previous run's.
- **ICE never reaches `Connected`, stuck at `Checking`, or reaches
  `Failed`.** Usually a firewall/NAT blocking the path — try with
  `--turn-username`/`--turn-password` set so a relay candidate is
  available as a fallback, or set `RUST_LOG=debug` (see Logging above) to
  see the suppressed-by-default warnings about STUN/TURN hosts failing to
  resolve or respond. Both binaries print `ICE did not connect: ...` and
  exit cleanly in this case — they don't try to send data or stream over
  a connection that never came up, so seeing that message and a clean exit
  (not a panic/backtrace) is the *expected* behavior when ICE genuinely
  fails, not a bug.
- **Publisher's `wait_for_subscriber` times out (120s).** The subscriber
  never actually reached the signaling server — check its
  `--signaling-addr` matches what the server printed, including the
  scheme (`ws://`, not `wss://` — these binaries don't set up TLS on the
  signaling WebSocket).
