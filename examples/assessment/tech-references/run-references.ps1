<#
.SYNOPSIS
  Reference-example assessment: count the SDK's example programs, build them all,
  and run each one to a recorded outcome.

.DESCRIPTION
  Answers three questions with evidence a reader can re-check:

    1. How many reference examples ship with the SDK?
    2. Do they all build?
    3. Does each one actually run?

  The inventory is derived from the workspace Cargo.toml's [[example]] table,
  cross-checked against the files on disk, so a stray source file with no entry
  (or an entry with no file) is a reported failure rather than a silent omission.
  The protocol-agnostic transfer assessment's own examples (examples/assessment/)
  are excluded -- they are the *other* assessment's apparatus, not reference code.

  "Runs" is the hard part: most of these are windowed apps that never exit on
  their own, so an exit code does not exist for them by default. Two mechanisms
  fix that:

    * XRDS_EXIT_AFTER_FRAMES -- an opt-in hook in xrds-runtime. With it set, an
      app renders that many frames, prints XRDS_FRAME_LIMIT_REACHED with a frame
      count, and exits 0. "It runs" becomes an exit code plus a frame count.
    * A window screenshot, captured off the desktop while the app is up, so a
      human can see the thing rendered rather than take the exit code on faith.

  Binaries are staged into one directory together with the native DLLs they link
  (FFmpeg, quiche). `cargo run` injects those search paths itself; a bare
  target/debug/examples/*.exe does not get them and dies with STATUS_DLL_NOT_FOUND.
  Staging them side by side is both the fix and a more honest test: these are
  binaries, not just cargo invocations.

.PARAMETER Frames
  Frames to render before a windowed XRDS example exits. Default 600.

.PARAMETER SettleSec
  Seconds to wait after a window appears before photographing it, so the first
  frames (and any asset loading) are done. Default 4.

.PARAMETER TimeoutSec
  Per-example wall-clock limit. Default 120.

.PARAMETER Only
  Wildcard filter on example name, e.g. -Only 'scene_document_*'.

.PARAMETER SkipBuild
  Reuse whatever is already in target/debug/examples.

.PARAMETER SkipRun
  Inventory and build only.

.EXAMPLE
  pwsh -File run-references.ps1
  pwsh -File run-references.ps1 -Only 'net_*' -Frames 300
#>
[CmdletBinding()]
param(
    [int]$Frames = 600,
    [int]$SettleSec = 4,
    [int]$TimeoutSec = 120,
    [string]$Only = '*',
    [switch]$SkipBuild,
    [switch]$SkipRun,
    [switch]$IncludeHardware,
    [string]$RetryFrom = '',
    [string]$RunId = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$ExamplesDir = Join-Path $RepoRoot 'examples'
$CargoToml = Join-Path $RepoRoot 'Cargo.toml'
if (-not $RunId) { $RunId = (Get-Date -Format 'yyyyMMdd-HHmmss') }
$RunDir = Join-Path $PSScriptRoot "runs\$RunId"
# Shared across runs: debug binaries are large, and -SkipBuild is only useful if
# a previous build's staging survives.
$StageDir = Join-Path $PSScriptRoot 'runs\_stage'
$EvidenceDir = Join-Path $RunDir 'examples'

# Source files that legitimately have no [[example]] entry: shared support code
# with no `main`, pulled in by its siblings via #[path].
$SupportFiles = @('examples/webrtc/realnet_common.rs')

# Run modes. Everything is auto-classified from its source (see Get-RunMode);
# this table is only for the cases source inspection cannot see.
$ModeOverrides = @{
    'net_app_publisher'                 = 'service'   # publishes in an infinite loop by design
    'webrtc_file_stream'                = 'service'   # serves until stopped
    'webrtc_webcam_stream'              = 'hardware'  # needs a real camera + mic
    'webrtc_realnet_signaling_server'   = 'paired'
    'webrtc_realnet_publisher'          = 'paired'
    'webrtc_realnet_subscriber'         = 'paired'
}

# ---------------------------------------------------------------- win32 capture

Add-Type -AssemblyName System.Drawing

# A type can only be added once per PowerShell session, and .NET cannot replace
# one that is already loaded -- so running this script twice in the same session
# would otherwise fail on the second go with "type name already exists".
$capType = 'XrdsCap.Win' -as [type]
if (-not $capType) {
    Add-Type -Namespace XrdsCap -Name Win -MemberDefinition @'
public delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lparam);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
'@
} elseif (-not $capType.GetMethod('GetForegroundWindow')) {
    # Loaded by an older copy of this script. Reusing it would skip the check
    # that stops a capture photographing the wrong window, so stop instead.
    throw @'
An older version of the XrdsCap.Win helper is already loaded in this PowerShell
session, and .NET cannot replace a loaded type. Start a fresh session:

    pwsh -File .\run-references.ps1

(running the script with `pwsh -File` always gets its own process, so this
cannot happen)
'@
}

function Get-WindowRect {
    param([IntPtr]$Hwnd)
    $r = New-Object XrdsCap.Win+RECT
    if (-not [XrdsCap.Win]::GetWindowRect($Hwnd, [ref]$r)) { return $null }
    [pscustomobject]@{ X = $r.Left; Y = $r.Top; W = $r.Right - $r.Left; H = $r.Bottom - $r.Top }
}

# A winit app owns small helper windows before its real surface exists, so the
# first non-zero MainWindowHandle is not the one worth photographing. Take the
# largest visible top-level window the process owns.
function Get-MainWindow {
    param([int]$ProcessId, [int]$MinEdge = 200)
    # The delegate body runs in its own scope; the running best has to live
    # where both it and this function can see it.
    $script:capBest = [IntPtr]::Zero
    $script:capBestArea = 0
    $script:capPid = $ProcessId
    $script:capMinEdge = $MinEdge
    $cb = [XrdsCap.Win+EnumProc] {
        param([IntPtr]$hwnd, [IntPtr]$lparam)
        $owner = 0
        [XrdsCap.Win]::GetWindowThreadProcessId($hwnd, [ref]$owner) | Out-Null
        if ($owner -eq $script:capPid -and [XrdsCap.Win]::IsWindowVisible($hwnd)) {
            $rc = Get-WindowRect -Hwnd $hwnd
            if ($rc -and $rc.W -ge $script:capMinEdge -and $rc.H -ge $script:capMinEdge `
                    -and ($rc.W * $rc.H) -gt $script:capBestArea) {
                $script:capBestArea = $rc.W * $rc.H
                $script:capBest = $hwnd
            }
        }
        return $true
    }
    [XrdsCap.Win]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
    $script:capBest
}

# GPU-composited (Vulkan/DX) surfaces are not reliably readable through
# PrintWindow, but they are always readable off the desktop -- so bring the
# window forward and copy the pixels the screen is actually showing.
function Save-WindowPng {
    param([IntPtr]$Hwnd, [string]$Path)
    # Windows can refuse a foreground change it did not initiate. Copying the
    # screen regardless would photograph whatever is actually on top -- an editor,
    # a browser -- and file it as the example's output. Confirm the target really
    # came forward, and refuse to save anything if it did not.
    $front = $false
    for ($attempt = 1; $attempt -le 3 -and -not $front; $attempt++) {
        [XrdsCap.Win]::ShowWindow($Hwnd, 5) | Out-Null
        [XrdsCap.Win]::SetForegroundWindow($Hwnd) | Out-Null
        Start-Sleep -Milliseconds 700
        $front = ([XrdsCap.Win]::GetForegroundWindow() -eq $Hwnd)
    }
    if (-not $front) { return $null }
    $rc = Get-WindowRect -Hwnd $Hwnd
    if (-not $rc) { return $null }
    $bmp = New-Object System.Drawing.Bitmap $rc.W, $rc.H
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
        $g.CopyFromScreen($rc.X, $rc.Y, 0, 0, (New-Object System.Drawing.Size $rc.W, $rc.H))
        $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $g.Dispose(); $bmp.Dispose()
    }
    "$($rc.W)x$($rc.H)"
}

# Luminance spread over a sampled grid. A window photographed before its assets
# finished loading is a flat field of the clear colour, which scores ~0 -- worth
# distinguishing from a picture that actually shows something.
function Get-ImageVariance {
    param([string]$Path, [int]$Grid = 32, [int]$TitleBar = 30)
    $bmp = New-Object System.Drawing.Bitmap $Path
    try {
        $lum = [System.Collections.Generic.List[double]]::new()
        for ($i = 1; $i -lt $Grid; $i++) {
            for ($j = 1; $j -lt $Grid; $j++) {
                $x = [int]($bmp.Width * $i / ($Grid + 1))
                $y = [int]($TitleBar + ($bmp.Height - $TitleBar) * $j / ($Grid + 1))
                if ($x -lt $bmp.Width -and $y -lt $bmp.Height) {
                    $c = $bmp.GetPixel($x, $y)
                    $lum.Add(0.299 * $c.R + 0.587 * $c.G + 0.114 * $c.B)
                }
            }
        }
        if (-not $lum.Count) { return 0 }
        $mean = ($lum | Measure-Object -Average).Average
        $sum = ($lum | ForEach-Object { ($_ - $mean) * ($_ - $mean) } | Measure-Object -Sum).Sum
        [math]::Round([math]::Sqrt($sum / $lum.Count), 2)
    } finally {
        $bmp.Dispose()
    }
}

# ------------------------------------------------------------------- inventory

function Get-DeclaredExamples {
    $declared = [System.Collections.Generic.List[object]]::new()
    $cur = $null
    foreach ($line in (Get-Content $CargoToml)) {
        $t = $line.Trim()
        if ($t -eq '[[example]]') {
            if ($cur) { $declared.Add($cur) }
            $cur = [pscustomobject]@{ name = $null; path = $null }
            continue
        }
        if ($t.StartsWith('[')) {
            if ($cur) { $declared.Add($cur); $cur = $null }
            continue
        }
        if ($cur) {
            if ($t -match '^name\s*=\s*"(.+)"\s*$') { $cur.name = $Matches[1] }
            elseif ($t -match '^path\s*=\s*"(.+)"\s*$') { $cur.path = $Matches[1] }
        }
    }
    if ($cur) { $declared.Add($cur) }
    $declared | Where-Object { $_.name -and $_.path }
}

# Source inspection decides how an example must be driven. Kept mechanical so a
# newly added example is classified without editing this script.
function Get-RunMode {
    param([string]$Name, [string]$Source)
    if ($ModeOverrides.ContainsKey($Name)) { return $ModeOverrides[$Name] }
    if ($Source -match 'run_xrds|Runtime::new') { return 'frames' }
    if ($Source -match 'App::new\(\)') { return 'window-soft' }
    if ($Source -match '#\[tokio::main\]') { return 'service' }
    'oneshot'
}

function Build-Inventory {
    $readme = Get-Content (Join-Path $ExamplesDir 'README.md') -Raw
    $items = [System.Collections.Generic.List[object]]::new()

    foreach ($d in Get-DeclaredExamples) {
        if ($d.path -like 'examples/assessment/*') { continue }
        $full = Join-Path $RepoRoot ($d.path -replace '/', '\')
        $exists = Test-Path $full
        $source = if ($exists) { Get-Content $full -Raw } else { '' }
        $rel = $d.path -replace '^examples/', ''
        $items.Add([pscustomobject]@{
                name       = $d.name
                path       = $d.path
                category   = ($rel -split '/')[0]
                exists     = $exists
                loc        = if ($exists) { (Get-Content $full).Count } else { 0 }
                mode       = if ($exists) { Get-RunMode -Name $d.name -Source $source } else { 'missing' }
                documented = $readme.Contains("($rel)")
            })
    }

    # Undeclared sources: a reference example nobody can run is not a reference.
    $declaredPaths = @($items | ForEach-Object { $_.path })
    $orphans = Get-ChildItem $ExamplesDir -Recurse -Filter *.rs |
        ForEach-Object { ($_.FullName.Substring($RepoRoot.Length + 1) -replace '\\', '/') } |
        Where-Object { $_ -notlike 'examples/assessment/*' } |
        Where-Object { $declaredPaths -notcontains $_ } |
        Where-Object { $SupportFiles -notcontains $_ }

    [pscustomobject]@{
        run_id    = $RunId
        generated = (Get-Date).ToString('o')
        examples  = $items
        orphans   = @($orphans)
        missing   = @($items | Where-Object { -not $_.exists } | ForEach-Object { $_.path })
    }
}

# ----------------------------------------------------------------------- build

function Invoke-BuildAll {
    Write-Host "== building all examples ==" -ForegroundColor Cyan
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Push-Location $RepoRoot
    try {
        $json = & cargo build --examples --message-format=json 2> (Join-Path $RunDir 'build-stderr.txt')
        $code = $LASTEXITCODE
    } finally {
        Pop-Location
    }
    $sw.Stop()

    $artifacts = @{}
    $errors = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $json) {
        if (-not $line.StartsWith('{')) { continue }
        try { $m = $line | ConvertFrom-Json } catch { continue }
        if ($m.reason -eq 'compiler-artifact' -and $m.target.kind -contains 'example' -and $m.executable) {
            $artifacts[$m.target.name] = $m.executable
        } elseif ($m.reason -eq 'compiler-message' -and $m.message.level -eq 'error') {
            $errors.Add($m.message.rendered)
        }
    }
    Write-Host "   exit=$code, $($artifacts.Count) example artifacts, $([int]$sw.Elapsed.TotalSeconds)s"
    [pscustomobject]@{
        exit_code  = $code
        seconds    = [math]::Round($sw.Elapsed.TotalSeconds, 1)
        artifacts  = $artifacts
        errors     = @($errors)
    }
}

# Windows resolves a DLL from the executable's own directory first, so putting
# the binaries next to the natives they link makes them runnable on their own --
# which is what `cargo run` was quietly doing for us via PATH.
function Publish-Stage {
    param($Artifacts, $Inventory)
    New-Item -ItemType Directory -Force $StageDir | Out-Null
    $dlls = @()
    $buildOut = Join-Path $RepoRoot 'target\debug\build'
    if (Test-Path $buildOut) {
        $dlls += Get-ChildItem $buildOut -Recurse -Filter *.dll -ErrorAction SilentlyContinue
    }
    $deps = Join-Path $RepoRoot 'target\debug\deps'
    if (Test-Path $deps) {
        # Proc-macro DLLs carry a -<hash> suffix and are build-time only; a plain
        # name (quiche.dll) is a real runtime dependency.
        $dlls += Get-ChildItem $deps -Filter *.dll | Where-Object { $_.BaseName -notmatch '-[0-9a-f]{16}$' }
    }
    $copied = 0
    foreach ($grp in ($dlls | Group-Object Name)) {
        $newest = $grp.Group | Sort-Object LastWriteTime | Select-Object -Last 1
        Copy-Item $newest.FullName $StageDir -Force
        $copied++
    }
    $staged = 0
    foreach ($item in $Inventory.examples) {
        if ($Artifacts.ContainsKey($item.name)) {
            Copy-Item $Artifacts[$item.name] $StageDir -Force
            $staged++
        }
    }
    Write-Host "   staged $staged binaries + $copied native DLLs into bin\"
    [pscustomobject]@{ binaries = $staged; dlls = $copied }
}

# Identify exactly which binaries were executed. A report saying "62 examples
# passed" is only checkable if you can tell whether the thing you have in front
# of you is the thing that was run -- so each staged executable is fingerprinted
# at the moment of the run, not at build time. DLLs are deliberately not hashed:
# they are third-party natives copied in unchanged, and they are not what this
# assessment makes a claim about.
function Write-BinaryManifest {
    param($Inventory, [string]$Path)
    $rows = foreach ($item in $Inventory.examples) {
        $exe = Join-Path $StageDir "$($item.name).exe"
        if (-not (Test-Path $exe)) { continue }
        $f = Get-Item $exe
        [pscustomobject]@{
            example      = $item.name
            file         = $f.Name
            bytes        = $f.Length
            modified_utc = $f.LastWriteTimeUtc.ToString('o')
            sha256       = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        }
    }
    $rows = @($rows)
    $rows | Export-Csv -LiteralPath $Path -NoTypeInformation -Encoding utf8
    $rows.Count
}

# ------------------------------------------------------------------------- run

function Start-Staged {
    param([string]$Name, [string[]]$Arguments = @(), [hashtable]$EnvVars = @{}, [string]$Dir)
    New-Item -ItemType Directory -Force $Dir | Out-Null
    # Bevy resolves its asset root from the executable's own directory, not the
    # working directory -- so a staged binary would look for assets/ inside the
    # stage and silently render an empty scene. This points it back at the repo.
    $EnvVars['BEVY_ASSET_ROOT'] = $RepoRoot
    foreach ($k in $EnvVars.Keys) { Set-Item "env:$k" $EnvVars[$k] }
    $splat = @{
        FilePath               = (Join-Path $StageDir "$Name.exe")
        WorkingDirectory       = $RepoRoot     # assets/ paths are repo-root relative
        PassThru               = $true
        RedirectStandardOutput = (Join-Path $Dir 'stdout.txt')
        RedirectStandardError  = (Join-Path $Dir 'stderr.txt')
    }
    if ($Arguments.Count) { $splat.ArgumentList = $Arguments }
    $p = Start-Process @splat
    foreach ($k in $EnvVars.Keys) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
    $p
}

function Stop-Staged {
    param($Process)
    if (-not $Process.HasExited) {
        try { $Process.Kill($true) } catch { }
        $Process.WaitForExit(5000) | Out-Null
    }
}

function Read-Evidence {
    param([string]$Dir)
    $out = if (Test-Path "$Dir\stdout.txt") { Get-Content "$Dir\stdout.txt" -Raw } else { '' }
    $err = if (Test-Path "$Dir\stderr.txt") { Get-Content "$Dir\stderr.txt" -Raw } else { '' }
    [pscustomobject]@{
        stdout  = $out ?? ''
        stderr  = $err ?? ''
        panicked = (($out ?? '') + ($err ?? '')) -match 'panicked at|fatal runtime error'
    }
}

# Returns "<w>x<h>" on success, or a string explaining why no photograph exists.
# A missing screenshot is reported rather than silently dropped -- "we never got
# a picture" and "the picture was blank" are different findings.
function Wait-ForWindowAndShoot {
    param($Process, [string]$Dir, [int]$MaxWaitSec = 40, [int]$Attempts = 4, [double]$BlankBelow = 3.0)
    $deadline = (Get-Date).AddSeconds($MaxWaitSec)
    $hwnd = [IntPtr]::Zero
    while ((Get-Date) -lt $deadline -and -not $Process.HasExited) {
        Start-Sleep -Milliseconds 250
        $hwnd = Get-MainWindow -ProcessId $Process.Id
        if ($hwnd -ne [IntPtr]::Zero) { break }
    }
    if ($hwnd -eq [IntPtr]::Zero) {
        return [pscustomobject]@{
            size = $null; variance = $null; blank = $false
            skipped = if ($Process.HasExited) { 'exited before a window appeared' } else { "no window within ${MaxWaitSec}s" }
        }
    }
    # Settle in slices so an early exit is noticed instead of slept through.
    for ($i = 0; $i -lt ($SettleSec * 4); $i++) {
        if ($Process.HasExited) { break }
        Start-Sleep -Milliseconds 250
    }
    if ($Process.HasExited) {
        return [pscustomobject]@{
            size = $null; variance = $null; blank = $false
            skipped = "window closed during the ${SettleSec}s settle (raise -Frames)"
        }
    }

    # Asset-heavy examples reach their first meaningful frame well after the
    # window exists, so a flat capture is retried rather than filed as evidence.
    # If every attempt is flat the app really is drawing nothing, which is a
    # finding in its own right.
    $png = Join-Path $Dir 'window.png'
    $size = $null; $variance = $null
    for ($try = 1; $try -le $Attempts; $try++) {
        $shotSize = $null
        try {
            $shotSize = Save-WindowPng -Hwnd $hwnd -Path $png
        } catch {
            if (-not $size) {
                return [pscustomobject]@{ size = $null; variance = $null; blank = $false; skipped = "capture failed: $($_.Exception.Message)" }
            }
        }
        # A retry against a window that has since closed writes nothing and must
        # not discard the picture an earlier attempt already got.
        if ($shotSize) {
            $size = $shotSize
            $variance = Get-ImageVariance -Path $png
            if ($variance -ge $BlankBelow) { break }
        }
        if ($try -lt $Attempts -and -not $Process.HasExited) { Start-Sleep -Seconds 3 } else { break }
    }

    [pscustomobject]@{
        size     = $size
        variance = $variance
        blank    = ($size -and $variance -lt $BlankBelow)
        skipped  = if ($size) { $null } else { 'window would not come to the foreground; refused to photograph another window' }
    }
}

# Trim the captured terminal output down to something worth printing in the
# report -- for the examples that have no window, this IS the evidence.
function Get-LogExcerpt {
    param([string]$Text, [int]$Lines = 8)
    if (-not $Text) { return '' }
    $keep = $Text -split "`r?`n" |
        Where-Object { $_.Trim() } |
        Where-Object { $_ -notmatch '^\s*\d+: <unknown>$' } |
        Select-Object -First $Lines
    ($keep -join "`n")
}

function Invoke-Example {
    param($Item)
    $dir = Join-Path $EvidenceDir $Item.name
    New-Item -ItemType Directory -Force $dir | Out-Null
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $shot = $null
    $exit = $null
    $killed = $false

    switch ($Item.mode) {
        'frames' {
            # The title override makes the screenshot self-identifying: the photo
            # says which example produced it, not just "OpenXRDS".
            $env2 = @{ XRDS_EXIT_AFTER_FRAMES = "$Frames"; XRDS_WINDOW_TITLE = $Item.name }
            $p = Start-Staged -Name $Item.name -EnvVars $env2 -Dir $dir
            $shot = Wait-ForWindowAndShoot -Process $p -Dir $dir
            if (-not $p.WaitForExit($TimeoutSec * 1000)) { $killed = $true; Stop-Staged $p }
            if (-not $killed) { $exit = $p.ExitCode }
        }
        'window-soft' {
            # Raw-Bevy examples never touch Runtime, so neither hook reaches them.
            $p = Start-Staged -Name $Item.name -Dir $dir
            $shot = Wait-ForWindowAndShoot -Process $p -Dir $dir
            if ($p.HasExited) { $exit = $p.ExitCode } else { Stop-Staged $p; $killed = $true }
        }
        'oneshot' {
            $p = Start-Staged -Name $Item.name -Dir $dir
            if (-not $p.WaitForExit($TimeoutSec * 1000)) { $killed = $true; Stop-Staged $p }
            if (-not $killed) { $exit = $p.ExitCode }
        }
        default {
            # service / hardware: alive-and-producing is the pass condition,
            # because these run until something stops them.
            $p = Start-Staged -Name $Item.name -Dir $dir
            if (-not $p.WaitForExit([math]::Min($TimeoutSec, 20) * 1000)) { $killed = $true; Stop-Staged $p }
            if (-not $killed) { $exit = $p.ExitCode }
        }
    }
    $sw.Stop()

    $ev = Read-Evidence -Dir $dir
    $frameProof = $ev.stdout -match 'XRDS_FRAME_LIMIT_REACHED frames=(\d+)'
    $frames = if ($frameProof) { [int]$Matches[1] } else { 0 }

    $status = switch ($Item.mode) {
        'frames' { if ($exit -eq 0 -and -not $ev.panicked) { 'PASS' } else { 'FAIL' } }
        'window-soft' { if ($shot -and $shot.size -and -not $ev.panicked) { 'PASS' } else { 'FAIL' } }
        'oneshot' { if ($exit -eq 0 -and -not $ev.panicked) { 'PASS' } else { 'FAIL' } }
        'service' { if (-not $ev.panicked -and ($killed -or $exit -eq 0)) { 'PASS' } else { 'FAIL' } }
        # Only reachable under -IncludeHardware; kept out of pass/fail either way.
        'hardware' { if (-not $ev.panicked -and ($killed -or $exit -eq 0)) { 'HARDWARE-OK' } else { 'HARDWARE-UNAVAILABLE' } }
        default { 'UNKNOWN' }
    }
    # Deliberately no automatic verdict on a flat-looking capture. Measured both
    # ways, it gets the answer wrong in both directions: load_gltf draws a small
    # dark model and scores as blank, while load_gltf_fail_case is *correctly*
    # blank because it demonstrates a glTF that fails to load. The spread is
    # reported as a number for a human to check, not turned into a status.

    $result = [pscustomobject]@{
        name        = $Item.name
        path        = $Item.path
        category    = $Item.category
        mode        = $Item.mode
        status      = $status
        exit_code   = $exit
        killed      = $killed
        seconds     = [math]::Round($sw.Elapsed.TotalSeconds, 1)
        frame_proof = $frameProof
        frames      = $frames
        screenshot  = if ($shot -and $shot.size) { "examples/$($Item.name)/window.png ($($shot.size))" } else { $null }
        variance    = if ($shot) { $shot.variance } else { $null }
        no_shot_why = if ($shot) { $shot.skipped } else { 'not a windowed mode' }
        panicked    = $ev.panicked
        stdout_head = ($ev.stdout -split "`n" | Select-Object -First 6) -join "`n"
        log         = Get-LogExcerpt -Text ($ev.stdout + "`n" + $ev.stderr)
    }
    $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $dir 'result.json')
    $result
}

# The realnet trio is a three-process handshake across a signaling server, so it
# is driven as one scenario instead of three unrelated launches.
function Invoke-RealnetTrio {
    param($Items)
    $dir = Join-Path $EvidenceDir '_webrtc_realnet'
    New-Item -ItemType Directory -Force $dir | Out-Null
    $port = 9443
    $addr = "ws://127.0.0.1:$port/"
    $procs = @()
    $status = 'FAIL'
    $note = ''
    try {
        $srv = Start-Staged -Name 'webrtc_realnet_signaling_server' -Arguments @('--port', "$port") -Dir (Join-Path $dir 'signaling_server')
        $procs += $srv
        Start-Sleep -Seconds 3

        $pub = Start-Staged -Name 'webrtc_realnet_publisher' -Arguments @('--signaling-addr', $addr, '--stream-seconds', '20') -Dir (Join-Path $dir 'publisher')
        $procs += $pub

        # The publisher prints the session id the subscriber has to join with.
        $session = $null
        $deadline = (Get-Date).AddSeconds(45)
        while ((Get-Date) -lt $deadline -and -not $session) {
            Start-Sleep -Seconds 1
            $txt = Get-Content (Join-Path $dir 'publisher\stdout.txt') -Raw -ErrorAction SilentlyContinue
            if ($txt -and $txt -match '(?im)session[ _-]?id\D{0,12}([0-9a-fA-F-]{8,})') { $session = $Matches[1] }
        }
        if (-not $session) {
            $note = 'publisher never printed a session id'
        } else {
            $sub = Start-Staged -Name 'webrtc_realnet_subscriber' -Arguments @('--signaling-addr', $addr, '--session-id', $session) -Dir (Join-Path $dir 'subscriber')
            $procs += $sub
            $sub.WaitForExit(60000) | Out-Null
            $subOut = Get-Content (Join-Path $dir 'subscriber\stdout.txt') -Raw -ErrorAction SilentlyContinue
            if ($subOut -match '(?i)connected|candidate pair|ICE') {
                $status = 'PASS'
                $note = "session $session; subscriber reported ICE progress"
            } else {
                $note = "session $session; subscriber produced no ICE output"
            }
        }
    } finally {
        foreach ($p in $procs) { Stop-Staged $p }
    }

    foreach ($item in $Items) {
        [pscustomobject]@{
            name = $item.name; path = $item.path; category = $item.category
            mode = 'paired'; status = $status; exit_code = $null; killed = $true
            seconds = 0; frame_proof = $false; frames = 0
            screenshot = $null; variance = $null; no_shot_why = 'not a windowed mode'
            panicked = $false; stdout_head = $note; log = $note
        }
    }
}

# Each example writes its own result.json as it finishes, so an interrupted run
# still has everything it got through. Prefer the rolled-up results.json when it
# exists; otherwise rebuild it from the per-example files rather than throwing
# away 40 minutes of completed work.
function Get-PriorResults {
    param([string]$Dir)
    $rolled = Join-Path $Dir 'results.json'
    if (Test-Path $rolled) { return @(Get-Content $rolled -Raw | ConvertFrom-Json) }
    $perExample = Join-Path $Dir 'examples'
    if (-not (Test-Path $perExample)) { return @() }
    @(Get-ChildItem $perExample -Recurse -Filter result.json |
        ForEach-Object { Get-Content $_.FullName -Raw | ConvertFrom-Json })
}

# ---------------------------------------------------------------------- report

function Write-Report {
    param($Inventory, $Build, $Stage, $Results, [int]$ManifestCount = 0)

    $total = @($Inventory.examples).Count
    # A live build hands back a hashtable; one reloaded from build.json arrives as
    # a PSCustomObject, so read the names the way each one actually stores them.
    $artifactNames = @()
    if ($Build) {
        $artifactNames = if ($Build.artifacts -is [hashtable]) { @($Build.artifacts.Keys) }
        else { @($Build.artifacts.PSObject.Properties.Name) }
    }
    $built = @($Inventory.examples | Where-Object { $artifactNames -contains $_.name }).Count
    $pass = @($Results | Where-Object { $_.status -eq 'PASS' }).Count
    $fail = @($Results | Where-Object { $_.status -eq 'FAIL' }).Count
    $skipped = @($Results | Where-Object { $_.status -eq 'SKIPPED-HARDWARE' })
    $shots = @($Results | Where-Object { $_.screenshot }).Count
    $proofs = @($Results | Where-Object { $_.frame_proof }).Count

    $lines = [System.Collections.Generic.List[string]]::new()
    $add = { param($s) $lines.Add($s) }

    & $add "=== XRDS reference examples: ASSESSMENT REPORT ==="
    & $add "run id      : $RunId"
    & $add "generated   : $((Get-Date).ToString('u'))"
    & $add "machine     : $env:COMPUTERNAME / $([Environment]::OSVersion.VersionString)"
    & $add ""
    & $add "-- 1. COUNT --"
    & $add "reference examples declared : $total"
    foreach ($g in ($Inventory.examples | Group-Object category | Sort-Object Name)) {
        & $add ("  {0,-12} {1,3}" -f $g.Name, $g.Count)
    }
    $undocumented = @($Inventory.examples | Where-Object { -not $_.documented })
    & $add "documented in examples/README.md : $($total - $undocumented.Count) / $total"
    foreach ($u in $undocumented) { & $add "  ? $($u.path) has no entry in examples/README.md" }
    & $add "source files with no [[example]] entry : $(@($Inventory.orphans).Count)"
    foreach ($o in $Inventory.orphans) { & $add "  ! $o" }
    & $add "[[example]] entries with no source file : $(@($Inventory.missing).Count)"
    foreach ($m in $Inventory.missing) { & $add "  ! $m" }
    & $add ""

    if ($Build) {
        & $add "-- 2. BUILD --"
        # Incremental by nature -- whatever target/ already held is reused, so this
        # is not a cold-build figure and should not be read as one.
        & $add "cargo build --examples : exit $($Build.exit_code) in $($Build.seconds)s (incremental)"
        & $add "binaries produced      : $built / $total"
        if (@($Build.errors).Count) { & $add "compiler errors        : $(@($Build.errors).Count)" }
        if ($Stage) { & $add "staged for running     : $($Stage.binaries) binaries + $($Stage.dlls) native DLLs" }
        & $add ""
    } else {
        & $add "-- 2. BUILD --"
        & $add "skipped (-SkipBuild); ran against whatever was already staged in runs\_stage"
        & $add ""
    }

    if ($ManifestCount) {
        & $add "-- BINARY FINGERPRINTS --"
        & $add "binaries.csv : SHA-256 of each of the $ManifestCount executables as run,"
        & $add "with size and build timestamp. Native DLLs are not hashed."
        & $add ""
    }

    if (@($Results).Count) {
        $assessed = @($Results | Where-Object { $_.status -ne 'SKIPPED-HARDWARE' }).Count
        & $add "-- 3. RUN --"
        & $add "assessed            : $assessed"
        & $add "pass                : $pass"
        & $add "fail                : $fail"
        & $add "skipped (hardware)  : $($skipped.Count)"
        & $add "frame-count proof   : $proofs (exited 0 after rendering $Frames frames)"
        & $add "window screenshots  : $shots"
        $noShot = @($Results | Where-Object { -not $_.screenshot -and $_.mode -in @('frames', 'window-soft') })
        if ($noShot.Count) {
            & $add "windowed but unphotographed : $($noShot.Count)"
            foreach ($n in $noShot) { & $add "  ? $($n.name): $($n.no_shot_why)" }
        }
        foreach ($s in $skipped) { & $add "  - $($s.name) skipped: $($s.stdout_head)" }
        & $add ""
        & $add ("{0,-38} {1,-12} {2,-20} {3}" -f 'EXAMPLE', 'MODE', 'STATUS', 'EVIDENCE')
        foreach ($r in ($Results | Sort-Object category, name)) {
            $ev = if ($r.frame_proof) { "$($r.frames) frames, exit 0" }
            elseif ($r.screenshot) { 'screenshot' }
            elseif ($null -ne $r.exit_code) { "exit $($r.exit_code)" }
            else { $r.stdout_head -split "`n" | Select-Object -First 1 }
            & $add ("{0,-38} {1,-12} {2,-20} {3}" -f $r.name, $r.mode, $r.status, $ev)
        }
        & $add ""

        # The windowless examples prove themselves in the terminal, so their
        # output belongs in the report rather than only in the evidence folder.
        $console = @($Results | Where-Object { $_.mode -in @('oneshot', 'service') -and $_.log })
        if ($console.Count) {
            & $add "-- TERMINAL EVIDENCE (examples with no window) --"
            foreach ($r in ($console | Sort-Object name)) {
                & $add "$($r.name) [$($r.mode)] -> $($r.status)"
                & $add "  $($r.log -replace "`n", "`n  ")"
                & $add ""
            }
        }

        # A measurement, not a verdict. Tried both ways, an automatic blank-window
        # judgement gets it wrong in both directions -- a small dark model scores
        # as empty, and an intentionally empty scene scores as content. So the
        # quietest captures are simply pointed at, for a person to open.
        $quiet = @($Results | Where-Object { $null -ne $_.variance } | Sort-Object variance | Select-Object -First 6)
        if ($quiet.Count) {
            & $add "-- LEAST VISUAL VARIATION (open these and check by eye) --"
            & $add "Luminance spread over a sampled grid. Low is not a failure: a small or"
            & $add "dark subject scores low, and some scenes are meant to be near-empty."
            foreach ($r in $quiet) {
                & $add ("  {0,-38} spread {1,-7} {2}" -f $r.name, $r.variance, $r.screenshot)
            }
            & $add ""
        }

        if ($fail) {
            & $add "-- FAILURES --"
            foreach ($r in ($Results | Where-Object { $_.status -eq 'FAIL' })) {
                & $add "$($r.name) [$($r.mode)] exit=$($r.exit_code) killed=$($r.killed) panicked=$($r.panicked)"
                & $add "  $($r.log -replace "`n", "`n  ")"
            }
            & $add ""
        }
    }

    if ($RetryFrom) {
        & $add "NOTE: partial run (-RetryFrom $RetryFrom). Only examples that had not"
        & $add "settled in run '$RetryFrom' were launched; the rest are unchanged there."
        & $add ""
    }
    & $add "evidence : $RunDir"
    $text = $lines -join "`r`n"
    $text | Set-Content (Join-Path $RunDir 'report.txt')
    $text
}

# ------------------------------------------------------------------------ main

New-Item -ItemType Directory -Force $RunDir | Out-Null
New-Item -ItemType Directory -Force $EvidenceDir | Out-Null

Write-Host "== inventory ==" -ForegroundColor Cyan
$inventory = Build-Inventory
$inventory | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $RunDir 'inventory.json')
Write-Host "   $(@($inventory.examples).Count) reference examples, $(@($inventory.orphans).Count) undeclared, $(@($inventory.missing).Count) missing"

$build = $null
$stage = $null
if (-not $SkipBuild) {
    $build = Invoke-BuildAll
    $build | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $RunDir 'build.json')
    if ($build.exit_code -ne 0) { Write-Warning "build failed; run stage will only cover what compiled" }
    $stage = Publish-Stage -Artifacts $build.artifacts -Inventory $inventory
} else {
    # Same idea as the recorded results below: a regenerated report should still
    # carry the build numbers of the run it belongs to.
    $priorBuild = Join-Path $RunDir 'build.json'
    if (Test-Path $priorBuild) {
        $build = Get-Content $priorBuild -Raw | ConvertFrom-Json
        Write-Host "   reusing recorded build from $priorBuild"
    }
}

$manifestCount = 0
if (Test-Path $StageDir) {
    Write-Host "== fingerprinting staged binaries ==" -ForegroundColor Cyan
    $manifestCount = Write-BinaryManifest -Inventory $inventory -Path (Join-Path $RunDir 'binaries.csv')
    Write-Host "   $manifestCount SHA-256 hashes -> binaries.csv"
}

$results = [System.Collections.Generic.List[object]]::new()
if (-not $SkipRun) {
    if (-not (Test-Path $StageDir)) { throw "nothing staged in $StageDir; re-run without -SkipBuild" }
    $todo = @($inventory.examples | Where-Object { $_.name -like $Only -and $_.exists })

    # Regression loop: a full pass costs ~40 minutes, almost all of it re-proving
    # examples that already work. -RetryFrom narrows the run to whatever did not
    # pass last time, so a fix can be checked in a minute instead.
    if ($RetryFrom) {
        $prior = Get-PriorResults -Dir (Join-Path $PSScriptRoot "runs\$RetryFrom")
        if (-not $prior.Count) { throw "no results to retry from in runs\$RetryFrom" }
        $settled = @($prior | Where-Object { $_.status -in @('PASS', 'EXCLUDED', 'SKIPPED-HARDWARE', 'HARDWARE-OK') } |
                ForEach-Object { $_.name })
        $before = $todo.Count
        $todo = @($todo | Where-Object { $settled -notcontains $_.name })
        Write-Host "   -RetryFrom $RetryFrom : $($todo.Count) unsettled of $before ($($settled.Count) already passing, not re-run)"
    }

    Write-Host "== running $($todo.Count) examples ==" -ForegroundColor Cyan

    # Hardware-dependent examples are excluded by default: on a machine with no
    # camera their outcome says nothing about the SDK, and counting them either
    # way would corrupt the pass rate. -IncludeHardware opts back in.
    if (-not $IncludeHardware) {
        foreach ($item in ($todo | Where-Object { $_.mode -eq 'hardware' })) {
            Write-Host ("   {0,-38} [{1}] " -f $item.name, $item.mode) -NoNewline
            Write-Host 'SKIPPED' -ForegroundColor DarkGray
            $results.Add([pscustomobject]@{
                    name = $item.name; path = $item.path; category = $item.category
                    mode = 'hardware'; status = 'SKIPPED-HARDWARE'; exit_code = $null; killed = $false
                    seconds = 0; frame_proof = $false; frames = 0
                    screenshot = $null; variance = $null; no_shot_why = 'not run'
                    panicked = $false
                    stdout_head = 'needs a real camera and microphone; re-run with -IncludeHardware'
                    log = ''
                })
        }
        $todo = @($todo | Where-Object { $_.mode -ne 'hardware' })
    }

    $trio = @($todo | Where-Object { $_.mode -eq 'paired' })
    foreach ($item in ($todo | Where-Object { $_.mode -ne 'paired' })) {
        Write-Host ("   {0,-38} [{1}] " -f $item.name, $item.mode) -NoNewline
        $r = Invoke-Example -Item $item
        $colour = if ($r.status -eq 'PASS') { 'Green' } elseif ($r.status -like 'HARDWARE*') { 'Yellow' } else { 'Red' }
        Write-Host $r.status -ForegroundColor $colour
        $results.Add($r)
    }
    if ($trio.Count) {
        Write-Host "   webrtc realnet trio (signaling + publisher + subscriber)"
        foreach ($r in (Invoke-RealnetTrio -Items $trio)) { $results.Add($r) }
    }
    $results | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $RunDir 'results.json')
} else {
    # Re-running with -SkipRun against an existing run id regenerates the report
    # from that run's recorded results, so the inventory can be refreshed without
    # launching 62 programs again.
    foreach ($r in (Get-PriorResults -Dir $RunDir)) { $results.Add($r) }
    if ($results.Count) { Write-Host "   reusing $($results.Count) recorded results from runs\$RunId" }
}

Write-Host ""
Write-Report -Inventory $inventory -Build $build -Stage $stage -Results $results -ManifestCount $manifestCount
