<#
.SYNOPSIS
    Builds (if needed) and launches the XRDS editor on Windows.

.DESCRIPTION
    The editor loads its UI from apps/xrds-editor/dist/ when that bundle exists,
    and otherwise from the Vite dev server at http://localhost:5173. This script
    keeps dist/ in step with the React sources so a plain run never shows a stale
    UI: it installs npm dependencies when node_modules/ is missing, rebuilds the
    frontend when any file under src/ (or index.html) is newer than
    dist/index.html, then runs `cargo run -p xrds-editor`.

.PARAMETER Release
    Build and run the editor with --release.

.PARAMETER Rebuild
    Force a frontend rebuild even if dist/ looks current.

.PARAMETER Dev
    Hot-reload mode: removes dist/ so the editor loads from the Vite dev server,
    starts `npm run dev` in a separate window, then runs the editor.

.EXAMPLE
    .\run-editor.ps1
    .\run-editor.ps1 -Release
    .\run-editor.ps1 -Dev
    .\run-editor.ps1 -- --some-editor-arg
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$Rebuild,
    [switch]$Dev,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$EditorArgs
)

$ErrorActionPreference = 'Stop'

$Root     = $PSScriptRoot
$Frontend = Join-Path $Root 'apps\xrds-editor'
$Dist     = Join-Path $Frontend 'dist'
$DistHtml = Join-Path $Dist 'index.html'

function Require-Command($name, $hint) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "'$name' not found on PATH. $hint"
    }
}

function Invoke-Checked([string]$what, [scriptblock]$block) {
    & $block
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)." }
}

Require-Command cargo 'Install Rust from https://rustup.rs'
Require-Command npm   'Install Node.js 20+ from https://nodejs.org'

Push-Location $Frontend
try {
    if (-not (Test-Path (Join-Path $Frontend 'node_modules'))) {
        Write-Host '[run-editor] Installing frontend dependencies...' -ForegroundColor Cyan
        Invoke-Checked 'npm install' { npm install }
    }

    if ($Dev) {
        # The editor picks the dev server only when dist/index.html is absent.
        if (Test-Path $Dist) {
            Write-Host '[run-editor] Removing dist/ so the editor uses the Vite dev server.' -ForegroundColor Cyan
            Remove-Item -Recurse -Force $Dist
        }
        Write-Host '[run-editor] Starting Vite dev server in a new window...' -ForegroundColor Cyan
        Start-Process -FilePath 'cmd.exe' -ArgumentList '/k', 'npm run dev' -WorkingDirectory $Frontend
    }
    else {
        $needsBuild = $Rebuild -or -not (Test-Path $DistHtml)
        if (-not $needsBuild) {
            $builtAt = (Get-Item $DistHtml).LastWriteTimeUtc
            $sources = @(Get-ChildItem (Join-Path $Frontend 'src') -Recurse -File) +
                       @(Get-Item (Join-Path $Frontend 'index.html'))
            $needsBuild = [bool]($sources | Where-Object { $_.LastWriteTimeUtc -gt $builtAt } | Select-Object -First 1)
        }
        if ($needsBuild) {
            Write-Host '[run-editor] Building frontend (npm run build)...' -ForegroundColor Cyan
            Invoke-Checked 'npm run build' { npm run build }
        }
        else {
            Write-Host '[run-editor] Frontend is up to date.' -ForegroundColor DarkGray
        }
    }
}
finally {
    Pop-Location
}

$cargoArgs = @('run', '-p', 'xrds-editor')
if ($Release) { $cargoArgs += '--release' }
if ($EditorArgs) { $cargoArgs += '--'; $cargoArgs += $EditorArgs }

Write-Host "[run-editor] cargo $($cargoArgs -join ' ')" -ForegroundColor Cyan
Push-Location $Root
try {
    Invoke-Checked 'cargo run' { cargo @cargoArgs }
}
finally {
    Pop-Location
}
