# android/quest/build.ps1 — Full Quest APK build and packaging script (Windows)
#
# Usage:
#   .\android\quest\build.ps1 [-SceneDir C:\path\to\exported\scene]
#
#   -SceneDir  Optional. Copy scene.json and assets\ from this directory into
#              the APK so no adb push is needed at runtime. Without it, push
#              the scene separately — see "Install and run" in README.md.
#
# Required environment variables:
#   OPENXR_LOADER   Path to libopenxr_loader.so from the Meta OpenXR Mobile SDK.
#                   Example: C:\ovr_openxr_mobile_sdk\OpenXR\Libs\Android\arm64-v8a\Release\libopenxr_loader.so
#
# Optional environment variables:
#   ANDROID_HOME    Android SDK root (default: $env:LOCALAPPDATA\Android\Sdk)
#   BUILD_TOOLS_VER Android build-tools version (default: 35.0.0)
#   KEYSTORE        Keystore for signing (default: $HOME\.android\debug.keystore)
#   KEYSTORE_PASS   Keystore password (default: android)
#
# Output: android\quest\build\xrds-app.apk

param(
    [string]$SceneDir = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir     = $PSScriptRoot
$WorkspaceRoot = (Resolve-Path "$ScriptDir\..\..")
$PackageName   = "org.openxrds.devicesdk"

# Defaults
$AndroidHome    = if ($env:ANDROID_HOME)    { $env:ANDROID_HOME }    else { "$env:LOCALAPPDATA\Android\Sdk" }
$BuildToolsVer  = if ($env:BUILD_TOOLS_VER) { $env:BUILD_TOOLS_VER } else { "35.0.0" }
$Keystore       = if ($env:KEYSTORE)        { $env:KEYSTORE }        else { "$HOME\.android\debug.keystore" }
$KeystorePass   = if ($env:KEYSTORE_PASS)   { $env:KEYSTORE_PASS }   else { "android" }
$BundledLoader  = "$ScriptDir\libs\arm64-v8a\libopenxr_loader.so"
$OpenXrLoader   = if ($env:OPENXR_LOADER)   { $env:OPENXR_LOADER }   elseif (Test-Path $BundledLoader) { $BundledLoader } else { "" }

$BuildTools = "$AndroidHome\build-tools\$BuildToolsVer"
$Platform   = "$AndroidHome\platforms\android-35"
$JniDir     = "$ScriptDir\jni\arm64-v8a"
$BuildDir   = "$ScriptDir\build"

# Locate NDK — try env vars first, then enumerate $AndroidHome\ndk\
$NdkHome = if ($env:ANDROID_NDK_HOME) { $env:ANDROID_NDK_HOME } `
           elseif ($env:NDK_HOME)      { $env:NDK_HOME } `
           elseif ($env:ANDROID_NDK_ROOT) { $env:ANDROID_NDK_ROOT } `
           else {
               $ndkDir = "$AndroidHome\ndk"
               if (Test-Path $ndkDir) {
                   $ver = Get-ChildItem $ndkDir -Directory | Sort-Object Name -Descending | Select-Object -First 1
                   if ($ver) { $ver.FullName } else { "" }
               } else { "" }
           }

# ---------------------------------------------------------------------------
# Validate prerequisites
# ---------------------------------------------------------------------------
if (-not $OpenXrLoader) {
    Write-Error @"
OpenXR loader not found. Run the fetch script first:

  .\android\quest\fetch_loader.ps1

Or set OPENXR_LOADER to an existing libopenxr_loader.so:

  `$env:OPENXR_LOADER = 'C:\ovr_openxr_mobile_sdk\OpenXR\Libs\Android\arm64-v8a\Release\libopenxr_loader.so'
  .\android\quest\build.ps1
"@
}

if (-not (Test-Path $OpenXrLoader)) {
    Write-Error "OpenXR loader not found: $OpenXrLoader"
}

if ($SceneDir -and -not (Test-Path $SceneDir -PathType Container)) {
    Write-Error "-SceneDir not found: $SceneDir"
}

foreach ($exe in @("aapt.exe", "zipalign.exe")) {
    if (-not (Test-Path "$BuildTools\$exe")) {
        Write-Error "$exe not found at $BuildTools\$exe`nInstall Android build-tools $BuildToolsVer via Android Studio SDK Manager."
    }
}

if (-not $NdkHome) {
    Write-Error "Android NDK not found. Install it via Android Studio SDK Manager, or set ANDROID_NDK_HOME."
}
$LibCppShared = "$NdkHome\toolchains\llvm\prebuilt\windows-x86_64\sysroot\usr\lib\aarch64-linux-android\libc++_shared.so"
if (-not (Test-Path $LibCppShared)) {
    Write-Error "libc++_shared.so not found at:`n  $LibCppShared`nCheck that NDK is installed and ANDROID_NDK_HOME is correct."
}

if (-not (Get-Command "cargo-ndk" -ErrorAction SilentlyContinue)) {
    Write-Error "cargo-ndk not found. Install with: cargo install cargo-ndk"
}

Write-Host "==> Build configuration"
Write-Host "    Android SDK    : $AndroidHome"
Write-Host "    Build tools    : $BuildToolsVer"
Write-Host "    OpenXR loader  : $OpenXrLoader"
$sceneLabel = if ($SceneDir) { $SceneDir } `
              elseif (Test-Path "$WorkspaceRoot\res\default.json") { "res/default.json (bundled default)" } `
              else { "<not set - push scene separately>" }
Write-Host "    Scene          : $sceneLabel"
Write-Host ""

# ---------------------------------------------------------------------------
# Step 1: Build libxrds_app.so
# ---------------------------------------------------------------------------
Write-Host "==> Step 1: Building libxrds_app.so..."
New-Item -ItemType Directory -Force -Path $JniDir | Out-Null
Push-Location $WorkspaceRoot
# Tell the linker (via cargo-ndk's proxy) where libopenxr_loader.so lives.
# Must be absolute — cargo-ndk's linker proxy has an unpredictable working directory.
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS = "-L $ScriptDir\libs\arm64-v8a -C link-arg=-Wl,--allow-shlib-undefined"
try {
    cargo ndk -t arm64-v8a -o "$ScriptDir\jni" build --release -p xrds-app
    if ($LASTEXITCODE -ne 0) { throw "cargo ndk failed" }
} finally {
    Pop-Location
    Remove-Item Env:CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS -ErrorAction SilentlyContinue
}
Write-Host "    Built: $JniDir\libxrds_app.so"

# ---------------------------------------------------------------------------
# Step 2: Copy OpenXR loader
# ---------------------------------------------------------------------------
Write-Host "==> Step 2: Copying OpenXR loader..."
Copy-Item $OpenXrLoader "$JniDir\libopenxr_loader.so" -Force
Write-Host "    Copied: $JniDir\libopenxr_loader.so"

# ---------------------------------------------------------------------------
# Step 3: Stage assets
# ---------------------------------------------------------------------------
Write-Host "==> Step 3: Staging assets..."
$Staging = "$BuildDir\assets"
if (Test-Path $Staging) { Remove-Item $Staging -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Staging | Out-Null

foreach ($sub in @("fonts", "shaders", "environment_maps", "sound", "textures")) {
    $src = "$WorkspaceRoot\assets\$sub"
    if (Test-Path $src) {
        Copy-Item $src "$Staging\$sub" -Recurse -Force
    }
}

$DefaultSceneJson = "$WorkspaceRoot\res\default.json"
if ($SceneDir) {
    Write-Host "    Bundling scene from $SceneDir..."
    Copy-Item "$SceneDir\scene.json" "$Staging\scene.json" -Force
    $userAssets = "$SceneDir\assets"
    if (Test-Path $userAssets -PathType Container) {
        Copy-Item "$userAssets\*" $Staging -Recurse -Force
    }
} elseif (Test-Path $DefaultSceneJson) {
    Write-Host "    Bundling default scene from res/default.json..."
    Copy-Item $DefaultSceneJson "$Staging\scene.json" -Force
}

# ---------------------------------------------------------------------------
# Step 4: Package APK
# ---------------------------------------------------------------------------
# NativeActivity is built into Android — no Java DEX bundling required.
Write-Host "==> Step 4: Packaging APK..."
$ResourcesApk = "$BuildDir\resources.apk"
$AlignedApk   = "$BuildDir\aligned.apk"
$OutputApk    = "$BuildDir\xrds-app.apk"

foreach ($f in @($ResourcesApk, $AlignedApk, $OutputApk)) {
    if (Test-Path $f) { Remove-Item $f -Force }
}
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null

# Compile Android resources (strings.xml etc.)
& "$BuildTools\aapt.exe" package -f -m `
    -S "$ScriptDir\res" `
    -M "$ScriptDir\AndroidManifest.xml" `
    -I "$Platform\android.jar" `
    -F $ResourcesApk
if ($LASTEXITCODE -ne 0) { throw "aapt package failed" }

# Add native libraries.
# Android Package Manager only extracts libs from lib/arm64-v8a/ in the APK.
# Stage them there, then add from $BuildDir so the in-APK path is lib/arm64-v8a/*.so
$LibStaging = "$BuildDir\lib\arm64-v8a"
New-Item -ItemType Directory -Force -Path $LibStaging | Out-Null
Copy-Item "$JniDir\libxrds_app.so"        "$LibStaging\" -Force
Copy-Item "$JniDir\libopenxr_loader.so"   "$LibStaging\" -Force
Copy-Item $LibCppShared                   "$LibStaging\" -Force

Push-Location $BuildDir
try {
    foreach ($lib in @("lib/arm64-v8a/libxrds_app.so", "lib/arm64-v8a/libopenxr_loader.so", "lib/arm64-v8a/libc++_shared.so")) {
        & "$BuildTools\aapt.exe" add $ResourcesApk $lib
        if ($LASTEXITCODE -ne 0) { throw "aapt add $lib failed" }
    }
} finally { Pop-Location }

# Add assets.
# Must run from $BuildDir so in-APK paths are assets/fonts/... etc.
Push-Location $BuildDir
try {
    $assetFiles = Get-ChildItem -Recurse -File "assets" |
        ForEach-Object { $_.FullName.Substring($BuildDir.Length + 1).Replace('\', '/') } |
        Sort-Object

    foreach ($f in $assetFiles) {
        & "$BuildTools\aapt.exe" add $ResourcesApk $f
        if ($LASTEXITCODE -ne 0) { throw "aapt add $f failed" }
    }
} finally { Pop-Location }

# Zipalign + sign
& "$BuildTools\zipalign.exe" -f 4 $ResourcesApk $AlignedApk
if ($LASTEXITCODE -ne 0) { throw "zipalign failed" }

# Sign (debug keystore by default; swap KEYSTORE/KEYSTORE_PASS for release)
& "$BuildTools\apksigner.bat" sign `
    --ks $Keystore `
    --ks-pass "pass:$KeystorePass" `
    --out $OutputApk `
    $AlignedApk
if ($LASTEXITCODE -ne 0) { throw "apksigner failed" }

Write-Host ""
Write-Host "==> Done: $OutputApk"
Write-Host ""
Write-Host "Install:"
Write-Host "  adb install -r $OutputApk"
Write-Host "  adb shell am start -n $PackageName/android.app.NativeActivity"

if (-not $SceneDir -and -not (Test-Path "$WorkspaceRoot\res\default.json")) {
    Write-Host ""
    Write-Host "No scene was bundled. Push one before launching:"
    Write-Host "  adb push scene.json /sdcard/Android/data/$PackageName/files/"
    Write-Host "  adb push assets/    /sdcard/Android/data/$PackageName/files/assets/"
}
