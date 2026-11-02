#!/usr/bin/env sh
# Wraps the firmware build outputs of the development tree into the two .ocfw containers in firmware/release/,
# the committed folder the release workflow packages (the DSP needs TI's compiler, so CI cannot build the
# firmware). The identity checks of the installer run first (`oc firmware pack`). Run it after building and
# testing both firmwares, check with `scripts/package_firmware.sh --from ../firmware/release`, then commit.
# Format: docs/developer/firmware-releases.md.
set -e
cd "$(dirname "$0")/.."
CARGO_TARGET_DIR=target/package cargo run --quiet -p oc-cli -- firmware pack --out ../firmware/release "$@"
# The raw build outputs are no longer committed: the containers carry them.
cd ../firmware/release
rm -rf boot opencompanion_dsp.out loader.out oc_tas.dfu oc_tas_data.bin
echo "firmware/release updated"
