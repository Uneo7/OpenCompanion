# Development loop

How to test firmware changes on the speaker without flashing anything until a build has passed its test plan.
Paths are relative to the repository root (`bose/`); `oc` is `opencompanion/app/target/release/oc.exe`
(`cargo build --release` in `opencompanion/app`, or `cargo run -p oc-cli --`).

## Safety rules
- **Never flash an untested image.** Every new USB-controller or DSP build runs from RAM first and passes the
  hardware test plan of its README (`firmware/usb/README.md`, `firmware/dsp/README.md`) before any EEPROM write.
  Never write a TAS header with DataType 1 for an untested firmware.
- **Low volume**: OS volume at 10 % or less, pod knob fully down, nothing playing until a test step says so;
  test tones at −60 / −40 dBFS. The DSP keeps its −12 dB ceiling and −6 dBFS limiter until the protection has been
  verified (`firmware/dsp/README.md`, "Output level and safety").
- **Backups**: the reference dumps are in `backups/2026-10-31/` (`u6002_tas.bin`, `u7001_dsp.bin`,
  `tas1020b_rom_8000.bin`, `SHA256SUMS`), read-only. Put any new EEPROM read in `backups/` too (e.g.
  `backups/recovery/`), never overwrite the reference set, and check hashes before using a dump for a restore.
- Keep the PC awake and the USB cable connected during any EEPROM write (`oc-install` inhibits sleep itself).
- Prefer `--dry-run` first: `oc install dsp|tas`, `oc tas data write` and `oc dsp ramload` all have one.

## USB controller (TAS1020B): RAM testing through the ROM DFU
The TAS ROM can boot a firmware into RAM over USB DFU without touching the EEPROM code.

1. **Data area (once per layout change)**: our TAS firmware streams its USB descriptors from the U6002 data area
   (0x1800..0x1FFF) and stays off USB if it is missing or invalid.
   ```
   make PYTHON="py -3"              # in opencompanion/firmware/usb: oc_tas.dfu + oc_tas_data.bin
   oc tas data info                 # layout, length, CRC-32 of the blob
   oc tas data write --dry-run      # reads 0x1800..0x1FFF, lists the pages to write
   oc tas data write                # type WRITE; only 0x1800..0x1950, each page read back
   ```
   It never writes below 0x1800. Through the stock firmware it uses the stock `T` command. Through our firmware
   it opens an EEPROM session (header DataType 3) and **closes it with the stock Bose header (DataType 1)**: the
   next power-up boots the untouched stock code.
2. **Enter the ROM DFU**, either:
   - from the stock firmware: `oc tas mode dfu --yes` (header DataType 2), then power-cycle the speaker; or
   - **software DFU entry** from our running firmware (validated): HID `REBOOT_DFU` (0x7E,
     `uv run tools/oc_tas.py dfu`). The ROM DFU appears as `05A7:1020` within ~1 s, no power cycle, nothing
     written. Fallback without a host command: touch the pod mute 4 times within 8 s after power-on (not yet
     validated on hardware).
3. **Download and run**, in an **elevated** (administrator) PowerShell:
   ```
   powershell -ExecutionPolicy Bypass -File tools\tas_dfu_run.ps1 opencompanion\firmware\usb\build\oc_tas\oc_tas.dfu
   ```
   The script (log in `tools/tas_dfu_run.log`): checks that `05A7:1020` is really the ROM DFU (class FE); binds
   WinUSB from `tools/dfu_driver` (package generated once by Zadig); runs `oc tas dfu download <image> --to-ram
   --yes`; restarts the device with `pnputil /restart-device` (the bus reset that starts the RAM code: WinUSB
   cannot issue it, and unplugging the cable does not work); then removes the WinUSB package again, because the
   stock firmware shares `05A7:1020`. Without an argument it loads `firmware/usb/build/oc_tas_test.dfu`.
   Expect `1209:0001` after a few seconds; `uv run tools/oc_tas.py info` for status.
4. **Back to normal**: a power cycle with DataType 2 returns to the ROM DFU (nothing persistent). To boot the
   stock firmware again: `uv run tools/tas_romdump.py --restore-header --yes` while `oc_tas_test` runs (writes
   only if the header is exactly the DataType-2 stock header, then reads it back; validated), or `oc tas mode app
   --yes` while the stock firmware runs.

Notes: the ROM announces a MANIFEST poll timeout of ~2097 s (endianness bug); our `REBOOT_DFU` passes its own
256 ms table, a cold-boot DFU needs the host to cap it. A USB suspend between download and reset blocks the ROM
DFU loop: power-cycle. The stock Bose firmware run from RAM is not usable as a test bench (no sound).
macOS / Linux: no driver; Linux needs udev access rules (see `opencompanion/app/README.md` and
`opencompanion/docs/recovery.md`), not tested.

## DSP: RAM loader and rescue
- **Stock DSP firmware in U7001** (before migration): `oc dsp ramload --image <elf> [--dry-run]` (same as
  `tools/dsp_ramload.py`) installs the loader stub through `BWW`, hijacks the output meter's process pointer and
  loads the image into RAM (`firmware/dsp/README.md`, RAM loader section). Nothing is written to any EEPROM; standby or
  unplugging reboots the stock firmware.
- **Migrated U7001** (boot chain installed): the stock hijack no longer applies. Reach **rescue mode** instead:
  console `RSC` (rescue flag + reboot), or `UPD` (invalidates the header, then reboots; this is the first step of
  an update), or any invalid image. Rescue answers the RAM loader protocol unchanged (INFO / WRITE / CRC / JUMP /
  FILL / REBOOT, `firmware/dsp/boot/docs/RESCUE.md`), so a test image can be run from RAM with JUMP and the
  EEPROM image comes back with `REBOOT` (`E 06`) or a power cycle. Check with `uv run tools/oc_tas.py dsp E
  --data 00` → `06 'OCL' 01 'R' <reason>`. Test 12 of `firmware/usb/README.md` walks through this.
- Our TAS firmware drops all DSP traffic while the DSP restarts and redoes the DSPINT handshake (`firmware/usb/
  README.md`, "DSP restarts"); relayed commands answer status 04 meanwhile and the host retries.
- A RAM-loaded DSP firmware must carry `oc_eeprom_layout_v2` (profiles at 0xC200): an older ELF would write its
  profiles over boot1.

## Installing (EEPROM writes)
Only after the RAM tests passed.
```
oc install status            # reads only: stock / OpenCompanion version / rescue / ROM DFU, per chip
oc install dsp --dry-run     # every read and check, updater loaded into RAM, no EEPROM write, DSP rebooted
oc install dsp               # from stock: M1..M5 migration (critical window M3, ~17 ms); later: update via rescue
oc install tas --dry-run
oc install tas               # DataType 3 → code pages (read back) → data area → final header, DataType 1 last
```
Both are resumable and ask to type `WRITE` before the first EEPROM write. `oc install dsp` is validated (migration
and normal update); `oc install tas` is **not yet validated** on hardware. `oc install restore [--dsp|--tas]`
(stock images from the user's backup, DSP first, TAS rebooted through our `REBOOT`) exists in the CLI but is **not
yet validated**; the app's "Restaurer…" is not wired yet. `oc backup` reads both EEPROMs (no write) through the stock
or our firmware. Details: `opencompanion/app/README.md`, `firmware/dsp/boot/docs/UPDATE.md`,
`re/docs/protocol_usb_hid.md` (EEPROM session commands 20..24).

## ROM DFU as a recovery path (backup only, non-nominal)
Nominal installs and updates go through HID only. The ROM DFU is the fallback when the TAS no longer runs an
application. User guide: `opencompanion/docs/recovery.md`. Order of escalation: app resume → DSP rescue (RSC,
validated through our TAS firmware) → ROM DFU → EEPROM programmer.

- **ROM boot logic** (`re/tas_rom/NOTES.md` §1.1, §4, static): DataType 1 → load the EEPROM app; DataType 2 or no
  EEPROM ACK (FE) → `DFU_EnterMode(1)`, **RAM only, never writes the EEPROM**; DataType 3 (interrupted TAS
  install, bad checksum tolerated), invalid header (FF) or failed DataType-1 load → `DFU_EnterMode(2)`: **a download is written to
  U6002** (header + code, below 0x1800, data area untouched), even with Attributes.1 = 0. Mode 2 recovery is
  **not yet validated** on hardware.
- **Identity**: VID/PID of the header copy when DataType is 2 or 3 (`05A7:1020`), else `FFFF:FFFE`. `0451:1021`
  is dead TI-kit code in the stock firmware (still in `TAS_DFU_IDS`). In normal mode the stock firmware also uses
  `05A7:1020` (class 00/EF): `tas_dfu_run.ps1` refuses to bind unless the compatible IDs show `Class_FE`.
- **Entry**: interrupted `oc install tas` (DataType 3, automatic); `oc tas mode dfu --yes` + power cycle (dev);
  `uv run tools/oc_tas.py dfu` (`REBOOT_DFU`, validated, RAM mode, own 256 ms poll table); boot escape (4 pod mute
  touches within 8 s after power-on, **not validated**; no pod or a hang before `main_idle()` = no escape).
  `uv run tools/oc_tas.py reboot` reboots through the ROM (validated); `oc_tas.py ee-read` reads U6002.
- **Windows driver**: elevated PowerShell is required (Zadig, `pnputil /add-driver|/restart-device|/delete-driver`).
  `tools/dfu_driver` (Zadig/libwdi package) only matches `VID_05A7&PID_1020`, so `tas_dfu_run.ps1` does not handle
  `FFFF:FFFE`: bind it with Zadig (https://zadig.akeo.ie, never redistributed by us), then download and
  `pnputil /restart-device <instance id>` in one command line. Always remove the libwdi package afterwards
  (`pnputil /delete-driver oemNN.inf /uninstall /force`; the script does it) or the stock firmware loses its audio
  and HID. macOS: no driver. Linux (not tested): hidraw rules for `05a7:1020` / `1209:0001` plus
  `SUBSYSTEM=="usb"` rules for `05a7:1020` and `ffff:fffe` (usbfs, `oc-dfu`/`nusb`), text in `recovery.md` §4.2.
- **Recipes**:
  - *Reinstall OpenCompanion*: `tas_dfu_run.ps1 …\oc_tas.dfu` (or `oc tas dfu download oc_tas.dfu --to-ram [--id]
    --yes` + immediate reset) → `1209:0001`; in mode 2 the code is now in U6002, so if nothing shows up, power-cycle
    once. No `1209:0001` at all = data area missing/invalid (firmware stays off USB by design): go back to stock
    instead. Then `oc install tas` (resumable, rewrites only what differs, data area included) and power-cycle.
  - *Back to stock from mode 1 (DataType 2 dev header)*: power cycle → `tas_dfu_run.ps1` (default
    `oc_tas_test.dfu`) → `uv run tools/tas_romdump.py --restore-header --yes` (writes only if the header is exactly
    the DataType-2 stock header, reads it back) → power cycle. **Validated** (U6002 equal to the Pico dump, sound OK).
  - *Back to stock from mode 2 (interrupted install)*: `oc tas restore-app --image <backup>/u6002_tas.bin --yes`
    (stock image, header DataType 1 / attributes 0x0A) → power cycle → `oc tas mode app --yes` (exact header,
    attributes 0x08). Or get `oc_tas` running first and use `oc install restore`. **Not validated.**
  - Never use the stock firmware run from RAM as a fix: it starts (green LED) with no sound and once enumerated
    with its TI DFU descriptors (0xFA39 left at 5).
- **Pitfalls**:
  - Never replug the cable to "start" the downloaded code: unplug = suspend, the ROM then spins at 0x82BD
    waiting for a resume that never comes (`NOTES.md` §3.4); only a power cycle gets out. Same if Windows
    selective-suspends the WinUSB device between download and reset: the reset must be immediate.
  - WinUSB cannot issue the port reset; `oc-dfu` then prints "USB reset request failed … replug the cable".
    That hint is wrong on this chip: use `pnputil /restart-device` (the script) or power-cycle.
  - ROM `bwPollTimeout` in MANIFEST ≈ 2097 s (endianness bug, the ROM itself waits 32 ms): `oc-dfu` clamps each
    poll to 500 ms (200 polls max).
- **Last resort** (rare): open the speaker, I²C EEPROM programmer (the Pico used for the reference dumps) with a
  clip on U6002 (AT24C64A, TSSOP-8) or U7001 (AT24C512, SOIC-8), speaker unpowered, restore from hash-checked
  backups. TI's alternative (`re/docs/txt/TAS1020B_Firmware_Update_Procedure.txt`, appendix D: short SCL/SDA of the
  TAS EEPROM at power-up to force DFU) is not validated on the Companion 5.
