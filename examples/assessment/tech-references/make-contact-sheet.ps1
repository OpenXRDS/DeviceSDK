<#
.SYNOPSIS
  Compose every captured example screenshot into one labelled contact sheet.

.DESCRIPTION
  The assessment writes one window.png per example, scattered across
  runs/<run-id>/examples/<name>/. Opening forty-eight of those one at a time is
  nobody's idea of a review, so this collects them into a single grid image with
  each example's name and status underneath. One picture, the whole visual
  result.

  By default it reads *every* run folder and keeps the newest capture of each
  example, which is what you want when the evidence is spread across a full run
  plus later -RetryFrom passes. Restrict it with -RunId when you want exactly
  one run's output.

  The sheet covers only the examples that open a window -- it is a visual index,
  not the assessment result. report.txt remains the authoritative record, and
  the header on the sheet says so by printing what fraction is shown.

.PARAMETER RunId
  One or more run folder names. Default: all of them, newest capture wins.

.PARAMETER Columns
  Grid columns. Default: chosen to land near a 16:9 sheet.

.PARAMETER CellWidth
  Width in pixels of each cell's image area. Default 420.

.PARAMETER Out
  Output path. Default runs/contact-sheet.png.

.EXAMPLE
  pwsh -File make-contact-sheet.ps1
  pwsh -File make-contact-sheet.ps1 -RunId final, retry1 -Columns 6
#>
[CmdletBinding()]
param(
    [string[]]$RunId = @(),
    [int]$Columns = 0,
    [int]$CellWidth = 420,
    [string]$Out = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing

$RunsDir = Join-Path $PSScriptRoot 'runs'
if (-not (Test-Path $RunsDir)) { throw "no runs/ directory yet -- run run-references.ps1 first" }
if (-not $Out) { $Out = Join-Path $RunsDir 'contact-sheet.png' }

# ------------------------------------------------------------------- gather

$runDirs = if ($RunId.Count) {
    $RunId | ForEach-Object {
        $d = Join-Path $RunsDir $_
        if (-not (Test-Path $d)) { throw "no such run: $_" }
        Get-Item $d
    }
} else {
    Get-ChildItem $RunsDir -Directory | Where-Object { $_.Name -ne '_stage' } | Sort-Object LastWriteTime
}

# Newest capture of each example wins, so a later -RetryFrom pass supersedes the
# picture the original full run took.
$shots = [ordered]@{}
foreach ($run in ($runDirs | Sort-Object LastWriteTime)) {
    $examples = Join-Path $run.FullName 'examples'
    if (-not (Test-Path $examples)) { continue }
    foreach ($png in (Get-ChildItem $examples -Recurse -Filter window.png)) {
        $name = $png.Directory.Name
        $status = 'UNKNOWN'
        $resultFile = Join-Path $png.DirectoryName 'result.json'
        if (Test-Path $resultFile) {
            try { $status = (Get-Content $resultFile -Raw | ConvertFrom-Json).status } catch { }
        }
        $shots[$name] = [pscustomobject]@{
            name = $name; path = $png.FullName; status = $status; run = $run.Name
        }
    }
}

$items = @($shots.Values | Sort-Object name)
if (-not $items.Count) { throw "no window.png files found under $RunsDir" }

# How many examples exist at all, so the header can be honest about coverage.
$declared = 0
foreach ($run in ($runDirs | Sort-Object LastWriteTime -Descending)) {
    $inv = Join-Path $run.FullName 'inventory.json'
    if (Test-Path $inv) {
        try { $declared = @((Get-Content $inv -Raw | ConvertFrom-Json).examples).Count; break } catch { }
    }
}

Write-Host "collected $($items.Count) screenshots from $(@($runDirs).Count) run folder(s)"

# ------------------------------------------------------------------- layout

if ($Columns -le 0) {
    # Aim for a sheet about 16:9 overall. Each cell is roughly 1.7:1 plus caption,
    # so cols ~= sqrt(n * 16/9 / cellAspect).
    $Columns = [math]::Max(1, [int][math]::Round([math]::Sqrt($items.Count * (16.0 / 9.0) / 1.9)))
}
$rows = [int][math]::Ceiling($items.Count / [double]$Columns)

$pad = 10
$captionH = 34
$imgH = [int]($CellWidth / 1.708)     # the examples' window aspect
$cellW = $CellWidth + $pad * 2
$cellH = $imgH + $captionH + $pad * 2
$headerH = 96

$sheetW = $Columns * $cellW
$sheetH = $headerH + $rows * $cellH

Write-Host "sheet: $Columns x $rows -> ${sheetW}x${sheetH}px"

$bg = [System.Drawing.Color]::FromArgb(24, 24, 27)
$fgMuted = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(160, 160, 170))
$fgBright = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(238, 238, 242))
$cellBg = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(32, 32, 36))

$statusColour = {
    param($s)
    switch ($s) {
        'PASS' { [System.Drawing.Color]::FromArgb(64, 180, 110) }
        'FAIL' { [System.Drawing.Color]::FromArgb(220, 80, 80) }
        'EXCLUDED' { [System.Drawing.Color]::FromArgb(120, 120, 130) }
        'SKIPPED-HARDWARE' { [System.Drawing.Color]::FromArgb(120, 120, 130) }
        default { [System.Drawing.Color]::FromArgb(200, 170, 70) }
    }
}

$sheet = New-Object System.Drawing.Bitmap $sheetW, $sheetH
$g = [System.Drawing.Graphics]::FromImage($sheet)
$g.Clear($bg)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit

$titleFont = New-Object System.Drawing.Font 'Segoe UI Semibold', 26
$subFont = New-Object System.Drawing.Font 'Segoe UI', 12
$nameFont = New-Object System.Drawing.Font 'Segoe UI Semibold', 11
$statusFont = New-Object System.Drawing.Font 'Segoe UI', 9

$g.DrawString('XRDS reference examples - visual results', $titleFont, $fgBright, $pad + 6, 16)
$coverage = if ($declared) { "$($items.Count) of $declared examples shown (only those that open a window)" }
            else { "$($items.Count) screenshots" }
$g.DrawString(
    "$coverage  |  captured $(Get-Date -Format 'yyyy-MM-dd')  |  full result in report.txt",
    $subFont, $fgMuted, $pad + 8, 58)

$fmt = New-Object System.Drawing.StringFormat
$fmt.Alignment = [System.Drawing.StringAlignment]::Near
$fmt.Trimming = [System.Drawing.StringTrimming]::EllipsisCharacter
$fmt.FormatFlags = [System.Drawing.StringFormatFlags]::NoWrap

for ($i = 0; $i -lt $items.Count; $i++) {
    $it = $items[$i]
    $col = $i % $Columns
    $row = [int][math]::Floor($i / $Columns)
    $x = $col * $cellW + $pad
    $y = $headerH + $row * $cellH + $pad

    $g.FillRectangle($cellBg, $x - $pad + 4, $y - $pad + 4, $cellW - 8, $cellH - 8)

    $img = $null
    try {
        $img = New-Object System.Drawing.Bitmap $it.path
        # Fit inside the cell without distorting; centre what is left over.
        $scale = [math]::Min($CellWidth / $img.Width, $imgH / $img.Height)
        $w = [int]($img.Width * $scale)
        $h = [int]($img.Height * $scale)
        $g.DrawImage($img, [int]($x + ($CellWidth - $w) / 2), [int]($y + ($imgH - $h) / 2), $w, $h)
    } catch {
        $g.DrawString('capture unreadable', $subFont, $fgMuted, $x + 8, $y + 8)
    } finally {
        if ($img) { $img.Dispose() }
    }

    $colour = & $statusColour $it.status
    $chip = New-Object System.Drawing.SolidBrush $colour
    $g.FillRectangle($chip, $x, $y + $imgH + 9, 4, 22)
    $g.DrawString($it.name, $nameFont, $fgBright,
        (New-Object System.Drawing.RectangleF ($x + 12), ($y + $imgH + 7), ($CellWidth - 14), 18), $fmt)
    $g.DrawString($it.status.ToLower(), $statusFont, $chip,
        (New-Object System.Drawing.RectangleF ($x + 12), ($y + $imgH + 23), ($CellWidth - 14), 16), $fmt)
    $chip.Dispose()
}

$g.Dispose()
$sheet.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$sheet.Dispose()

Write-Host "wrote $Out ($([math]::Round((Get-Item $Out).Length / 1MB, 1)) MB)"
