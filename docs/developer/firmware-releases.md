# Firmware releases (GitHub)

Production builds of the app take the firmware from the **latest release** of
[`Uneo7/OpenCompagnon`](https://github.com/Uneo7/OpenCompagnon) (note the spelling). Development builds read the
local development tree, as before. The two never mix: there is no runtime detection.

## Build commands

| Build | Command | Firmware source |
|---|---|---|
| Development (default) | `cargo build -p oc-app` / `cargo run -p oc-app` | the dev tree (`firmware/*/build`), or `--firmware <dir>` |
| Production (packaging, CI) | `cargo build --release -p oc-app --features release-updates` | GitHub latest release only, cached in `<data>/firmware/<tag>/` |
| Firmware containers | `oc firmware pack [--from DIR] [--out DIR]`, or `scripts/stage_firmware.*` (into `firmware/release/`) | the two `.ocfw` from the dev tree (or a folder) |
| Release assets | `scripts/package_firmware.ps1` or `scripts/package_firmware.sh` | builds `dist/` from the dev tree, or from the committed `firmware/release/` with `--from ../firmware/release` (what the release workflow does: [`releasing.md`](releasing.md)) |

`release-updates` turns on `oc-device`'s `github` feature: pure-Rust HTTPS (`ureq` + `rustls`, bundled Mozilla
roots, no system library), `zip`, `sha2`. The default build does not contain them. `<data>` is the data folder
of `oc_install::paths` (`%APPDATA%\OpenCompanion`, `~/Library/Application Support/OpenCompanion`,
`~/.local/share/opencompanion`).

## Release assets

A release carries three assets:

| Asset | Content |
|---|---|
| `opencompanion-firmware-<dsp>-<tas>.zip` | the two `.ocfw` containers (below) + `firmware-manifest.json` |
| `opencompanion-firmware-<dsp>-<tas>.zip.sha256` | `<sha256 hex>  <zip name>` |
| `firmware-manifest.json` | the same manifest, alone (the update check reads only this) |

`<dsp>` and `<tas>` are the semantic versions (`0.2.1`, `0.4.7`). Files in the zip:

```
opencompanion-dsp.ocfw     DSP container: opencompanion_dsp.out, loader.out, boot/{tramp,boot1,updater}.out
opencompanion-usb.ocfw     USB controller container: oc_tas.dfu, oc_tas_data.bin
firmware-manifest.json
```

No Bose file is ever part of a release: the restore uses the user's own backup.

## Manifest

```json
{
  "format": "opencompanion-firmware",
  "manifest_version": 1,
  "dsp_version": "0.2.1+20261101235107",
  "tas_version": "0.4.7",
  "min_app_version": "0.1.0",
  "files": [
    { "path": "opencompanion-dsp.ocfw", "size": 76543, "sha256": "<64 hex>" },
    { "path": "opencompanion-usb.ocfw", "size": 8421, "sha256": "…" }
  ]
}
```

- `dsp_version`: the string the DSP reports (`SVM`, without `OC`), with its build stamp; `tas_version`: the
  USB controller's `INFO` version.
- `files`: exactly the two containers above.
- `min_app_version`: an older app does not use the release (status line "needs app … or newer").

## `.ocfw` container

Every firmware file the app installs is an `.ocfw`, one per chip; the app's "Installer…" accepts nothing else
(the file dialog lists only `.ocfw`). Built by `oc firmware pack` (`scripts/stage_firmware.*`), after the same
identity checks as the installer.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `"OCFW"` |
| 4 | 2 | format version (1), LE |
| 6 | 1 | chip: 1 = DSP, 2 = USB controller |
| 7 | 1 | reserved (0) |
| 8 | 4 | payload length, LE |
| 12 | 4 | CRC-32 of the payload, LE |
| 16 | 32 | firmware version, UTF-8, NUL-padded (`0.2.3+20261102112526`, `0.5.4`) |
| 48 | 16 | minimum app version, UTF-8, NUL-padded |
| 64 | … | payload: entry count (u16), then per entry: name length (u16), name (`/` paths), data length (u32), data |

Entries: DSP `opencompanion_dsp.out`, `loader.out`, `boot/tramp.out`, `boot/boot1.out`, `boot/updater.out`;
USB `oc_tas.dfu`, `oc_tas_data.bin`, in that order, nothing else.

Checks, in order, before anything is written (code: `oc-device/src/install/ocfw.rs`, `identity.rs`):
1. the container: magic, format, chip (the one the user picked), length, CRC-32, minimum app version, entries;
2. the contents (also for the development tree and the GitHub releases):
   - DSP: a TI C6000 ELF (`e_machine` 140) with our version marker (`"D" OC_VERSION "U"`); then the image build
     checks the `oc_eeprom_layout_v*` symbol, the load segments and the body size against the U7001 map;
   - USB controller: the `OC-x.y.z` marker in the code, the `OCDA` data area (magic, CRC-32), and our USB identity
     in its device descriptor (05A7:1020 with bcdDevice ≥ 0x0500 or the "OpenCompanion" product string, or the
     development IDs 1209:0001 / 1209:0002). A stock Bose image is refused (only the restore writes the stock
     firmware, from the user's verified backup).

A file that fails shows "Ce fichier n'est pas un firmware OpenCompanion pour ce processeur". Installing an older
version than the one running is allowed, with an explicit warning in the confirmation.

## What the app does (production build)

1. **Update check** (each connection, opening Settings › Firmware or the installer, after an install):
   `GET https://api.github.com/repos/Uneo7/OpenCompagnon/releases/latest` (User-Agent `OpenCompanion/<version>`,
   no token), then the `firmware-manifest.json` asset. Its versions are compared with the running ones; the
   Firmware rows show "current → available".
2. **Update / first install**: a "Download" step downloads the zip (real progress), checks its SHA-256
   (`.sha256` asset, and GitHub's `digest` when given), extracts it into `<data>/firmware/<tag>.partial`, checks
   that the inner manifest equals the published one, every file's size and SHA-256, and both `.ocfw` containers
   (opened next to them), then renames the folder
   to `<tag>` and marks it `.verified`. The install then uses that folder.
3. **Offline or rate-limited** (60 requests per hour without a token): the newest verified release stays in use,
   silently; `InstallController::release_status()` gives a short line for the UI.

Nothing is ever uploaded by the app or the scripts: the release workflow only creates a draft, and publishing it is a
manual step ([`releasing.md`](releasing.md)).

## Code

| Part | Where |
|---|---|
| Manifest, file list, SHA-256 | `app/crates/oc-device/src/install/release/manifest.rs` |
| GitHub API and downloads | `…/release/github.rs` |
| Verified cache `<data>/firmware/<tag>/` | `…/release/store.rs` |
| Packaging (`oc firmware package`) | `…/release/package.rs`, `app/crates/oc-cli/src/firmware_cmd.rs` |
| Update check, download step | `…/install/check.rs`, `…/install/runner.rs` |

Tests use a local HTTP server (`release::github` tests) and a reproducible zip (`release::package`).
