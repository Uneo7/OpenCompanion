#!/usr/bin/env bash
# Unsigned release packages of the app and of the `oc` command-line tool, into dist/ (Windows:
# scripts\package_app.ps1). Nothing is uploaded and nothing is signed with an identity.
#
#   macos          OpenCompanion-<ver>-macos-universal.{zip,dmg}  OpenCompanion.app, arm64 + x86_64, ad-hoc signed
#                  oc-<ver>-macos-universal.tar.gz
#   linux-x86_64   OpenCompanion-<ver>-linux-x86_64.AppImage       zig-linked against glibc 2.31
#   linux-aarch64  OpenCompanion-<ver>-linux-aarch64.AppImage
#                  oc-<ver>-linux-<arch>.tar.gz, 70-opencompanion.rules (udev)
# Each artefact gets a `<file>.sha256` (`<hex>  <file>`).
#
# Usage: scripts/package_app.sh [--out DIR] [macos|linux-x86_64|linux-aarch64 ...]
#   Default: the host's packages (macOS: macos; Linux: both Linux architectures). The Linux binaries can be
#   cross-built from any host (zig + cargo-zigbuild, like check_all_targets.sh); the AppImage step itself needs a
#   Linux host (elsewhere the AppDir is left in the stage folder). macOS packages need a Mac (SDK, lipo, codesign,
#   hdiutil).
# Env: CARGO_TARGET_DIR (default target/package-app: never target/release, the developer's own build),
#      APPIMAGETOOL (path to appimagetool; downloaded into the target dir otherwise).
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$PWD
OUT=dist
TARGETS=()
while [ $# -gt 0 ]; do
    case $1 in
        --out) OUT=$2; shift 2 ;;
        -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
        *) TARGETS+=("$1"); shift ;;
    esac
done

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target/package-app}
SYSROOTS=$CARGO_TARGET_DIR/cross-sysroot
STAGE=$CARGO_TARGET_DIR/stage
. scripts/lib/linux_cross.sh
. scripts/lib/package_common.sh
. scripts/lib/package_macos.sh
. scripts/lib/package_linux.sh

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
mkdir -p "$OUT" "$STAGE"
OUT=$(cd "$OUT" && pwd)

if [ ${#TARGETS[@]} -eq 0 ]; then
    case $HOST_OS in
        macos) TARGETS=(macos) ;;
        linux) TARGETS=(linux-x86_64 linux-aarch64) ;;
        windows) echo "Windows packages: scripts\\package_app.ps1 (here: give Linux targets to cross-build)" >&2; exit 2 ;;
    esac
fi

echo "OpenCompanion $VERSION -> $OUT"
for t in "${TARGETS[@]}"; do
    echo "=== $t"
    case $t in
        macos) package_macos ;;
        linux-x86_64) package_linux x86_64 ;;
        linux-aarch64) package_linux aarch64 ;;
        *) echo "unknown package: $t (macos, linux-x86_64, linux-aarch64)" >&2; exit 2 ;;
    esac
done
