# XRDS assessment — RECEIVER kit (copy this to the other machine)

Everything the **receiving** machine needs, and nothing it doesn't. Copy this
folder, run the binaries from inside it, collect the evidence it writes.

No content files here on purpose: only the sender reads the files being
transferred. This side writes what it is given.

Windows x64, no install, no Rust toolchain. The sending machine runs the other
half, `assessment-sender-kit/`, which holds the senders and the content. No
binary appears in both folders.

## Run each protocol

Only the two pull protocols take the sender's address (`--url`); the listening
receivers take a `--port` instead, and it is the *sender* that types this
machine's address. Replace `SENDER-IP` below with the sending machine's IP.
Start whichever side listens first — see the table below.

**You should not have to look up an IP.** Whichever side listens prints the
exact command to run on the other machine, with its own address filled in:

```text
  on the sending machine, run:
      assess_ws_sender.exe --url ws://172.16.1.51:9301/
```

```bat
:: WebSocket — this machine listens on TCP 9301
assess_ws_receiver.exe --port 9301

:: FTP — this machine listens on TCP 2121 (+ passive 49152-65535)
assess_ftp_receiver.exe --port 2121

:: HTTP — the SENDER listens; start it there first, then pull from here
assess_http_receiver.exe --url http://SENDER-IP:9302

:: CoAP — the SENDER listens (UDP); start it there first
assess_coap_receiver.exe --url coap://SENDER-IP:5683

:: MQTT — neither listens; both reach the broker. Same topic on both sides.
assess_mqtt_receiver.exe --url mqtt://172.16.1.55:1883/xrds/assessment/run1
```

Each run transfers the whole content suite — five XR content types — and ends
with `RESULT : 5/5 PASS`.

## Who listens, and what to open in the firewall

Both machines' addresses land in the evidence either way: each side records its
own IP on the route to the other, and the sender's crosses in the manifest.

| protocol | listens | so open, on |
|----------|---------|-------------|
| ws | **this machine** | TCP 9301 here |
| ftp | **this machine** | TCP 2121 **and** passive 49152–65535 here |
| http | the sender | TCP 9302 on the sending machine |
| coap | the sender | **UDP** 5683 on the sending machine |
| mqtt | neither | outbound TCP 1883 from both |

FTP listens on all interfaces by default here; `--bind 127.0.0.1` restricts it
to this machine.

## The evidence this machine writes

One folder per run, under this folder:

```text
received/<protocol>/<run-id>/
    <the received files>     byte-identical to the originals
    report.txt               the printed report, verbatim
    result.json              the same, machine-readable, per item
```

`report.txt` names both machines, which is what makes it proof of a transfer
between two nodes rather than a local copy:

```text
  run id        : a40a2019
  sent from     : host SENDER-PC (192.168.0.20) -> ws://192.168.0.10:9301/
  received on   : host RECEIVER-PC (192.168.0.10) <- ws://0.0.0.0:9301/
  gltf-model      StainedGlassLamp.gltf   30985   1   0.004 s  PASS
                  sha256 ddebbf01… (byte-identical (size + sha256 match))
  RESULT        : 5/5 PASS
```

Collect `received/` from **this** machine for the evidence
pack. `--out-dir <path>` puts it somewhere else.

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
  (.NET WebSocket, PowerShell HTTP, Windows `ftp.exe`, and hand-built CoAP GET
  and MQTT CONNECT packets). Run it while the endpoints are up:

  ```bat
  powershell -ExecutionPolicy Bypass -File prove-protocols.ps1 -SenderIp 192.168.0.20 -ReceiverIp 192.168.0.10
  ```

  It writes `protocol-proof.txt` — keep it with the run's evidence. No
  Administrator needed.

- `WIRESHARK.md` — capture filters, the `Decode As` settings the non-standard
  ports need, and the display filter and packets to point at per protocol.

## If something goes wrong

- **Sender says "connection refused"** — this side isn't up, the firewall
  blocked it, or the port moved (use the command this side printed). For FTP
  also check that the passive data ports 49152-65535 are open: the control
  connection succeeds and the *data* connection is refused.
- **CoAP times out and retries** — CoAP is **UDP**; a TCP-only firewall rule
  won't pass it — `prove-protocols.ps1` sends a raw CoAP GET and reports whether
  anything answers.
- **A port was already in use** — the binary says so and moves to the next one;
  use the command it prints. With `--strict-port` it exits instead.
- **MQTT receives nothing** — check both sides use the *same* topic, that the
  broker is reachable, and that outbound TCP 1883 is allowed.

Every binary takes `--help`.
