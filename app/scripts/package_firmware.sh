#!/usr/bin/env sh
# Layout and manifest: docs/developer/firmware-releases.md. Extra arguments go to `oc firmware package`
# (e.g. --from <release folder>, --out <dir>, --min-app 0.2.0). Nothing is uploaded.
set -e
cd "$(dirname "$0")/.."
CARGO_TARGET_DIR=target/package cargo run --quiet -p oc-cli --features firmware-package -- firmware package "$@"
