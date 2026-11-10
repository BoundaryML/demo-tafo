#!/bin/bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
export SMART_SWITCH_APP='dist/Tafo.app'
./scripts/build.sh
/usr/bin/codesign --verify --deep --strict "$SMART_SWITCH_APP"
arch=$(/usr/bin/uname -m)
name="Tafo-macos-${arch}"
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$SMART_SWITCH_APP" "dist/${name}.zip"
(cd dist && /usr/bin/shasum -a 256 "${name}.zip" > "${name}.zip.sha256")
printf 'Packaged dist/%s.zip\n' "$name"
