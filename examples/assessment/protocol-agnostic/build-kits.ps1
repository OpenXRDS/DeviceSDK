<#
.SYNOPSIS
  Build the ten assessment binaries and assemble the two portable kits.

.DESCRIPTION
  Replaces the hand-copying the README used to describe. Every drift problem
  these kits have had came from assembling them by hand: content that no longer
  matched the binaries, a sample file nothing used any more, and a receiver kit
  that shipped seventeen folders of somebody else's results. A script cannot
  forget a step.

  Each kit is rebuilt from scratch, so anything left over from a previous
  assembly is gone rather than silently carried forward. The kits are disjoint:
  no binary appears in both.

    dist/assessment-sender-kit/     five senders + samples/ + docs
    dist/assessment-receiver-kit/   five receivers + docs

  Debug builds by default. What these binaries verify is that every byte
  arrives, not how fast — the throughput figures they print are conservative
  and labelled as such in RUN.md. Pass -Release if you want the faster ones.

.PARAMETER Release
  Build with --release instead of debug.

.PARAMETER SkipBuild
  Assemble from whatever is already in target/, without invoking cargo.

.EXAMPLE
  pwsh -File build-kits.ps1
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$KitDir = $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $KitDir '..\..\..')).Path
$Profile_ = if ($Release) { 'release' } else { 'debug' }
$BuiltDir = Join-Path $RepoRoot "target\$Profile_\examples"
$DistDir = Join-Path $RepoRoot 'dist'

$Senders = @('assess_ws_sender', 'assess_mqtt_sender', 'assess_ftp_sender',
    'assess_http_sender', 'assess_coap_sender')
$Receivers = @('assess_ws_receiver', 'assess_mqtt_receiver', 'assess_ftp_receiver',
    'assess_http_receiver', 'assess_coap_receiver')

# --------------------------------------------------------------------- build

if (-not $SkipBuild) {
    Write-Host "== building ($Profile_) ==" -ForegroundColor Cyan
    $args_ = @('build')
    if ($Release) { $args_ += '--release' }
    foreach ($e in ($Senders + $Receivers)) { $args_ += @('--example', $e) }
    Push-Location $RepoRoot
    try {
        & cargo @args_
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
}

foreach ($e in ($Senders + $Receivers)) {
    if (-not (Test-Path (Join-Path $BuiltDir "$e.exe"))) {
        throw "missing $e.exe in $BuiltDir — build first (drop -SkipBuild)"
    }
}

# ------------------------------------------------------------------ assemble

function New-Kit {
    param([string]$Name, [string[]]$Binaries, [switch]$WithSamples, [string]$RunDoc)

    $dest = Join-Path $DistDir $Name
    # From scratch every time: a kit that accumulates is a kit that lies.
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    New-Item -ItemType Directory -Force $dest | Out-Null

    foreach ($b in $Binaries) { Copy-Item (Join-Path $BuiltDir "$b.exe") $dest }

    # Each kit gets its own half of the instructions, named RUN.md on arrival.
    Copy-Item (Join-Path $KitDir "kit\$RunDoc") (Join-Path $dest 'RUN.md') -Force
    Copy-Item (Join-Path $KitDir 'WIRESHARK.md') $dest
    Copy-Item (Join-Path $KitDir 'prove-protocols.ps1') $dest

    if ($WithSamples) {
        # The binaries carry no content of their own; they look for samples/
        # beside the executable. This copy is the kit's entire payload.
        Copy-Item -Recurse (Join-Path $KitDir 'samples') (Join-Path $dest 'samples')
    }

    $bytes = (Get-ChildItem $dest -Recurse -File | Measure-Object -Property Length -Sum).Sum
    Write-Host ("   {0,-28} {1,2} binaries, {2,6:N1} MB" -f $Name, $Binaries.Count, ($bytes / 1MB))
    $dest
}

Write-Host "== assembling kits ==" -ForegroundColor Cyan
$senderKit = New-Kit -Name 'assessment-sender-kit' -Binaries $Senders -WithSamples -RunDoc 'RUN-sender.md'
$receiverKit = New-Kit -Name 'assessment-receiver-kit' -Binaries $Receivers -RunDoc 'RUN-receiver.md'

# ------------------------------------------------------------------ manifest

# Same reasoning as the reference-example assessment: a result is only
# checkable if you can tell whether the binary in front of you is the binary
# that produced it.
foreach ($kit in @($senderKit, $receiverKit)) {
    $rows = Get-ChildItem $kit -Recurse -File | ForEach-Object {
        [pscustomobject]@{
            file   = $_.FullName.Substring($kit.Length + 1)
            bytes  = $_.Length
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        }
    }
    $rows | Export-Csv -LiteralPath (Join-Path $kit 'MANIFEST.csv') -NoTypeInformation -Encoding utf8
}

Write-Host ""
Write-Host "sender kit  : $senderKit" -ForegroundColor Green
Write-Host "receiver kit: $receiverKit" -ForegroundColor Green
Write-Host "each carries MANIFEST.csv (SHA-256 of every file it ships)"
