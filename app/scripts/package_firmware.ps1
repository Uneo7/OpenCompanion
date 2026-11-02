# Layout and manifest: docs\developer\firmware-releases.md. Extra arguments go to `oc firmware package`
# (e.g. --from <release folder>, --out <dir>, --min-app 0.2.0). Nothing is uploaded.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$env:CARGO_TARGET_DIR = 'target\package'
cargo run --quiet -p oc-cli --features firmware-package -- firmware package @args
exit $LASTEXITCODE
