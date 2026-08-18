#!/usr/bin/env bash
# android/quest/fetch_loader.sh
# Downloads the Khronos OpenXR loader for Android from Maven Central and places it at:
#   android/quest/libs/arm64-v8a/libopenxr_loader.so
#
# Run once; the build scripts pick it up automatically from that path.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST="$SCRIPT_DIR/libs/arm64-v8a/libopenxr_loader.so"
TMP="$(mktemp).aar"

echo "==> Fetching latest Khronos OpenXR loader for Android from Maven Central..."

check_cmd() { command -v "$1" &>/dev/null || { echo "ERROR: '$1' not found. $2"; exit 1; }; }
check_cmd curl    "Install curl"
check_cmd python3 "Install python3 (used to unzip .aar)"

# Resolve latest version from Maven Central metadata
META_URL="https://repo1.maven.org/maven2/org/khronos/openxr/openxr_loader_for_android/maven-metadata.xml"
VERSION=$(curl -fsSL "$META_URL" | python3 -c "
import sys
from xml.etree import ElementTree as ET
xml = ET.fromstring(sys.stdin.read())
print(xml.find('versioning/latest').text)
")
AAR_URL="https://repo1.maven.org/maven2/org/khronos/openxr/openxr_loader_for_android/$VERSION/openxr_loader_for_android-$VERSION.aar"

echo "    Version : $VERSION"
echo "    Download: $AAR_URL"

curl -fsSL -o "$TMP" "$AAR_URL"

# .aar is a zip; extract the arm64-v8a .so
mkdir -p "$(dirname "$DEST")"
python3 -c "
import zipfile
with zipfile.ZipFile('$TMP') as z:
    with z.open('jni/arm64-v8a/libopenxr_loader.so') as src, open('$DEST', 'wb') as dst:
        dst.write(src.read())
"

rm -f "$TMP"

echo ""
echo "==> Saved: $DEST"
echo "    Build scripts will use this automatically — no OPENXR_LOADER needed."
