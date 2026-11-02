#!/usr/bin/env sh
# Builds OpenCompanion.app from an opencompanion binary (any host; signing is done by scripts/package_app.sh on a
# Mac). Usage: packaging/macos/bundle.sh [<binary> <out dir>]
#   default: target/release/opencompanion -> target/release/OpenCompanion.app (after `cargo build --release`)
set -e
cd "$(dirname "$0")/../.."
bin="${1:-target/release/opencompanion}"
out="${2:-target/release}"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
app="$out/OpenCompanion.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/opencompanion"
chmod +x "$app/Contents/MacOS/opencompanion"
cp packaging/macos/opencompanion.icns "$app/Contents/Resources/"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
echo "$app"
