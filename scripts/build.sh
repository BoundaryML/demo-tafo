#!/bin/bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
export SMART_SWITCH_CLOUD=${SMART_SWITCH_CLOUD:-0}
export BAML_TELEMETRY=off
export BOUNDARY_API_KEY=${BOUNDARY_API_KEY:-local}
BAML_BIN=${BAML_BIN:-baml}
"$BAML_BIN" check
mkdir -p .build
generate_args=(generate)
if [[ ${TAFO_EMBED_TELEMETRY:-0} == 1 ]]; then
    : "${TAFO_TELEMETRY_KEY:?Set an ingestion-only Boundary key}"
    generate_args+=(--embed-telemetry)
fi
"$BAML_BIN" "${generate_args[@]}"
cargo_args=(build --release --locked)
if [[ -n ${SMART_SWITCH_FEATURES:-} ]]; then
    cargo_args+=(--features "$SMART_SWITCH_FEATURES")
fi
cargo "${cargo_args[@]}"
app=${SMART_SWITCH_APP:-dist/Tafo.app}
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources/mascot"
rm -f "$app/Contents/Resources/mascot/ferret-"{idle,switch,correct}".png"
cp assets/mascot/ferret-{idle,switch,correct}-sprites.png "$app/Contents/Resources/mascot/"
cp target/release/tafo "$app/Contents/MacOS/Tafo.next"
mv -f "$app/Contents/MacOS/Tafo.next" "$app/Contents/MacOS/Tafo"
rm -f "$app/Contents/MacOS/SmartSwitch"
library=$(target/release/tafo --runtime-library-path)
cp "$library" "$app/Contents/Frameworks/libbridge_cffi.dylib"
/usr/bin/codesign --force --sign - "$app/Contents/Frameworks/libbridge_cffi.dylib"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.boundaryml.tafo</string>
<key>CFBundleName</key><string>Tafo</string>
<key>CFBundleDisplayName</key><string>Tafo</string>
<key>CFBundleExecutable</key><string>Tafo</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.2.0</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>LSUIElement</key><true/>
</dict></plist>
PLIST
/usr/bin/codesign --force --deep --sign - "$app"
printf 'Built %s\n' "$app"
