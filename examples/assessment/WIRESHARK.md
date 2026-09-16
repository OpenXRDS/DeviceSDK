# Capturing the assessment traffic in Wireshark

Why bother: the suite reports prove the *bytes* arrived intact, but both ends
are our code, so they do not by themselves prove which protocol carried them. A
capture settles it — the evaluator sees `MQTT Publish`, `WebSocket Binary`,
`FTP STOR`, `HTTP GET` and `CoAP 2.05` frames on the wire.

`prove-protocols.ps1` answers the same question without a capture, by having an
independent client of each protocol talk to our endpoints. Do both if the
report needs to stand on its own.

## Where to capture

Capture on the **receiving machine** (or either — the traffic is the same), on
the adapter carrying the LAN address you used in the URLs: *Ethernet* or
*Wi-Fi*, not *Adapter for loopback traffic capture*.

One thing to know before you try this on a single machine: Windows short-
circuits traffic to its own address, so a run where both sides are the same box
shows **nothing** on the Ethernet adapter. It needs Npcap's loopback adapter,
and even then FTP's data channel can be missed. **Capture a genuine two-machine
run** — which is what the assessment is about anyway.

## Two filter boxes, two different syntaxes

This catches everyone once. Wireshark has two filters and they are **not** the
same language:

| | where | syntax | example |
|---|-------|--------|---------|
| **Capture** filter | *Capture > Options*, before starting | BPF — spaces, no dots, no `==` | `tcp port 9301` |
| **Display** filter | the bar above the packet list, after capturing | Wireshark fields | `tcp.port == 9301` |

Typing `tcp.port == 9301` into the *capture* box turns it red and captures
nothing. Typing `tcp port 9301` into the *display* box is also rejected. When in
doubt, capture with **no** capture filter at all and narrow afterwards with a
display filter — you only lose disk space.

## Capture filter (typed before you start, "Capture > Options")

Narrow, so the file stays small and the evidence stays readable. Use the ports
you actually ran with:

```text
tcp port 9301 or tcp port 2121 or tcp port 9302 or tcp port 1883 or udp port 5683 or (tcp portrange 49152-65535)
```

The last clause is FTP's passive data channel — drop it and the file transfer
itself is missing from the capture, leaving only the control commands.

Per protocol, if you would rather capture one at a time (cleaner evidence, one
file per protocol — recommended):

| protocol | capture filter |
|----------|----------------|
| ws | `tcp port 9301` |
| mqtt | `tcp port 1883 and host 172.16.1.55` |
| ftp | `tcp port 2121 or (tcp portrange 49152-65535)` |
| http | `tcp port 9302` |
| coap | `udp port 5683` |

## Make Wireshark decode the non-standard ports

Wireshark keys its dissectors off well-known port numbers. Three of ours are
not well-known, so without this you see raw `TCP` and nothing else. Right-click
any packet of the conversation → **Decode As…** → set:

| port | field to set | decode as |
|------|--------------|-----------|
| 9301 (ws) | TCP port | **HTTP** — the WebSocket dissector takes over automatically after the `101` upgrade |
| 9302 (http) | TCP port | **HTTP** |
| 2121 (ftp) | TCP port | **FTP** |
| 5683 (coap) | UDP port | already CoAP by default |
| 1883 (mqtt) | TCP port | already MQTT by default |
| 15683 (the lab libcoap server) | UDP port | **CoAP** |

Press *Save* in the Decode As dialog so it survives a restart.

FTP's data connection needs no rule: once the control channel is decoded as
FTP, Wireshark reads the `227 Entering Passive Mode` reply and follows the
data ports itself, labelling them `FTP-DATA`.

## Display filters, and what to point at

Set these after capturing, in the filter bar.

| protocol | display filter | what proves it |
|----------|----------------|----------------|
| ws | `websocket` | the `GET / HTTP/1.1` with `Upgrade: websocket`, the `101 Switching Protocols`, then `WebSocket Binary` frames — our manifest/chunk/end frames, each followed by a small ack frame the other way |
| mqtt | `mqtt` | `Connect Command` / `Connect Ack`, then `Publish Message` carrying the topic `xrds/assessment/...`, each answered by `Publish Ack`. One TCP connection **per chunk** — that is the reconnect cost finding 2 describes, visible on the wire |
| ftp | `ftp or ftp-data` | `USER xrds`, `PASS`, `227 Entering Passive Mode`, `STOR StainedGlassLamp.gltf`, `226 Transfer complete`, and the payload on the `FTP-DATA` stream |
| http | `http` | `GET /count`, `GET /item/0/manifest`, `GET /item/0/payload` and the `200 OK` responses with `Content-Type: application/octet-stream` |
| coap | `coap` | `CON GET /item/0/chunk/0` and `ACK 2.05 Content`, with `Block2` options when a chunk spans blocks |

Useful extras:

- `mqtt.msgtype == 3` — publishes only, one per chunk.
- `websocket.opcode == 2` — binary frames only.
- `ftp.request.command == "STOR"` — the uploads.
- `coap.code == 69` — 2.05 Content responses (69 = 2·32+5).
- `frame contains "assessment_scene"` — find a specific content item by name in
  any protocol that carries the name in the clear.

## Nothing appears in the capture

Work down this list; the first two account for almost every case.

1. **Filter syntax in the wrong box** — see "Two filter boxes" above.
   `tcp.port==9301` as a *capture* filter captures nothing.
2. **Both ends on one machine.** Windows never puts that traffic on the
   Ethernet/Wi-Fi adapter. Capture on *Adapter for loopback traffic capture*
   (Npcap), or run the two sides on two machines.
3. **The port moved.** Receivers fall back when a port is busy and print
   `note: port 9301 is unavailable — using 9302 instead`. Check the first line
   the binary printed, or `netstat -an | findstr 930`.
4. **Wrong adapter.** If the machine has several (Ethernet, Wi-Fi, VPN, Hyper-V
   vEthernet), pick the one holding the IP you used in the URL — `ipconfig`
   tells you which.
5. **Capturing after the fact.** These runs take well under a second; start the
   capture *before* the sender. Use `--keep` on the HTTP/CoAP senders to leave
   the endpoint up while you set Wireshark going.
6. **Traffic really is absent** — the transfer failed. The suite report on the
   receiving machine says so; fix that first.

## Saving it as evidence

1. **File > Save As** → `evidence/<protocol>.pcapng`, next to the run's
   `report.txt`, so the capture and the report share a run id.
2. For a written report: **File > Export Packet Dissections > As Plain Text**,
   with *Packet summary line* ticked and the display filter applied — that
   gives a readable list of the protocol frames rather than a binary file.
3. **Statistics > Conversations** shows byte counts per connection; they should
   match the suite report's totals, allowing for protocol overhead and, for
   CoAP, the base64 expansion.

## Cross-checking against the report

The run's `report.txt` names both machines and every file with its SHA-256. The
capture shows the same exchange on the wire. Two things line up neatly if you
want to make the point precisely:

- the number of `WebSocket Binary` frames (or MQTT publishes, or CoAP GETs)
  matches the chunk counts printed in the report;
- the byte totals in *Statistics > Conversations* track the suite total
  (128,391 bytes for the default suite).
