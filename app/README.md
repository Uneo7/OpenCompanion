# app — OpenCompanion (Rust)

Rust workspace (`cargo build --release`):

| Crate | Role |
|---|---|
| `oc-ais` | DSP AIS images: reader (including Bose's zlib extension), writer, TI ELF → AIS |
| `oc-protocol` | USB link: vendor HID (report ID 2), DSP console (BRR/BWW/text), TAS EEPROM `t` reads / guarded `T` writes |
| `oc-ramload` | DSP RAM loader, host side (stub install through BWW, hijack, loader protocol) |
| `oc-tas-image` | TAS1020B image format: 18-byte header (checksum, DataType, header-overwrite bit) + code |
| `oc-dfu` | USB DFU 1.0 host for the TAS1020B ROM boot loader (`nusb`) |
| `oc-install` | installation and update chain: DSP boot-chain migration M1..M5 and updates, TAS EEPROM flashing, status (transport-agnostic, tested on a simulated speaker) |
| `oc-osvolume` | OS volume/mute of the speaker (Core Audio on Windows/macOS, ALSA mixer on Linux) with change notifications; `cargo run -p oc-osvolume --example osvolume` lists devices (read-only) |
| `oc-cli` | `oc` tool (below) |
| `oc-app` | egui app |

## `oc` commands
| Command | Effect |
|---|---|
| `oc ais info/unpack/pack` | DSP AIS images |
| `oc probe`, `oc dump-tas` | read-only status and TAS EEPROM dump |
| `oc dsp ramload [--image X] [--dry-run] [--probe] [--skip-stub]` | run a DSP image from RAM (same as `tools/dsp_ramload.py`; standby/unplug returns to Bose) |
| `oc tas mode show` | decode the TAS EEPROM header |
| `oc tas mode dfu --yes` | DataType 2: next power-up boots the ROM DFU (speaker unusable until a firmware is downloaded or Bose restored) |
| `oc tas mode app --yes` | DataType 1 (stock Bose header), needs the Bose firmware running |
| `oc tas dfu list` | DFU devices on the bus |
| `oc tas dfu download <image> [--to-ram] [--id VID:PID] --yes` | DFU download of a TAS image (header + code) |
| `oc tas restore-app [--image u6002_tas.bin] --yes` | from DFU mode back to Bose: downloads the Bose image with the header written to the EEPROM (DataType 1, attributes 0x0A), then `oc tas mode app --yes` restores the exact stock header (0x08) |
| `oc tas image info/build` | inspect / build TAS images |
| `oc install status` | what runs on each chip: stock / OpenCompanion version / rescue / ROM DFU (reads only) |
| `oc install dsp [--dry-run]` | DSP: from stock, M1 hijack → updater → boot1 → boot0 (critical window) → image, header last → reboot + `SVM`; later, updates through rescue. Resumable; asks to type `WRITE` before the first EEPROM write |
| `oc install tas [--image oc_tas.dfu] [--dry-run]` | TAS: header DataType 3 → code page by page (read back) → final header (DataType 1 last) → full verify; through the stock `T` command or our firmware's EE_* commands. Resumable, same confirmation |

Writes (`mode`, `dfu download`, `restore-app`) are dry runs unless `--yes` is given. `oc tas mode` refuses any
header that is not the stock Bose one (apart from DataType and the header-overwrite bit).

## Windows: WinUSB driver for the TAS DFU interface
In DFU mode the TAS1020B ROM enumerates as a DFU device (VID/PID from the EEPROM header, e.g. `05A7:1020`, or
`FFFF:FFFE` without a valid header). Windows has no class driver for it: install **WinUSB** on that device with
[Zadig](https://zadig.akeo.ie/) (Options → List All Devices → select the DFU device → WinUSB → Replace Driver).
Only do this for the DFU-mode device, never for the speaker's audio/HID interfaces in normal mode.
On macOS no driver is needed. On Linux, HID goes through hidraw: give the user access with the udev rule
`packaging/linux/70-opencompanion.rules` (published with each release; it also has the two DFU-mode lines of
`docs/recovery.md`), copied to `/etc/udev/rules.d/`. Its HID lines:
```
KERNEL=="hidraw*", ATTRS{idVendor}=="05a7", ATTRS{idProduct}=="1020", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0001", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0002", TAG+="uaccess"
```
then `sudo udevadm control --reload && sudo udevadm trigger` and replug the speaker. The `05a7` line covers the stock
firmware and OpenCompanion 0.5.0+ (same 05A7:1020, told apart by bcdDevice / product string, `oc_hid::is_opencompanion`);
the `1209` lines only a speaker still on OpenCompanion 0.4.x. During EEPROM writes the
computer is kept awake (Windows `SetThreadExecutionState`, macOS IOKit assertion, Linux logind inhibitor through
`systemd-inhibit`; a warning is printed when none is available).

Verified: `oc ais unpack u7001_dsp.bin` reproduces the stock Bose RAM image exactly; `oc dsp ramload --dry-run`
prints the same plan as `tools/dsp_ramload.py`.

## Cross-platform build check
Windows, macOS and Linux are all first-class targets. From any host:
```
scripts/check_all_targets.sh            # Linux, macOS, Git Bash
scripts\check_all_targets.ps1           # Windows PowerShell
```
It builds and tests the host, cross-links `x86_64`/`aarch64-unknown-linux-gnu` with
[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) (needs `zig` and `cargo install cargo-zigbuild`;
libudev and libasound are taken from Debian packages into `target/cross-sysroot`), and builds both macOS targets
on a Mac. Elsewhere macOS is only type-checked (`cargo check`): hidapi's `mac/hid.c` and the final link need the
Apple SDK, which is not redistributable (set `SDKROOT` to a local SDK to link with zig).
Native Linux build dependencies: `libudev-dev libasound2-dev libgtk-3-dev pkg-config` (X11, Wayland and GL are loaded at run
time). CI: `../.github/workflows/ci.yml` (Windows, macOS, Ubuntu: build + test; macOS x86_64; Linux aarch64).

## Release packages
Unsigned, into `dist/`, built in `target/package-app` (never `target/release`):
```
scripts\package_app.ps1                                  # Windows: portable zip + oc zip
scripts/package_app.sh                                   # macOS: universal .app (zip, dmg) + oc; Linux: AppImages
scripts/package_app.sh linux-x86_64 linux-aarch64        # Linux binaries from any host (zig; AppImage on Linux only)
scripts/package_firmware.sh --from ../firmware/release   # firmware archive from the committed files
```
Every artefact gets a `.sha256`. Assets, workflows, committed firmware files and publishing steps:
[`../docs/developer/releasing.md`](../docs/developer/releasing.md).
