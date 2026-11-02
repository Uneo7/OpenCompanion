# Needs only cargo (the generator is a standalone crate, never built with the app).
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$env:CARGO_TARGET_DIR = 'target\gen-icons'
cargo run --release --manifest-path scripts/gen_icons/Cargo.toml
exit $LASTEXITCODE
