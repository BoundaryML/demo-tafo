#!/bin/bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
export BAML_TELEMETRY=off
export BOUNDARY_API_KEY=local
BAML_BIN=${BAML_BIN:-baml}
python3 scripts/check_rust_boundaries.py
"$BAML_BIN" check
"$BAML_BIN" test
mkdir -p .build
"$BAML_BIN" generate
cargo test --locked
