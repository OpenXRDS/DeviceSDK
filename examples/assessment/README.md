### Purpose of this directory

- This directory contains sample codes to test one of the project performance metrics which is:

  - 1. Protocol Agnostic 지원 XR 콘텐츠 타입
       1. 3차년도 목표: 5 종
       2. 4차년도 목표: 8종

  * 평가 시나리오
    * Protocol Agnostic 지원 가능한 XR 콘텐츠 종류별 API 동작 및 전송 프로토콜 확인

---

## The test

A **content × protocol matrix**: every supported XR content type, over every
protocol. Five content types × five protocols = 25 transfers.

Running it is five commands per side, not 25: **one run of a sender/receiver
pair carries the whole content suite over one protocol**, so each pair is
started once and fills in a whole column of the matrix.

Each sender declares, per item, a manifest — content type, file name, byte
count, SHA-256. Each receiver verifies the bytes it actually got against that
manifest, prints a per-item line and a suite report, and exits non-zero if any
item failed. The verification is identical for every protocol, so a PASS on
MQTT means exactly what a PASS on FTP means: **the bytes that arrived are the
bytes that were sent**.

Nothing in the transfer path branches on content type, and the only thing that
changes between protocols is the URL scheme. That is the claim being assessed.

## Test procedure

### The one rule: whichever side LISTENS starts first

Get this wrong and the second command fails with "connection refused". It is
not the same side every time — the two pull protocols invert the roles, because
for them the node holding the content is the one that serves it.

| # | protocol | start first | then start | why |
|---|----------|-------------|------------|-----|
| 1 | `ws` | **receiver** | sender | the receiver is the WebSocket server |
| 2 | `mqtt` | **receiver** | sender | it must be subscribed before anything is published, or the messages are gone |
| 3 | `ftp` | **receiver** | sender | the receiver is the FTP server |
| 4 | `http` | **sender** | receiver | the sender serves the content; the receiver pulls it |
| 5 | `coap` | **sender** | receiver | same as HTTP, over UDP |

A memory hook: **ws / mqtt / ftp → receiver first. http / coap → sender first.**

You do not have to look up any IP address. **Whichever side you start first
prints the exact command to run on the other machine**, with its own address
already filled in:

```text
  on the sending machine, run:
      assess_ws_sender.exe --url ws://192.168.0.10:9301/
```

Copy that line to the other machine and run it. If a port was busy, the binary
moved to the next free one and the line it printed already says so.

### Step 0 — prepare both machines

On the **sending** machine: the repo checkout (run with `cargo run --example`)
or `dist/assessment-sender-kit/`.
On the **receiving** machine: `dist/assessment-receiver-kit/`.

Run every command **from inside the kit folder** (or the crate root, for
`cargo run`) — content files and output folders are relative paths.

Open the ports from the table in "Running the two nodes on two machines" below.
The usual first-run problem is Windows Firewall prompting on the listening side;
allow it for the private network.

### Steps 1–5 — one protocol at a time

Each pair transfers the whole content suite, so five runs cover the 25-cell
matrix. Below, `S>` is a command on the sending machine and `R>` on the
receiving machine, in the order given. Substitute the addresses the tools print.

```text
1. WebSocket        R> assess_ws_receiver.exe --port 9301
                    S> assess_ws_sender.exe --url ws://<RECEIVER-IP>:9301/

2. MQTT             R> assess_mqtt_receiver.exe --url mqtt://172.16.1.55:1883/xrds/assessment/run1
                    S> assess_mqtt_sender.exe   --url mqtt://172.16.1.55:1883/xrds/assessment/run1
                       (use a NEW topic name for every run)

3. FTP              R> assess_ftp_receiver.exe --port 2121
                    S> assess_ftp_sender.exe --url ftp://xrds:assessment@<RECEIVER-IP>:2121/

4. HTTP             S> assess_http_sender.exe --port 9302
                    R> assess_http_receiver.exe --url http://<SENDER-IP>:9302

5. CoAP             S> assess_coap_sender.exe --port 5683
                    R> assess_coap_receiver.exe --url coap://<SENDER-IP>:5683
```

What you should see each time, on the **receiving** machine:

```text
  RESULT        : 5/5 PASS
  evidence      : examples/assessment/received/ws/a40a2019
```

Both processes exit on their own when the suite is done — nothing to Ctrl-C.
Exit code is 0 only when every item passed, so a script can check it.

### Step 6 — prove the protocol (optional, recommended)

With the endpoints still up, on either machine:

```bat
powershell -ExecutionPolicy Bypass -File prove-protocols.ps1 -SenderIp <SENDER-IP> -ReceiverIp <RECEIVER-IP>
```

It points an independent client at each endpoint and writes
`protocol-proof.txt`. For a packet capture instead, see `WIRESHARK.md` — start
Wireshark *before* the sender, since a run takes well under a second.

### Step 7 — collect the evidence

From the **receiving** machine, take
`examples/assessment/received/<protocol>/<run-id>/` for each of the five runs:
the received files, `report.txt` and `result.json`. Add `protocol-proof.txt`
and any `.pcapng` captures.

### If something goes wrong

- **"connection refused"** — the listening side isn't up yet (check the table
  above), the firewall blocked it, or the port moved: use the command the
  listening side printed.
- **Receiver sits at "waiting ..."** — the sender hasn't run yet, or it is
  pointed at a different port/topic. For MQTT both sides must name the *same*
  topic.
- **CoAP retries or times out** — CoAP is UDP; a TCP-only firewall rule does
  not pass it.
- **A port was busy** — the binary says so and uses the next one; take the
  address from the line it printed.

## The evidence on disk

The receiving machine writes the transferred files, and they are the proof the
transfer happened. Each run gets its own folder so runs never overwrite each
other:

```text
received/<protocol>/<run-id>/
    StainedGlassLamp.gltf        the received files, byte-identical to the originals
    arrow_box.png
    spatial_test_ping.wav
    tiny_test.mp4
    assessment_scene.xrds
    report.txt                   the printed suite report, verbatim
    result.json                  the same thing machine-readable, per item
```

`report.txt` names both machines, which is what makes it evidence of a transfer
*between two nodes* rather than a local file copy:

```text
=== XRDS protocol-agnostic transfer: SUITE REPORT ===
  protocol      : ws
  xrds-net API  : XRNetServer (WS) + registered binary handler
  run id        : a40a2019
  sent from     : host SENDER-PC (192.168.0.20) -> ws://192.168.0.10:9301/
  received on   : host RECEIVER-PC (192.168.0.10) <- ws://0.0.0.0:9301/
  ...
  gltf-model      StainedGlassLamp.gltf      30985    1   0.004 s  PASS
                  sha256 ddebbf01… (byte-identical (size + sha256 match))
  RESULT        : 5/5 PASS
```

The sender's host name, its own IP and the target URL travel in the manifest,
so the receiving node can record where each file came from; it adds its own host
and IP and the endpoint it received on. Each side finds its address by the route
to the other (a `connect`ed UDP socket, which sends nothing), because a
listening receiver never learns the peer's address through the xrds-net API.
`result.json` carries the same, plus the expected and received SHA-256 for every
item and the sender's timestamp.

Collect `received/` from the **receiving** machine for the evidence pack. It is
git-ignored: run output, not source.

## The content suite

The supported XR content types, and the file used for each by default:

| content type | meaning | default file | size |
|--------------|---------|--------------|------|
| `gltf-model` | glTF / GLB 3D model | `assets/models/StainedGlassLamp/StainedGlassLamp.gltf` | 30,985 B |
| `texture` | image texture, lightmap, environment map | `assets/textures/arrow_box.png` | 2,217 B |
| `audio` | audio clip (spatial or ambient) | `assets/sound/wav/spatial_test_ping.wav` | 88,244 B |
| `video` | video clip (2D or 360) | `assets/videos/tiny_test.mp4` | 2,943 B |
| `scene-document` | XRDS scene document | `fixtures/assessment_scene.xrds` | 4,002 B |

The vocabulary is fixed: a sender naming anything else is rejected rather than
quietly accepted, so the matrix in the report is the matrix that was agreed.

Note on the glTF item: `StainedGlassLamp.gltf` is the glTF *document* — its
geometry (`.bin`) and textures are sibling files, and only the document itself
is this item. That is fine for a transfer test (the bytes are verified either
way), but if the received file should be loadable on its own, run the
gltf-model item against a self-contained GLB instead:
`--file assets/models/animated/phoenix_bird.glb --content-type gltf-model`.

The default files are small on purpose — every protocol has to get through the
whole suite in a sitting, and MQTT opens one broker connection per chunk (cheap
on a LAN broker, ~0.8 s each against a public one). To assess throughput
instead, swap in bigger assets:

```bash
# one content type, one file
--file assets/models/animated/phoenix_bird.glb --content-type gltf-model

# a custom suite (types inferred from the extension unless --types is given)
--files assets/models/animated/phoenix_bird.glb,assets/videos/2d_test_sample.mp4
```

`fixtures/assessment_scene.xrds` is a genuine scene document produced by
`XrdsSceneDocument::save_json` — a root, a cube with a material, a point light
and a camera — not a hand-written stand-in.

## The protocols

| protocol | sender uses | receiver uses | shape |
|----------|-------------|---------------|-------|
| `ws://` | `XrdsNet::open` → `NetChannel::send` / `try_recv` | `XRNetServer` (WS) + registered binary handler | bidirectional session, chunked + acked |
| `mqtt://` | `XrdsNet::dispatch` (URL path = topic) | `XrdsNet::listen` → `EventStream` | pub/sub via a broker (third node) |
| `ftp://` | `XrdsNet::transfer(Upload / Download / List)` | `XRNetServer` (FTP, libunftp) | bulk file transfer |
| `http://` | peer node: minimal `std::net` HTTP server | `XrdsNet::request` (GET) | request/response pull |
| `coap://` | peer node: `coap` crate UDP server | `XrdsNet::request` (GET) | request/response pull over UDP |

All four verbs of the protocol-agnostic API — `request`, `dispatch` + `listen`,
`transfer`, `open` — are exercised end-to-end against a live peer.

The HTTP and CoAP receivers make the *same calls* against the *same resource
layout* (`/count`, `/item/<i>/manifest`, then the payload) and differ only in
the URL scheme. That is the protocol-agnostic claim stated as plainly as it can
be.

For HTTP and CoAP the *sending* node is the one that serves: an XR app pulls a
GLB from a content server, so that is the direction worth assessing. xrds-net
has no HTTP or CoAP server, so `http_sender.rs` (on `std::net`) and
`coap_sender.rs` (on the `coap` crate) exist only to give the xrds-net client a
peer. All assessed code is on the receivers.

## Running (single machine, and the per-protocol options)

"Test procedure" above is the follow-along for a two-machine run; this section
is the reference for running it all on one machine and for the flags each pair
accepts.

With no `--file`/`--files` both sides use the default suite, so the commands
below are the whole test. Start the side that listens first — for `ws`, `mqtt`
and `ftp` that is the receiver, for `http` and `coap` the sender. `--help`
prints usage; exit code is 0 when every item passed.

```bash
# --- WebSocket ---------------------------------------------------------------
cargo run --example assess_ws_receiver -- --port 9301
cargo run --example assess_ws_sender   -- --url ws://127.0.0.1:9301/

# --- MQTT (use a fresh topic per run: MQTT retains the last message) ---------
# 172.16.1.55 is the KETI lab broker (1883 and 11883 both work); any broker
# does. Avoid public brokers — see the note below the commands.
cargo run --example assess_mqtt_receiver -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1
cargo run --example assess_mqtt_sender   -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1

# --- FTP ---------------------------------------------------------------------
cargo run --example assess_ftp_receiver -- --port 2121
cargo run --example assess_ftp_sender   -- --url ftp://xrds:assessment@127.0.0.1:2121/

# --- HTTP (the SENDER serves; start it first) --------------------------------
cargo run --example assess_http_sender   -- --port 9302
cargo run --example assess_http_receiver -- --url http://127.0.0.1:9302

# --- CoAP (the SENDER serves; start it first) --------------------------------
cargo run --example assess_coap_sender   -- --port 5683
cargo run --example assess_coap_receiver -- --url coap://127.0.0.1:5683
```

Per-protocol options worth knowing:

- **WS** — `--chunk-size` (default 65536), `--ack-timeout` seconds.
- **MQTT** — `--chunk-size` (default 4096; rumqttc's default packet limit is
  10 KiB each way). Receiver `--timeout` is an *inactivity* timeout (default
  300 s), so a slow suite is not mistaken for a hang. **Use a broker you
  control.** The suite takes 0.13 s through the lab broker and 35 s through
  `test.mosquitto.org`, because `dispatch` reconnects per chunk and a public
  broker puts an internet round trip on each one — the public figure measures
  the internet, not the SDK.
- **FTP** — the receiver listens on all interfaces (`--bind 127.0.0.1` for a
  local-only run); credentials are URL userinfo, and libunftp's default
  authenticator accepts any user/password. The sender also reads each file back
  with `TransferOp::Download` and compares hashes, proving the round trip
  independently of the receiver.
- **HTTP / CoAP** — the sender exits once the whole suite has been served;
  `--keep` keeps it up for more runs. CoAP `--chunk-size` defaults to 16384;
  block-wise transfer (RFC 7959) carries whatever it is set to, so it is a
  memory/retry trade-off rather than a datagram limit — verified to 64 KiB on
  loopback and over a LAN. Drop it to 512 on a lossy link. Receiver `--retries`
  (default 3) covers lost datagrams and the count is reported.

## The portable kit (running on a machine without the toolchain)

These binaries live in the root `xrds` package, whose dev-dependencies include
`xrds-media` (→ **ffmpeg-next**, needing FFmpeg dev libraries installed),
`xrds-openxr` and Bevy. So `cargo run --example assess_*` compiles a renderer
and a media stack to move a file, and fails outright on a machine without
FFmpeg — even though the assessment itself only uses `xrds-net`. (It does *not*
pull quiche/BoringSSL: the root package has no quiche edge.)

For the evaluation machines, copy the prebuilt kits instead — two disjoint
folders, so no binary is duplicated:

| folder | holds | goes to |
|--------|-------|---------|
| `dist/assessment-sender-kit/` (~53 MB) | the five senders + the content files + the `.xrds` fixture | the sending machine |
| `dist/assessment-receiver-kit/` (~77 MB) | the five receivers | the receiving machine |

Both also carry `RUN.md`, `WIRESHARK.md` and `prove-protocols.ps1`. Each runs
from its own folder, needs nothing installed, and the receiver kit needs no
content files (only the sender reads them).

Rebuild **both** after changing any of this code:

```bash
cargo build --example assess_ws_sender --example assess_ws_receiver \
            --example assess_mqtt_sender --example assess_mqtt_receiver \
            --example assess_ftp_sender --example assess_ftp_receiver \
            --example assess_http_sender --example assess_http_receiver \
            --example assess_coap_sender --example assess_coap_receiver
# then copy the senders over dist/assessment-sender-kit/
#      and the receivers over dist/assessment-receiver-kit/
```

`dist/` is git-ignored — the kit is a build artifact, not source. Its binaries
are debug builds: correctness is what they verify, so the throughput figures
they print are conservative.

If the toolchain barrier becomes a recurring nuisance, the durable fix is to
move these binaries into their own workspace member depending only on
`xrds-net`, which would make a plain clone-and-run work anywhere.

## Running the two nodes on two machines

Everything above works unchanged across a network — replace `127.0.0.1` in the
URL with the other machine's address. Two things to get right first.

**Which machine has to be reachable depends on the protocol**, because the two
pull protocols invert the roles: HTTP and CoAP are request-shaped, so the node
that *serves* is the one holding the content.

That also decides **which side types the other's address**. A receiver that
listens never needs the sender's IP:

| receiver | is given the sender's address? | how |
|----------|-------------------------------|-----|
| `assess_http_receiver` | yes | `--url http://SENDER-IP:9302` |
| `assess_coap_receiver` | yes | `--url coap://SENDER-IP:5683` |
| `assess_mqtt_receiver` | no — the **broker's** | `--url mqtt://BROKER-IP:1883/<topic>` |
| `assess_ws_receiver` | no, it listens | `--port 9301` (the *sender* types the receiver's IP) |
| `assess_ftp_receiver` | no, it listens | `--port 2121` |

Either way both addresses end up in the evidence: each side records its own IP
on the route to the other, and the sender's travels in the manifest.

Nobody has to look an address up, either: **whichever side listens prints the
command for the other machine**, with its own IP filled in —

```text
  on the sending machine, run:
      assess_ws_sender.exe --url ws://172.16.1.51:9301/
```

so the ws/ftp receivers and the http/coap senders each hand over a line to paste
on the far side.

| protocol | listens (must be reachable, opens the firewall) | dials out |
|----------|------------------------------------------------|-----------|
| `ws://` | **receiver** — TCP `--port` (9301) | sender |
| `ftp://` | **receiver** — TCP `--port` (2121) **and** passive data ports 49152–65535 | sender |
| `http://` | **sender** — TCP `--port` (9302) | receiver |
| `coap://` | **sender** — UDP `--port` (5683) | receiver |
| `mqtt://` | neither — **both dial out** to the broker on TCP 1883 | both |

**Every receiver listens on all interfaces**, FTP included (`--bind 127.0.0.1`
for a local-only run). Note that `XRNetServer` itself still defaults to loopback
for FTP — its server runs with libunftp's anonymous authenticator, so exposing
it is the caller's decision; this harness makes that decision because a
cross-machine transfer is its whole purpose.

Say the receiver machine is `192.168.0.10` and the sender machine is
`192.168.0.20`:

```bash
# --- WebSocket:  receiver listens -------------------------------------------
# on 192.168.0.10
cargo run --example assess_ws_receiver -- --port 9301
# on 192.168.0.20
cargo run --example assess_ws_sender -- --url ws://192.168.0.10:9301/

# --- FTP:  receiver listens --------------------------------------------------
# on 192.168.0.10
cargo run --example assess_ftp_receiver -- --port 2121
# on 192.168.0.20
cargo run --example assess_ftp_sender -- --url ftp://xrds:assessment@192.168.0.10:2121/

# --- HTTP:  the SENDER listens (it serves the content) -----------------------
# on 192.168.0.20
cargo run --example assess_http_sender -- --port 9302
# on 192.168.0.10
cargo run --example assess_http_receiver -- --url http://192.168.0.20:9302

# --- CoAP:  the SENDER listens (UDP) -----------------------------------------
# on 192.168.0.20
cargo run --example assess_coap_sender -- --port 5683
# on 192.168.0.10
cargo run --example assess_coap_receiver -- --url coap://192.168.0.20:5683

# --- MQTT:  no listener; both reach the broker -------------------------------
# on 192.168.0.10
cargo run --example assess_mqtt_receiver -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1
# on 192.168.0.20
cargo run --example assess_mqtt_sender -- --url mqtt://172.16.1.55:1883/xrds/assessment/run1
```

Notes for an on-site run:

- **Both machines need the asset files.** The suite is resolved from paths
  relative to the crate root on each side, so run from a checkout with
  `assets/` present on both. (Only the sender reads them; the receiver just
  needs the repo to build.)
- **Firewall.** Windows prompts on first bind; allow it for the private
  network, or pre-authorise the ports in the table. CoAP is **UDP** — a rule
  that only covers TCP will silently produce timeouts (the receiver's retry
  messages are the symptom).
- **MQTT broker.** Both nodes just need to reach it; it does not have to be
  either of them. The default is the KETI lab broker `172.16.1.55` (verified on
  both `1883` and `11883`). Any other broker works by changing the URL on both
  sides — a public one is a poor choice, as the timings above show.
- **The receiver writes to its own machine**, under
  `examples/assessment/received/<protocol>/<run-id>/` — the files, `report.txt`
  and `result.json` together (see "The evidence on disk"). Collect that folder
  from the receiving machine for the evidence pack.
- **A port that is already in use** is handled: every binary that binds checks
  the port first and moves to the next free one (up to 10), printing
  `note: port 9301 is unavailable — using 9302 instead`. The command it prints
  for the other machine carries the port it settled on. `--strict-port` exits
  with the OS error instead of moving.

  Without that check the failure was invisible on the machine that caused it:
  `XRNetServer` (WS) and libunftp both bind inside a spawned task and swallow
  the error, so the receiver printed "waiting ..." forever while the *sender*
  failed with "connection refused".
- **Verified cross-interface:** the whole suite was run against this machine's
  own LAN address rather than `127.0.0.1` for `ws`, `ftp`, `http` and `coap` — 5/5 PASS each, which exercises the same non-loopback path
  a second machine takes, including FTP's passive-mode address negotiation.

## Recorded results (2026-09-15, Windows 11, loopback)

25/25 transfers PASS — every content type over every protocol, all
byte-identical (size and SHA-256 both matched).

| content type ↓ / protocol → | ws | mqtt | ftp | http | coap |
|---|---|---|---|---|---|
| gltf-model (30,985 B) | PASS | PASS | PASS | PASS | PASS |
| texture (2,217 B) | PASS | PASS | PASS | PASS | PASS |
| audio (88,244 B) | PASS | PASS | PASS | PASS | PASS |
| video (2,943 B) | PASS | PASS | PASS | PASS | PASS |
| scene-document (4,002 B) | PASS | PASS | PASS | PASS | PASS |
| **whole suite, 128,391 B** | **0.046 s** | **0.13 s** | **0.046 s** | **0.011 s** | **0.075 s** |

The MQTT column was run twice: **0.13 s** through the lab broker
(`172.16.1.55:1883`, and again on `11883` — both PASS) and **35.6 s** through
the public `test.mosquitto.org`. The figure above is the lab broker, since the
public one measures the internet rather than the SDK.

The spread in the last row is the interesting part, and it is a property of the
API rather than of the protocols:

- **ws / ftp / http** are all sub-0.05 s — one connection (or one request) per
  item.
- **coap** is 0.075 s at the 16 KiB default (11 requests). It was 0.964 s while
  this harness chunked at 512 bytes — 253 requests, each paying a round trip
  *and a fresh tokio runtime* (finding 4). The chunk size was the harness's
  conservatism, not a CoAP limit.
- **mqtt** is 0.13 s against a LAN broker, where `dispatch`'s reconnect-per-chunk
  costs ~3 ms. The same code took 35.6 s against `test.mosquitto.org`, where
  each reconnect carries an internet round trip (finding 2) — a 270× spread
  from the broker alone, with the SDK unchanged.

Single-file runs, kept because they show behaviour at size: a 19.3 MB video over
`ws` (295 chunks, 12.4 MB/s), a 5.3 MB MP3 over `http` (0.017 s), and a 1.6 MB
GLB over `ftp` (0.056 s) and `coap` (100 chunks, **0.718 s**, ~2.2 MiB/s, 0
retries) — all PASS. That CoAP figure was 11.5 s at the old 512-byte chunk size;
same code, same file, 16× apart on chunking alone.

### Interoperability: a third-party CoAP server

The CoAP results above use this repo's own peer node, so they could in
principle be two halves of one bug. They are not: `XrdsNet::request` was also
run against an **independent libcoap server** (`172.16.1.55:15683`, unrelated to
this project) and read it correctly —

- `GET /.well-known/core` → 2.05, the RFC 6690 discovery listing
  (`</>`, `</time>`, `</async>`, `</example_data>`),
- `GET /` → 2.05, libcoap's banner,
- `GET /time` → 2.05, `Sep 15 07:37:07`,
- `GET /example_data` → 2.05, 1,500 bytes intact — larger than one datagram,
  which is what first showed block-wise transfer works through this client.

Those reads were made with a small probe binary that has since been removed —
`prove-protocols.ps1` now sends a raw CoAP GET of its own, which is both
stronger evidence (no code of ours on either side of the packet) and the
quickest way to tell a blocked UDP port from a missing resource.

## Proving which protocol carried the bytes

The suite reports prove the bytes arrived intact. They do not, by themselves,
prove *which protocol* carried them — both ends are this repo's code, so a
sceptical reader can ask whether that was five protocols or one. Two answers,
which are stronger together:

**1. Independent clients** — `prove-protocols.ps1` (also in both kits) points a
client written by somebody else at each endpoint. Each only succeeds if the
endpoint implements the real protocol:

| protocol | independent client | what it got |
|----------|--------------------|-------------|
| ws | .NET `ClientWebSocket` | RFC 6455 handshake completed, socket `Open` |
| http | PowerShell `Invoke-WebRequest` | `HTTP 200 OK` with headers |
| ftp | Windows' built-in `ftp.exe` | logged in, `257` reply to `PWD` |
| coap | a raw CoAP `CON GET`, built byte by byte in the script | `ACK 2.05 Content` |
| mqtt | a raw MQTT 3.1.1 `CONNECT` packet, built byte by byte in the script | `CONNACK` with return code 0 |

Verified 5/5 against live endpoints. It needs no Administrator and no capture
tooling, and writes `protocol-proof.txt` to keep with the run's evidence.

The MQTT row is worth reading twice: the broker is a third-party server
(mosquitto), and it relayed the suite. Only a real MQTT client can make that
happen. CoAP has the same independent witness in reverse — this repo's client
reads the lab's libcoap server correctly (see "Interoperability" above).

**2. A packet capture** — `WIRESHARK.md` (also in both kits) has the capture
filters, the `Decode As` settings the non-standard ports need, the display
filter per protocol, and what to point at in each: the `101 Switching
Protocols` upgrade for WS, `Publish Message` per chunk for MQTT, `STOR` plus
the `FTP-DATA` stream for FTP, `200 OK` payload fetches for HTTP, `ACK 2.05
Content` for CoAP. Capture a genuine two-machine run: Windows short-circuits
traffic to its own address, so a single-machine run shows nothing on the
Ethernet adapter.

## Findings worth carrying into the assessment write-up

These are limitations of the library the runs exposed, not of the harness:

1. **QUIC cannot be assessed through the public API.** `quic://` is the protocol
   `XrdsNet::open` was built for, but a QUIC client config always verifies the
   peer (`quic_shared.rs`) and the one escape hatch, `ClientContext::insecure`,
   is `pub(crate)` with no setter — it is reachable only from inside the crate,
   which is how `quic.rs`'s own round-trip test uses it. So an example can only
   speak QUIC to a server holding a publicly-trusted certificate. Exposing an
   explicit opt-in (a `?insecure=true` URL option, or `XrdsNet::open_with`)
   would make QUIC assessable the same way the other five are.
2. **`XrdsNet::dispatch` opens one connection per call**, but it costs far less
   than it first appeared. It is a one-shot verb (connect, publish, drop), so a
   chunked payload opens one broker connection per chunk. Against the public
   test broker that was ~0.8 s per chunk and 35.6 s for the suite — but the
   same suite through a broker on the LAN takes **0.13 s**, so almost all of
   that was internet round-trip to a shared public server, not the reconnect
   itself (~3 ms each locally). The reconnect is still the reason MQTT scales
   badly with chunk count over any high-latency link; a streaming publish, or
   reusing the expert `Client` across chunks, would remove it. **Assess against
   a local broker**, or the number measures the internet rather than the SDK.
3. **HTTP request bodies must be valid UTF-8.** `RequestOptions::body` is
   `Vec<u8>`, but `XrdsNet::request` converts it to a `String` and errors
   otherwise (`net_intent.rs`), so a binary POST upload is not expressible —
   which is why the HTTP pair assesses the pull direction. Passing the bytes
   through to the body would remove the restriction.
4. **CoAP response bodies must be valid UTF-8, and each request builds a tokio
   runtime.** `CoapHandler::request_coap` runs the payload through
   `String::from_utf8` and returns an empty body with an error for anything
   else (`protocols/coap.rs`), so raw asset bytes cannot survive the trip — the
   CoAP pair base64-encodes each chunk to work around it, at 33% overhead. The
   same function also builds a fresh `tokio::runtime::Runtime` per request.
   Returning the payload bytes unmodified, and reusing one runtime, would fix
   both. Block-wise transfer, by contrast, is *not* a problem: it works through
   this client, so the per-request cost is paid per chunk-size rather than per
   datagram, and a larger `--chunk-size` amortises it (1.6 MB in 0.7 s at 16 KiB
   versus 11.5 s at 512 B).
5. **The WS server logs every message payload in full** (`ws_server.rs` prints
   the received `Message`), so a multi-megabyte transfer floods the terminal.
   Harmless, but it dominates the receiver's output.
6. **`XrdsNet::open` over `ws://` works**, contradicting its doc comment
   ("`quic://` today"). `WsHandler` implements `SessionHandler` and the docs in
   `net_intent.rs` were not updated.
7. **The FTP server bound loopback only** (`server.rs`), which made a
   cross-machine FTP run impossible. Fixed here by adding
   `XRNetServer::set_bind_address`, opt-in so the default is unchanged.

## Files

| file | role |
|------|------|
| `assess_common.rs` | shared support: content vocabulary, default suite, manifest, frame format, integrity check, suite report. No `main`; included with `#[path]`, like `examples/webrtc/realnet_common.rs` |
| `ws_sender.rs` / `ws_receiver.rs` | WebSocket pair |
| `mqtt_sender.rs` / `mqtt_receiver.rs` | MQTT pair |
| `ftp_sender.rs` / `ftp_receiver.rs` | FTP pair |
| `http_sender.rs` / `http_receiver.rs` | HTTP pair (sender is the content server) |
| `coap_sender.rs` / `coap_receiver.rs` | CoAP pair (sender is the content server) |
| `fixtures/assessment_scene.xrds` | the `scene-document` item, produced by the SDK's own document serializer |

Wire format for the push transports (WS, MQTT): per item, a manifest frame,
then chunk frames, then an end frame — each a 14-byte header (`XRDS` magic,
version, kind, sequence, length) plus payload, with the manifest's
`suite_index` / `suite_size` telling the receiver when the suite is complete.
The pull transports expose the same information as resources instead:
`/count` and `/item/<i>/manifest` for HTTP and CoAP, and a
`<name>.xrds-manifest.json` sidecar per item for FTP.
