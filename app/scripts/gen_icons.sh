#!/usr/bin/env sh
# Needs only cargo (the generator is a standalone crate, never built with the app).
set -e
cd "$(dirname "$0")/.."
CARGO_TARGET_DIR=target/gen-icons cargo run --release --manifest-path scripts/gen_icons/Cargo.toml
