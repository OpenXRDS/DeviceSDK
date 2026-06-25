# android/quest/fetch_loader.ps1
# Downloads the Khronos OpenXR loader for Android from Maven Central and places it at:
#   android/quest/libs/arm64-v8a/libopenxr_loader.so
#
# Run once; the build scripts pick it up automatically from that path.
# Re-run to update to a newer loader version.

$ErrorActionPreference = "Stop"

$Dest = "$PSScriptRoot\libs\arm64-v8a\libopenxr_loader.so"

Write-Host "==> Fetching latest Khronos OpenXR loader for Android from Maven Central..."

# Resolve latest version from Maven Central metadata
$MetaUrl = "https://repo1.maven.org/maven2/org/khronos/openxr/openxr_loader_for_android/maven-metadata.xml"
$xml     = Invoke-RestMethod $MetaUrl
$Version = $xml.metadata.versioning.latest

Write-Host "    Version : $Version"

$AarUrl = "https://repo1.maven.org/maven2/org/khronos/openxr/openxr_loader_for_android/$Version/openxr_loader_for_android-$Version.aar"
$Tmp    = [System.IO.Path]::GetTempPath() + "openxr_loader_android.aar"

Write-Host "    Download: $AarUrl"
Invoke-WebRequest -Uri $AarUrl -OutFile $Tmp -UseBasicParsing

# An .aar is a zip; extract jni/arm64-v8a/libopenxr_loader.so
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip   = [System.IO.Compression.ZipFile]::OpenRead($Tmp)
$entry = $zip.Entries | Where-Object { $_.FullName -eq "jni/arm64-v8a/libopenxr_loader.so" } | Select-Object -First 1

if (-not $entry) {
    $zip.Dispose()
    Write-Error "jni/arm64-v8a/libopenxr_loader.so not found inside AAR"
}

New-Item -ItemType Directory -Force -Path (Split-Path $Dest) | Out-Null
$stream = $entry.Open()
$file   = [System.IO.File]::Create($Dest)
$stream.CopyTo($file)
$file.Dispose(); $stream.Dispose(); $zip.Dispose()
Remove-Item $Tmp

Write-Host ""
Write-Host "==> Saved: $Dest"
Write-Host "    Build scripts will use this automatically — no OPENXR_LOADER needed."
