# XRDS assessment — SENDER kit

The sending half: the five sender binaries plus the content they transfer. Run
the `.exe` files **from inside this folder** — the content files are found by
relative path.

The other machine gets `assessment-receiver-kit/`, which holds the five
receivers. No binary appears in both folders; the two kits are disjoint, so
there is nothing to keep in sync between them beyond rebuilding both.

Nothing to install, no Rust toolchain, no clone.

Windows x64 only. These are debug builds, so throughput figures are
conservative; what is being verified is correctness (every byte arrives), not
peak speed.

## What it tests

A **content × protocol matrix**: five XR content types × five protocols = 25
transfers. One run of a sender/receiver pair carries the *whole content suite*
over one protocol, so five pairs of commands cover the matrix.

Each sender declares a manifest per item (content type, file name, size,
SHA-256); each receiver verifies what it actually got against it, prints a suite
report, and exits non-zero if anything failed.

| content type | file | size |
|--------------|------|------|
| gltf-model | `samples/StainedGlassLamp.gltf` | 30,985 B |
| texture | `samples/arrow_box.png` | 2,217 B |
| audio | `samples/spatial_test_ping.wav` | 88,244 B |
| video | `samples/tiny_test.mp4` | 2,943 B |
| scene-document | `samples/assessment_scene.xrds` | 4,002 B |

All five live in the `samples/` folder beside these executables — one file per
content type, 128 KB in total. `samples/README.md` lists their SHA-256 hashes
and licensing.

## Which machine runs what

The two pull protocols invert the roles: HTTP and CoAP are request-shaped, so
the node that *serves* is the one holding the content.

| protocol | listens (must be reachable / firewall) | dials out |
|----------|---------------------------------------|-----------|
| ws | **receiver** — TCP 9301 | sender |
| ftp | **receiver** — TCP 2121 **and** passive data ports 49152–65535 | sender |
| http | **sender** — TCP 9302 | receiver |
| coap | **sender** — UDP 5683 | receiver |
| mqtt | neither — **both** dial the broker on TCP 1883 | both |

## Running (two machines)

Receiver machine `192.168.0.10` (running the **receiver kit**), sender machine
`192.168.0.20` (this kit). Start the listening side first in each pair. To try
both halves on one machine, run the receiver from its own kit folder and use
`127.0.0.1`.

**You should not have to look up an IP.** Whichever side listens prints the
exact command for the other machine with its own address filled in — the ws/ftp
receivers and the http/coap senders all do this on startup.

```bat
:: --- WebSocket: receiver listens -----------------------------------------
:: on 192.168.0.10
assess_ws_receiver.exe --port 9301
:: on 192.168.0.20
assess_ws_sender.exe --url ws://192.168.0.10:9301/

:: --- FTP: receiver listens (all interfaces; --bind 127.0.0.1 = local only) --
:: on 192.168.0.10
assess_ftp_receiver.exe --port 2121
:: on 192.168.0.20
assess_ftp_sender.exe --url ftp://xrds:assessment@192.168.0.10:2121/

:: --- HTTP: the SENDER serves, so start it first ---------------------------
:: on 192.168.0.20
assess_http_sender.exe --port 9302
:: on 192.168.0.10
assess_http_receiver.exe --url http://192.168.0.20:9302

:: --- CoAP: the SENDER serves (UDP) ----------------------------------------
:: on 192.168.0.20
assess_coap_sender.exe --port 5683
:: on 192.168.0.10
assess_coap_receiver.exe --url coap://192.168.0.20:5683

:: --- MQTT: no listener; both reach the broker. Fresh topic per run. -------
:: on 192.168.0.10
assess_mqtt_receiver.exe --url mqtt://172.16.1.55:1883/xrds/assessment/run1
:: on 192.168.0.20
assess_mqtt_sender.exe --url mqtt://172.16.1.55:1883/xrds/assessment/run1
```

`172.16.1.55` is the KETI lab broker; `1883` and `11883` both work, and neither
machine has to host it. Use a **new topic name for every run** — MQTT retains
the last message on a topic.

All five protocols finish the suite in well under a second against that broker
(MQTT: 0.13 s). Avoid public brokers such as `test.mosquitto.org`: the same
suite takes 35 s there, because the API reconnects per chunk and each reconnect
then carries an internet round trip.

## Results

**The receiving machine writes the transferred files, and they are the proof.**
Inside this folder, one directory per run so nothing is overwritten:

```text
examples/assessment/received/<protocol>/<run-id>/
    <the received files>     byte-identical to the originals
    report.txt               the printed report, verbatim
    result.json              the same, machine-readable, per item
```

`report.txt` names **both machines**, which is what makes it evidence of a
transfer between two nodes rather than a local copy:

```text
  run id        : a40a2019
  sent from     : host SENDER-PC (192.168.0.20) -> ws://192.168.0.10:9301/
  received on   : host RECEIVER-PC (192.168.0.10) <- ws://0.0.0.0:9301/
  gltf-model      StainedGlassLamp.gltf   30985   1   0.004 s  PASS
                  sha256 ddebbf01… (byte-identical (size + sha256 match))
  RESULT        : 5/5 PASS
```

Collect that folder from the **receiving** machine for the evidence pack. The
console prints the same report. (The `examples/assessment/` nesting is the
repo-relative default the binaries share with the source tree; `--out-dir
<path>` on a receiver puts it elsewhere.)

## Variations

```bat
:: one content type, one file
assess_ws_sender.exe --url ws://192.168.0.10:9301/ --file D:\my_modelsig_model.glb --content-type gltf-model

:: a custom suite (types inferred from the extension unless --types is given)
assess_ws_sender.exe --url ws://192.168.0.10:9301/ --files samples/arrow_box.png,samples/tiny_test.mp4

:: any file you supply — put it in this folder and point --file at it
assess_http_sender.exe --port 9302 --file my_asset.glb --content-type gltf-model
```

Content types are exactly: `gltf-model`, `texture`, `audio`, `video`,
`scene-document`. Anything else is rejected.

Every binary takes `--help`.

## If a port is taken

Every binary that binds a port checks it **before** announcing itself, and moves
to the next free port (up to 10) if it cannot have the one asked for:

```text
  note: port 9301 is unavailable — using 9302 instead (pass --strict-port to fail instead of moving on)
```

The command it prints for the other machine already carries the port it settled
on, so just use that line. `--strict-port` refuses to move and exits with the
OS error instead — use it when a specific port is the point.

This matters because two of these servers bind inside a spawned task and
**swallow** the error: without the check, a taken port left the receiver
printing "waiting ..." forever while the sender failed with "connection
refused" on the other machine.

## Proving which protocol was used

The reports prove the bytes arrived; they do not prove which protocol carried
them. Two ways to close that gap, both included here:

- `prove-protocols.ps1` — points an **independent client** at each endpoint
  (.NET WebSocket, PowerShell HTTP, Windows `ftp.exe`, a hand-built MQTT
  CONNECT packet, the CoAP probe). Run it while the endpoints are up:

  ```bat
  powershell -ExecutionPolicy Bypass -File prove-protocols.ps1 -SenderIp 192.168.0.20 -ReceiverIp 192.168.0.10
  ```

  It writes `protocol-proof.txt` — keep it with the run's evidence. No
  Administrator needed.

- `WIRESHARK.md` — capture filters, the `Decode As` settings the non-standard
  ports need, and the display filter and packets to point at per protocol.

## If something goes wrong

- **"connection refused" on the sender** — the listening side isn't up, the
  firewall blocked it, or the port moved (use the command the listening side
  printed). For FTP also check that the passive data ports 49152-65535 are
  open: the control connection succeeds and the *data* connection is refused.
- **CoAP times out and retries** — CoAP is **UDP**; a firewall rule covering
  only TCP will not let it through — `prove-protocols.ps1` sends a raw CoAP GET
  and reports whether anything answers. On a lossy link, lower the sender's
  `--chunk-size` (16384 by default).
- **A port was already in use** — the binary says so and moves to the next one;
  use the command it prints. With `--strict-port` it exits instead.
- **MQTT receives nothing** — check outbound TCP 1883 (or 11883) is allowed, and that both
  sides were given the *same* topic, and that both can reach the broker host.
  Any broker works: point both at `mqtt://<broker-ip>:1883/xrds/assessment/run1`.
  No internet is needed.
