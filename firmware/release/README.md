# Firmware files of the next release

The firmware containers that the release workflow (`.github/workflows/release.yml`) packages into
`opencompanion-firmware-<dsp>-<tas>.zip`. They are committed because CI cannot build them: the DSP firmware needs
TI's C6000 compiler (not redistributable), and both firmwares are validated on a speaker before a release.

| File | Content |
|---|---|
| `opencompanion-dsp.ocfw` | `opencompanion_dsp.out` (`firmware/dsp`), `loader.out`, `boot/{tramp,boot1,updater}.out` |
| `opencompanion-usb.ocfw` | `oc_tas.dfu`, `oc_tas_data.bin` (`firmware/usb`, SDCC) |

Each `.ocfw` is a checked container (magic `OCFW`, chip, version, CRC-32, minimum app version); only these two
files are committed, every raw build output (`firmware/**/build/`) is ignored.

To update them for a release:
1. build both firmwares and test them on a speaker;
2. `app/scripts/stage_firmware.sh` (or `.ps1`) wraps them here (`oc firmware pack`, identity checks first);
3. `app/scripts/package_firmware.sh --from ../firmware/release` checks that they package (versions printed);
4. commit this folder with the release.

Format of the archive and manifest: [`docs/developer/firmware-releases.md`](../../docs/developer/firmware-releases.md).
