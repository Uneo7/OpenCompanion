OpenCompanion @VERSION@: settings app and firmware for the Bose® Companion® 5 speaker.

**Before installing, read the [README](https://github.com/@REPO@/blob/@TAG@/README.md)** and the
[installation guide](https://github.com/@REPO@/blob/@TAG@/docs/installation.md). The app backs up the original
Bose® firmware automatically before changing anything.

## Downloads

| System | File |
|---|---|
| Windows 10 / 11 (x64) | `OpenCompanion-@VERSION@-windows-x64.zip` (portable: unzip, run `opencompanion.exe`) |
| macOS (Apple Silicon and Intel) | `OpenCompanion-@VERSION@-macos-universal.dmg` (or `.zip`) |
| Linux x86_64 | `OpenCompanion-@VERSION@-linux-x86_64.AppImage` + `70-opencompanion.rules` |
| Linux ARM64 | `OpenCompanion-@VERSION@-linux-aarch64.AppImage` + `70-opencompanion.rules` |
| Command-line tool (recovery) | `oc-@VERSION@-<system>.{zip,tar.gz}` |
| Firmware (downloaded by the app itself) | `opencompanion-firmware-*.zip`, `firmware-manifest.json` |

These builds are **not code-signed**: Windows SmartScreen and macOS Gatekeeper warn on first launch. See
[First run of an unsigned build](https://github.com/@REPO@/blob/@TAG@/README.md#2-first-run-of-an-unsigned-build).

## Checksums

Every file has a `.sha256`; `SHA256SUMS.txt` lists them all (`sha256sum -c SHA256SUMS.txt`).

OpenCompanion is free software (GPL-3.0-or-later), by the OpenCompanion contributors. It is not affiliated with
or endorsed by Bose Corporation.
