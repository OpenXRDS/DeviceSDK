<#
.SYNOPSIS
  Proves each assessment endpoint really speaks its protocol, by talking to it
  with an INDEPENDENT client rather than with our own binaries.

.DESCRIPTION
  The suite reports prove the bytes arrived intact. They do not, on their own,
  prove which protocol carried them — both ends are our code, so a sceptical
  reader could ask whether it was five protocols or one.

  This script answers that without a packet capture (and without Administrator):
  for each protocol it uses a client written by somebody else, which only
  succeeds if the endpoint implements the real protocol.

    ws    .NET ClientWebSocket completes an RFC 6455 handshake
    http  PowerShell's HTTP stack gets a 200 with headers
    ftp   Windows' built-in ftp.exe logs in and gets a 257 PWD reply
    coap  a raw CoAP GET, built byte by byte here, is answered 2.05 Content
    mqtt  a raw MQTT 3.1.1 CONNECT packet, written byte by byte here,
          is answered with a CONNACK

  Run it while the endpoints are up. The listening side of each pair must be
  started first (see RUN.md); this script does not start anything itself.

.EXAMPLE
  .\prove-protocols.ps1 -SenderIp 192.168.0.20 -ReceiverIp 192.168.0.10
#>
[CmdletBinding()]
param(
    # Machine running the senders — it serves HTTP and CoAP.
    [string]$SenderIp = '127.0.0.1',
    # Machine running the receivers — it listens for WS and FTP.
    [string]$ReceiverIp = '127.0.0.1',
    [string]$Broker = '172.16.1.55',
    [int]$WsPort = 9301,
    [int]$FtpPort = 2121,
    [int]$HttpPort = 9302,
    [int]$CoapPort = 5683,
    [int]$BrokerPort = 1883,
    [string]$OutFile = 'protocol-proof.txt'
)

$lines = New-Object System.Collections.Generic.List[string]
function Say($text) { Write-Host $text; $lines.Add($text) }

Say "=== XRDS assessment: independent-client protocol proof ==="
Say ("  run at        : {0}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'))
Say ("  sender node   : {0} (serves http {1}, coap {2})" -f $SenderIp, $HttpPort, $CoapPort)
Say ("  receiver node : {0} (listens ws {1}, ftp {2})" -f $ReceiverIp, $WsPort, $FtpPort)
Say ("  mqtt broker   : {0}:{1}" -f $Broker, $BrokerPort)
Say ""

$results = @{}

# --- ws: RFC 6455 handshake by .NET -----------------------------------------
Say "[ws]   .NET ClientWebSocket -> ws://${ReceiverIp}:${WsPort}/"
try {
    $ws = New-Object System.Net.WebSockets.ClientWebSocket
    $ct = [Threading.CancellationToken]::None
    $null = $ws.ConnectAsync([Uri]"ws://${ReceiverIp}:${WsPort}/", $ct).Wait(8000)
    if ($ws.State -eq 'Open') {
        Say "       handshake completed, socket state Open -> real WebSocket"
        $results['ws'] = 'PASS'
    } else {
        Say "       state $($ws.State)"
        $results['ws'] = 'FAIL'
    }
    try { $null = $ws.CloseAsync('NormalClosure', 'done', $ct).Wait(2000) } catch {}
} catch {
    Say "       FAILED: $($_.Exception.Message)"
    $results['ws'] = 'FAIL'
}
Say ""

# --- http: PowerShell's own client ------------------------------------------
Say "[http] Invoke-WebRequest -> http://${SenderIp}:${HttpPort}/count"
try {
    $r = Invoke-WebRequest -Uri "http://${SenderIp}:${HttpPort}/count" -UseBasicParsing -TimeoutSec 10
    Say ("       HTTP {0} {1}; headers: {2}; body: {3}" -f `
        $r.StatusCode, $r.StatusDescription, ($r.Headers.Keys -join ','), $r.Content.Trim())
    $results['http'] = if ($r.StatusCode -eq 200) { 'PASS' } else { 'FAIL' }
} catch {
    Say "       FAILED: $($_.Exception.Message)"
    $results['http'] = 'FAIL'
}
Say ""

# --- ftp: Windows' built-in client ------------------------------------------
Say "[ftp]  ftp.exe -> ftp://${ReceiverIp}:${FtpPort}/"
try {
    $script = Join-Path $env:TEMP 'xrds-ftp-proof.txt'
    "open $ReceiverIp $FtpPort", 'user xrds assessment', 'pwd', 'quit' |
        Set-Content -Encoding ascii $script
    $out = (ftp -n -v -s:$script 2>&1) -join "`n"
    Remove-Item $script -ErrorAction SilentlyContinue
    # 257 is the PWD reply; reaching it means USER/PASS were accepted.
    if ($out -match '257') {
        Say "       logged in, server replied 257 to PWD -> real FTP control channel"
        $results['ftp'] = 'PASS'
    } else {
        Say "       unexpected transcript: $out"
        $results['ftp'] = 'FAIL'
    }
} catch {
    Say "       FAILED: $($_.Exception.Message)"
    $results['ftp'] = 'FAIL'
}
Say ""

# --- coap: a GET built here, byte by byte ------------------------------------
# Nothing of ours is involved on this side either: a CoAP server is what answers
# a CON GET with an ACK carrying 2.05 Content.
Say "[coap] raw CoAP GET -> coap://${SenderIp}:${CoapPort}/count"
try {
    $udp = New-Object System.Net.Sockets.UdpClient
    $udp.Client.ReceiveTimeout = 8000
    $udp.Connect($SenderIp, $CoapPort)

    $path = [Text.Encoding]::ASCII.GetBytes('count')
    $packet = New-Object System.Collections.Generic.List[byte]
    $packet.Add(0x40)                       # version 1, type CON, token length 0
    $packet.Add(0x01)                       # code 0.01 GET
    $packet.AddRange([byte[]]@(0x12, 0x34)) # message id
    $packet.Add([byte](0xB0 -bor $path.Length)) # option 11 (Uri-Path), length
    $packet.AddRange($path)

    $bytes = $packet.ToArray()
    $null = $udp.Send($bytes, $bytes.Length)

    $remote = New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Any, 0)
    $reply = $udp.Receive([ref]$remote)
    $udp.Close()

    # Response code lives in the second byte: 2.05 Content is (2 << 5) | 5 = 0x45.
    if ($reply.Length -ge 2 -and $reply[1] -eq 0x45) {
        $body = ''
        # Payload follows a 0xFF marker, when there is one.
        $marker = [Array]::IndexOf($reply, [byte]0xFF)
        if ($marker -ge 0 -and $marker + 1 -lt $reply.Length) {
            $body = [Text.Encoding]::ASCII.GetString($reply, $marker + 1, $reply.Length - $marker - 1)
        }
        Say "       ACK 2.05 Content received (payload '$body') -> real CoAP server"
        $results['coap'] = 'PASS'
    } else {
        Say ("       unexpected reply: {0}" -f (($reply | Select-Object -First 4 | ForEach-Object { '0x{0:X2}' -f $_ }) -join ' '))
        $results['coap'] = 'FAIL'
    }
} catch {
    Say "       FAILED: $($_.Exception.Message)"
    $results['coap'] = 'FAIL'
}
Say ""

# --- mqtt: a CONNECT packet built here, byte by byte -------------------------
# Nothing of ours is involved: if a CONNACK comes back, the endpoint is an MQTT
# broker, and the suite's traffic went through it.
Say "[mqtt] raw MQTT 3.1.1 CONNECT -> ${Broker}:${BrokerPort}"
try {
    $client = New-Object System.Net.Sockets.TcpClient
    $null = $client.ConnectAsync($Broker, $BrokerPort).Wait(8000)
    $stream = $client.GetStream()
    $stream.ReadTimeout = 8000

    $clientId = [Text.Encoding]::ASCII.GetBytes('xrds-proof')
    $payload = New-Object System.Collections.Generic.List[byte]
    $payload.AddRange([byte[]]@(0x00, 0x04))                       # protocol name length
    $payload.AddRange([Text.Encoding]::ASCII.GetBytes('MQTT'))     # protocol name
    $payload.Add(0x04)                                             # level 4 = 3.1.1
    $payload.Add(0x02)                                             # flags: clean session
    $payload.AddRange([byte[]]@(0x00, 0x3C))                       # keep-alive 60 s
    $payload.AddRange([byte[]]@(0x00, [byte]$clientId.Length))     # client id length
    $payload.AddRange($clientId)

    $packet = New-Object System.Collections.Generic.List[byte]
    $packet.Add(0x10)                                              # CONNECT
    $packet.Add([byte]$payload.Count)                              # remaining length
    $packet.AddRange($payload)

    $bytes = $packet.ToArray()
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()

    $reply = New-Object byte[] 4
    $read = $stream.Read($reply, 0, 4)
    # CONNACK is 0x20, then remaining length 2, then flags and a return code of 0.
    if ($read -ge 4 -and $reply[0] -eq 0x20 -and $reply[3] -eq 0x00) {
        Say "       CONNACK received (0x20 .. rc=0) -> real MQTT broker"
        $results['mqtt'] = 'PASS'
    } else {
        Say ("       unexpected reply: {0}" -f (($reply | ForEach-Object { '0x{0:X2}' -f $_ }) -join ' '))
        $results['mqtt'] = 'FAIL'
    }
    $client.Close()
} catch {
    Say "       FAILED: $($_.Exception.Message)"
    $results['mqtt'] = 'FAIL'
}
Say ""

Say "--- summary ---"
foreach ($p in 'ws', 'mqtt', 'ftp', 'http', 'coap') {
    Say ("  {0,-5} {1}" -f $p, $results[$p])
}
$passed = ($results.Values | Where-Object { $_ -eq 'PASS' }).Count
Say ""
Say "  RESULT: $passed/5 endpoints answered an independent client of their protocol"

$lines -join "`r`n" | Set-Content -Encoding utf8 $OutFile
Write-Host ""
Write-Host "  written to $OutFile — keep it with the run's evidence folder"
if ($passed -ne 5) { exit 1 }
