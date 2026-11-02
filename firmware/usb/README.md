# firmware/usb — TAS1020B (U6000)

C firmwares for the TI TAS1020B USB controller of the Bose Companion 5, built with SDCC.
References: Data Manual SLES025B (`re/docs/txt/tas1020b-557821.txt`, "DM" below), service manual
(`re/docs/txt/bose_companion5_service_manual.txt`), stock firmware in Ghidra (`u6002_tas.bin`, every routine
named), boot ROM analysis (`re/tas_rom/NOTES.md`).

| Target | Sources | Output | Purpose |
|---|---|---|---|
| `oc_tas` | `app/` + `common/` | `build/oc_tas/oc_tas.dfu` + `build/oc_tas/oc_tas_data.bin` (U6002 data area) | **OpenCompanion firmware**: USB Audio Class 2.0 speaker (full speed; user-selected audio mode 5.1 16-bit or 2.1 = stereo 24-bit; pod → OS volume through the UAC2 interrupt endpoint) + vendor HID + board control |
| `oc_tas_test` | `test/` + `common/` | `build/oc_tas_test.dfu` | ROM dump / read-only test firmware, plus the one-shot `RESTORE_HEADER` |

Both images are for **RAM download through the ROM DFU** (header Attributes bit 1 = 0, `scripts/mkdfu.py`).
Nothing in this folder has been run on hardware yet except the original `oc_tas_test` stack.

## Build
```
make PYTHON="py -3"          # both targets + the data area blob; SDCC in PATH (C:\Program Files\SDCC\bin)
make size PYTHON="py -3"     # code size / buffer usage of oc_tas
```

### Size (SDCC 4.5.0, `--opt-code-size --peep-return --no-xinit-opt`)
| | Used | Limit |
|---|---|---|
| `oc_tas` code | **5940 bytes** (0.4.9) | 6016 (program RAM 0x0000..0x177F, DM 6.1) — 76 bytes free; overlays are impossible (code RAM read-only at run time, lever 3) |
| `oc_tas` endpoint buffer space | 1208 bytes of buffers + 96 bytes of XDATA variables (16 used) | 1304 (0xFA10..0xFF27, DM 6.4.2) — full |
| `oc_tas` internal RAM | data up to 0x7F, stack 0x80..0xFF (128 bytes) | 256 — direct RAM full (INFO uses R2..R5 as scratch) |
| U6002 data area (`oc_tas_data.bin`) | **741 bytes** (12-byte header + 729-byte payload, layout 6) at 0x1800..0x1AE4 | 2048 (0x1800..0x1FFF) — 1307 bytes free |
| U6002 audio mode record | 1 byte at 0x17FF | outside code area and data area |
| `oc_tas_test` code | 3240 bytes | 6016 |

The code RAM is the binding constraint. Left out (see "Not implemented"): stand-by, a memory-peek command. No
feature was dropped for UAC2, for the formats of 0.3.0 or for the audio modes of 0.4.0 (0.4.0 drops the 3-channel 2.1
alternate setting of 0.3.0 by design: one format per mode). Endpoint buffers: unchanged map, the 8-byte EP3 IN buffer
of the former consumer control now carries the UAC2 interrupt messages.

**UAC2 budget** (measured build to build, SDCC 4.5.0): UAC1 + HID consumer control 5985 bytes; first complete
UAC2 build 6030 (14 over); after normal optimisation 6012; with the data area 5974.
| Item | Bytes |
|---|---|
| Removed: `usb/consumer.c` (key press / release) | −78 |
| Removed: consumer interface (25) and report (22) descriptors, their `GET_DESCRIPTOR` branches | about −60 |
| Added: UAC2 configuration descriptor (184 bytes instead of 170) + sampling-frequency and volume RANGE tables (34) | +48 |
| Added: `usb/uac_notify.c` (interrupt data messages) | +90 |
| Rest: `usb/uac.c` UAC2 requests, `board/pod_sync.c` absolute knob mapping, `common/usb_core.c` long class reads | about +45 |

Size levers, in the order requested, and what each gave:
1. **ROM routines** (re/tas_rom/NOTES.md 1.3): **0 bytes, not usable**. The ROM API exports no delay or copy
   routine; its only I²C entry, `I2C_RunTransfer` (API index 1, 0x8EE0), takes its parameter block at XDATA
   0xFA2E..0xFA54 (NOTES.md 1.3, 1.5), inside our EP2 IN buffer (0xFA20..0xFA5F, config.h), and the ROM USB engine
   reserves IDATA 0x68..0x7F (NOTES.md recommendation 1), where our stack lives. The ROM's Keil helpers
   (`?C?CCASE` 0x84D8...) use Keil's calling convention, not SDCC's. `common/i2c.c` (360 bytes) stays.
2. **Constant data in the U6002 data area** (approved 2026-11-01): **−38 bytes net** (6012 → 5974). Moved out of
   the code RAM: device (18) and configuration (184) descriptors, the three strings (64), the vendor HID report
   descriptor (25), the sampling-frequency and volume RANGE tables (34): 325 bytes, plus `string_descriptor()`.
   Added: the boot check `data/ee_area.c` (236 bytes: header compare, byte-wise CRC-32 in assembly), the EEPROM
   branch of the EP0 IN loader (`common/usb_core.c`, ~48), the larger write window of `proto/eeprom.c` (+19).
   A code-space fallback descriptor set (HID-only device, 59 bytes of data + ~50 of code) was built and measured,
   then dropped for size: without a valid data area the firmware stays off USB (below). A table-driven descriptor
   lookup was also measured: 20 bytes larger than the `switch`, not kept.
3. **Code overlays from the EEPROM**: **impossible**. DM 6.1: "in the normal operating mode, the RAM resource
   becomes a read-only memory resource that cannot be written to" (the 6 KB RAM is writable only as XDATA in boot
   loader mode, before SDW). Only constant data can move to the EEPROM (lever 2).
4. **Normal size optimisation**: **−19 bytes**: Feature Unit `GET CUR` volume sent straight from `uac_volume`
   (SDCC keeps int16 little-endian, as USB), volume clamp on the high byte only (MIN 0x8100 / MAX 0 enforced by a
   `_Static_assert`).
Everything fits, with 42 bytes free (50 with the 0.2.1 sample format: 8-byte frames simplify the DMA re-alignment check).

**0.3.0 budget** (three formats, measured build to build from 6007): table-driven `stream_set_alt()` (C-port BPTSL
switch, DMA slots, endpoint; +47 code, +28 tables), slot-size mask for the DMA check, `R` layout byte, descriptor
tables (EEPROM only) → 6086, 70 over. Paid back, nothing removed:
| Change | Bytes |
|---|---|
| `usb_app_descriptor()`: lookup table `desc_tab` in the data area, read by I²C, loop in assembly (replaces the 122-byte `switch`; parameters copied to statics, see the code) | −37 |
| `protocol.c key_ok()` in assembly, and | −26 together |
| `uac.c`: "SET only with CUR" checked once in `uac_setup()` instead of per control | (above) |
| `protocol.c`: REBOOT / REBOOT_DFU in one case (`ACTION_*` = command codes) | −19 |
| Tried and dropped (larger): CPTCNF2 inside the per-alt table (+15), 5-byte fixed frame copy in `dsp_transact` (+4) | — |

**0.4.0 budget** (audio modes, from 6004): mode record read at boot (+12, one 13-byte read covers 0x17FF and the
header), MODE command (~+80), re-enumeration (`usb_restart()`, shared with the boot sequence), per-mode descriptor
table (+7), layout byte → 6161. The 3-channel alt (−7) and these savings brought it to 6001:
| Change | Bytes |
|---|---|
| Mode record at 0x17FF read with the header (instead of a separate record in the data area: +36, and host tools that would erase it) | −24 |
| `protocol.c cmd_info()` in assembly: one source table (internal RAM or 0xFFxx register per reply byte), flags and tick in R2..R5 | −85 |
| EP0 OUT int16 read through a pointer (`uac.c`) | −9 |
| `dsp_link.c` version copy in assembly; `R` rate bytes with one test | −10 |
| `protocol.c`: EE commands as one range test; reply padding in assembly | −14 |
| `stream.c`: CPTCNF2 and slot mask computed from the mode (no register read-back) | −6 |
| Tried and dropped: a static 16-bit address in `cmd_ee` (direct RAM full), a static scratch block for INFO (same) | — |

Earlier measured savings (UAC1 + consumer control), still in place:
| Change | Saved (bytes) |
|---|---|
| `app/sys/crt0.asm` replaces SDCC's generated vector table (which carries 71 bytes of unused "restartable atomic support routines") and its start-up modules; C entry renamed `oc_main` | 93 |
| `servo.c dma_realign()`: `bytes % 6` with 8-bit `DIV` (no 16-bit `__moduint` library routine) | 81 |
| `dsp_link.c dsp_transact()`: register pointers + `xcopy()` instead of a static 16-bit pointer | 56 |
| request frames no longer zero-padded (the stock TAS sends stale bytes too) + `pod_sync.c` on a single 20 ms timer | 84 |
| `common/xreg.c`: table-driven register writes (2 bytes per write instead of 6) in `stream_init`, `ep0_init`, `hid_init` and the interrupt endpoint init; `hid_tx_free()` without bit-to-byte conversion | 91 |
| `protocol.c cmd_info()`: table of the internal-RAM bytes copied into INFO; `configured` bit set in `flags0()` | 46 |
| `rom.c quiesce()`: endpoint-disable loop over register addresses (EP_CNF stride 8); `key_ok()` with early returns | 35 |

### U6002 data area (`app/data/`)
| EEPROM range | Content |
|---|---|
| 0x0000..0x0011 | TI boot header |
| 0x0012..0x15F5 | stock Bose code (5604 bytes; checked on `u6002_tas.bin`: 0xFF from 0x15F6 to the end) |
| 0x0012..0x1791 | largest OpenCompanion image (6016 bytes of code) |
| 0x17FF | **audio mode record** (0.4.0): 2 = mode 2.1, anything else (blank FF) = 5.1; written only by HID `MODE` |
| **0x1800..0x1FFF** | **data area** (2048 bytes, 64 whole pages): overlaps neither image |

Format (`data/ee_layout.h`): `"OCDA"`, layout version (**5** since 0.4.1; 4 = 0.4.0, 3 = 0.3.0, 2 = 0.2.x), 0,
payload length (LE16), CRC-32 of the payload (zlib / crc32fast, LE32), then `struct ee_data` (GET_DESCRIPTOR lookup
tables `desc_tab_51` and `desc_tab_21`, device descriptors of mode 5.1 (PID 0x0001) and of mode 2.1 (PID 0x0002), configuration of mode 5.1 (200 bytes) and of mode 2.1 (184), strings 0..2,
frequency RANGE, volume RANGE, HID report descriptor), then 0xFF. A firmware only accepts its own layout version:
0.4.1 stays off USB with an older area, and older firmwares with a layout-5 one.

**Audio mode record** (0.4.0, 0x17FF): kept outside the data area on purpose. The host tools never write there
(code updates stop at 0x1791, data-area writes start at 0x1800, `EE_WRITE` refuses 0x1792..0x17FF), so the user's
mode survives every update without host support; it is outside the CRC, so a bad byte can never keep the device off
USB (anything but 2 = 5.1); it is one byte (single-byte EEPROM write, no page tearing); the boot check reads it in
the same 13-byte I²C read as the header. A spare byte of the TI boot header was rejected (checksummed, read by the
ROM) and so was a CRC-covered byte (a torn update of value + CRC would take the device off USB). Built by `make` from `data/ee_data.c` (compiled on its own, never linked into
the firmware) into `build/oc_tas/oc_tas_data.bin` by `scripts/mkeedata.py`, which also checks every descriptor
length, wTotalLength and RANGE count (a short C initialiser would be zero-padded silently). Host side:
`oc-tas-image::data_area` (same format, tests), `oc tas data info|write`, `oc install tas --data`.

At boot (`data/ee_area.c`, before the USB connection) the firmware reads the header, compares magic, version,
reserved byte and length, and checks the CRC-32 of the payload (one byte per I²C read, ~30 ms). **If anything is
wrong (blank area, other layout, torn write, I²C error) the firmware never connects to USB**: no descriptors, no
half-enumerated device; the rest runs (DSP link, AUX, pod, LOCAL knob mode), nothing waits on the EEPROM.
Recovery: write the data area (`oc tas data write` with the stock firmware, or `oc install tas`), or reach the
ROM DFU (power cycle in DataType 2; boot escape = 4 pod mute touches within 8 s).

At run time, `usb_app_descriptor()` looks the request up in the table of the current audio mode (`desc_tab_51` at
0x180C, `desc_tab_21` at 0x180C + 43: 6-byte entries `[type][index][length LE16][EEPROM address LE16]`, type 0 =
end; each entry is read into the EP0 IN buffer, which EP0 NAKs while the setup is decoded; at most 7 reads
≈ 2 ms). Both tables share the device descriptor, strings and HID report; they differ in the configuration and the
HID class descriptor inside it. It and the class GET RANGE / GET CUR (rate) return **EEPROM addresses**: the USB
core treats any source address ≥ 0x1800 (`USB_EE_BASE`, above the 0x177F end of the code space) as EEPROM and
reads each 8-byte EP0 packet by I²C straight into the EP0 IN buffer (`common/usb_core.c`). EP0 stays at 8 bytes
(bMaxPacketSize0 8): a 64-byte EP0 would need 112 more bytes of endpoint buffer, and the 1304-byte space is full.
The reads never interleave with another I²C transfer (`i2c.c` never yields; `main_idle()` only runs between
transfers); if one fails (EEPROM in a write cycle during an update), the request is stalled and the host retries.

## Layout
```
common/            shared by both targets
  tas1020b.h       register map (DM 6.4, 6.5)
  usb_core.c/.h    polled EP0 engine (refactored from the validated oc_tas_test stack) + usb_app_* callbacks
  i2c.c/.h         I2C master, port of the stock I2cWrite 0x0E10 / I2cRead 0x001E
app/               OpenCompanion firmware
  xreg.c/.h        table-driven writes to the 0xFFxx registers
  main.c           start-up order, main loop (oc_main), main_idle()
  config.h         identity, versions, endpoint buffer map
  usb_config.h     USB interrupt sources for the core
  sys/             crt0.asm (reset / Timer 1 vectors, C start-up), tick.c (1 ms Timer 1), rom.c (ROM DFU /
                   reboot hand-over)
  usb/             descriptors.h (constants), usb_app.c (core callbacks, descriptor addresses), uac.c (UAC2
                   class requests), uac_notify.c (UAC2 interrupt endpoint, EP3), hid.c (EP2)
  data/            U6002 data area: ee_layout.h (format), ee_data.c (descriptors and tables: built into
                   oc_tas_data.bin, not linked), ee_area.c (boot check, CRC-32)
  audio/           stream.c (clocks, codec port, DMA, ISO endpoint), servo.c (ACG lock to USB SOF)
  board/           power.c (pins, VREN, DSP reset, USB presence), dsp_link.c (I2C frames, DSPINT
                   handshake, 'S' poll), pod.c (mute debounce, pod detection, boot escape), pod_sync.c (pod knob
                   and mute -> Feature Unit CUR + interrupt)
  proto/           protocol.c (vendor HID commands), eeprom.c (guarded EEPROM access)
test/              oc_tas_test (see the end of this file)
scripts/           mkdfu.py (18-byte boot header), mkeedata.py (data-area blob), size_report.py
```

## oc_tas — what it does

### USB identity
**0.5.0 (user decision 2026-11-02): the stock Bose VID:PID 05A7:1020, in both audio modes** (the legal side is the
user's call). bcdDevice **0x0500** (stock 0x0100), strings "OpenCompanion" / "Companion 5 (OpenCompanion)" (stock
"Bose Corporation" / "Bose USB Audio"; stock device descriptor at 0x059F of `u6002_tas.bin`: bcdUSB 1.00, bcdDevice
0x0100). The ROM DFU also announces 05A7:1020, but with a DFU interface and no HID.
**How a host tells OpenCompanion from the stock firmware** (no device opened: all of it is enumeration data that
Windows, macOS and Linux report, e.g. hidapi `release_number` / `product_string`), `oc_protocol::oc_hid::is_opencompanion`:
| Device | Rule |
|---|---|
| OpenCompanion 0.5.0+ | 05A7:1020 **and** (bcdDevice ≥ 0x0500 **or** product string contains "OpenCompanion") |
| OpenCompanion 0.4.x (transition) | 1209:0001 or 1209:0002 |
| stock Bose firmware | 05A7:1020 and not the first row (the exact complement: neither path ever opens the other's device) |
| ROM DFU | 05A7:1020 (or FFFF:FFFE, 0451:1021) with no vendor HID interface |
Both firmwares expose a vendor HID with usage page 0xFF00 / usage 1, so the usage page alone is not enough (stock
reports carry report ID 2, ours none). Older identities: 1209:0001 / 0002 (pid.codes test PIDs, 0.2.x..0.4.x).
configuration or driver binding). Device class EF/02/01 (IAD), bcdUSB 2.00, full speed only. Firmware 0.5.4
(**bcdDevice 0x0504**: knob at 0 % mutes; 0x0503 for 0.5.3: pod re-plug detection; 0x0502 for 0.5.2: no knob / pod mute without a pod; 0x0501 for 0.5.1: 48 kHz only, buffer level guard; 0x0500 for 0.5.0: 05A7:1020, interface numbers per mode, data-area layout 7; 0x0410 for 0.4.10: relay reply
matching; 0x0409 for 0.4.9: loader-safe DSPINT handshake rules; 0x0408 for 0.4.8: DSPINT wait timeout, SOF clears suspend, no TAS knob median; 0x0407 for 0.4.7: stock pod curve for the knob, data-area layout 6; 0x0406 for 0.4.6: knob → OS volume restored (0.4.3 path), pod mute reported once when the host becomes active;
0x0405 for 0.4.5: OS mute forced while the pod is muted; 0x0404 for 0.4.4: pod knob left to the DSP, as stock; 0x0403 for 0.4.3: knob median + hysteresis; 0x0402 for 0.4.2: knob hysteresis; 0x0401 for 0.4.1: one PID per audio mode, data-area layout 5, HID protocol 4,
boot-header firmware-version byte 0x41 (0.4.2: 0x42, 0.4.3: 0x43, 0.4.4: 0x44, 0.4.5: 0x45, 0.4.6: 0x46, 0.4.7: 0x47, 0.4.8: 0x48, 0.4.9: 0x49); 0x0400 for 0.4.0: audio modes with a single PID;
0x0300 for 0.3.0: three alternate settings; 0x0201 for 0.2.1..0.2.4: 4-byte format; 0.2.2: 960-byte
ISO buffer, DSP restart handling; 0.2.3: pod detection after a reload, immediate mute report, 44.1 kHz re-alignment
check, INFO 23..24; 0.2.4: 2 % knob deadband; 0.4.2: knob backlash hysteresis, bcdDevice 0x0402).
Strings: "OpenCompanion" / "Companion 5 dev".

### Interfaces
| If | Class | Endpoints |
|---|---|---|
| 0 | Audio Control (UAC2): Clock Source 5 → IT 1 (USB streaming) → FU 3 (master mute + volume) → OT 4 (speaker); IAD over interfaces 0..1 | 0x83 interrupt IN, 6 bytes, 4 ms |
| 1 | Audio Streaming, 44.1 / 48 kHz, PCM: alt 0 idle; alt 1 = the format of the **audio mode** (HID `MODE`, below): **5.1** (L R C LFE Ls Rs) 16-bit in 2-byte subslots, or **2.1** = stereo 24-bit in 4-byte subslots (the DSP adds the subwoofer) | 0x01 iso OUT, adaptive, 588 (5.1) / 392 (2.1) bytes |
| 2 | HID vendor 0xFF00, OpenCompanion protocol (`re/docs/protocol_usb_hid.md`) | 0x82 IN, 0x02 OUT, 64 bytes |

Feature Unit volume: MIN 0x8100 (−127 dB), MAX 0x0000 (0 dB), RES 0x0100 (1 dB), initial 0xC900 (−55 dB); stock
UAC1: 0x8001..0xFFFF, RES 0x0147, initial 0xC8CF. The DSP receives `V` (int16 BE) and `m` on change, like
SendVolumeToDsp 0x136B / SendUsbMuteToDsp 0x158C. Details: "USB Audio Class 2.0" below.

### Audio mode (0.4.0, one USB identity per mode since 0.4.1): the OS cannot pick another format
| Mode | HID `MODE` byte | Configuration descriptor (data area) | Stream | `R` layout |
|---|---|---|---|---|
| 5.1 (default), PID 0x0001 | 1 | `config_51`, 200 bytes: IT 6 ch 0x3F, FU 34 bytes, one alt: 6 ch, 2 / 16, 588 B | stock 5.1 16-bit | 0 idle / 1 streaming |
| 2.1, PID 0x0002 | 2 | `config_21`, 184 bytes (= the 0.2.x stereo one): IT 2 ch 0x03, FU 18 bytes, one alt: 2 ch, 4 / 24, 392 B | stereo 24-bit | 2 idle / 3 streaming |
Each configuration has a single streaming alternate setting, so every OS has exactly one format to choose (at
44.1 or 48 kHz). The 3-channel 2.1 alternate setting of 0.3.0 is gone: Windows never selected it, and in mode 2.1
the DSP builds the subwoofer from L + R (each satellite plays its channel on both drivers). In mode 5.1 the LFE
channel stays available to 5.1 sources.
**One Windows device node per mode (0.5.0: interface numbers; 0.4.1..0.4.10: one PID per mode).** Hardware result
with 0.4.0 (2026-11-01): after switching to 2.1 Windows played nothing. Registry, read only: one device node
`USB\VID_1209&PID_0001&MI_00\7&25E6EDBA&0&0000` for both modes, hence one render endpoint
`{56bd3baf-45fc-44ed-96ca-5fd3dc73069b}` whose cached mix format (MMDevices `Properties\{f19f064d-082c-4e27-bc73-
6882a1bb8e4c},0` = PKEY_AudioEngine_DeviceFormat) was 6 ch, 48000 Hz, 16 bits, mask 0x3F: a format the mode-2.1
interface (2 ch, 24-bit) does not offer. Since 0.5.0 both modes are 05A7:1020 (no second PID), so the audio function
moves instead: **mode 5.1 = AC 0, AS 1, HID 2 (the stock layout); mode 2.1 = HID 0, AC 1, AS 2**. The composite
driver names each function's child device node after its first interface (`...&MI_00` vs `...&MI_01`), so Windows
binds the 2.1 audio function to another device node, with its own endpoint, mix format, volume and default-device
settings; switching back finds the 5.1 node intact (and its 6 ch 16-bit format is also the stock one). The firmware
uses the interface numbers of the current mode (`audio/stream.h` `IF_AC` / `IF_AS`: class requests, SET_INTERFACE,
interrupt message wIndex). Rejected: a second Bose PID (user decision: one identity; usb.ids lists 05A7:1020 and
4000..bc51, so 1021 looked free, but not verifiable for all Bose products); a different iSerial per mode (the
instance ID becomes the serial, identical on every speaker: two speakers on one PC would collide); resetting the
cached format (a Windows setting changed behind the user's back, Windows only). macOS and Linux do not key anything
on interface numbers. [I] To check on hardware: the first enumeration of 0.5.0 on a port where the stock firmware
was bound (UAC1, usbaudio.sys, same `MI_00` node) must re-bind usbaudio2.sys (the hardware IDs change with
bcdDevice and the protocol 0x20); if Device Manager shows a failed "USB Audio Device", uninstall that device once.

Change (`re/docs/protocol_usb_hid.md` `30 MODE`, `uv run oc_tas.py mode 2.1`, the app): the TAS writes the byte at
0x17FF (verified), replies, and ~50 ms later re-enumerates (`main.c usb_restart()`: D+ pull-up off, stream stopped,
USB state reset, 200 ms, reconnect by `power_poll_usb()`, exactly the boot sequence). No reboot, no power cycle: the
DSP link, pod and servo keep running; the first stream of the new mode rewrites BPTSL (C-port stopped a few µs).

### Audio path (stock values, `audio/stream.c`)
- Clocks (stock InitClockRegs 0x1338): MCLKO1 = ACG1 = 24.576 MHz (codec-port master clock, servoed);
  MCLKO2 = ACG2 = 24.576 MHz fixed (DSP clock: manual "a separate 24.576 MHz clock is provided for driving the DSP").
- Codec port (stock InitAcg 0x11AB): CPTCNF1 0x38 (8 slots, GP mode), CPTCNF2 0x0D in mode 5.1 (= stock, 16 data
  bits in a 32-bit slot) or 0x2D in mode 2.1 (32 data bits), CPTCNF3 0x34 (BYOR = 1), CPTCNF4 0x61 (CSCLK =
  MCLKO1 / 2 = 12.288 MHz → 48 kHz × 256).
- DMA channel 0 → EP1 OUT, ISO circular buffer 960 bytes (stock size) for every format:
  | Mode | Channels → TDM slots | Subslot | Frame | DMATSH0 / DMATSL0 | BPTSL | OEPCNF1 | Packet 48 / 44.1 kHz |
  |---|---|---|---|---|---|---|---|
  | 5.1 | L R C LFE Ls Rs → 0..5 | 2 B, 16 bit | 12 B | 0x40 / 0x3F (= stock) | 16 | 0xCB (= stock) | 576 / 528–540 |
  | 2.1 | L R → 0, 1 | 4 B, 24 bit | 8 B | 0xC0 / 0x03 | 32 | 0xC7 | 384 / 352–360 |
  The DSP word is the sample left-justified in its 32-bit slot in both formats (16-bit: `b1 b0 00 00` as stock;
  24-bit: USB `[pad b0 b1 b2]` → BYOR → `[b2 b1 b0 pad]`, the DSP masks the pad byte): the DSP only changes its
  routing. Unused slots carry zeros. The first stream after a mode change rewrites BPTSL, which needs CPTEN = 0
  (DM 6.5.4): the codec port (CSCLK / CSYNC to the DSP) stops for a few µs.
- **44.1 kHz DMA re-alignment** (0.2.3, `audio/servo.c`): the check is "count not a multiple of the slot size"
  (2 bytes in mode 5.1, 4 in mode 2.1; 0.2.3: 4, stock: 12).
  At 44.1 kHz the SOF drifts through the C-port frame (44.1 frames per USB frame) and, when it falls between the
  slot 0 and slot 1 transfers, the latched count is legitimately 4 (mod 8); the stock-style check would then stop the
  DMA (BADDATA, flush) about every 10 ms. [I] Hypothesis for "no sound / input at 0 dBFS at 44.1 kHz" on 0.2.2:
  confirmed if, on 0.2.2 at 44.1 kHz, INFO "DMA re-alignments" climbs steadily (it stays 0 at 48 kHz).
- **Format history**: 0.2.0 used 3-byte subslots (DMA 3 bytes, BPTSL 24 in a 32-bit slot). First hardware test
  (2026-11-01, Windows, 24-bit 48 kHz music): DSP input RMS −62 dBFS with peaks −11 dBFS on both channels (crest
  factor ~50 dB), AUX normal, servo locked, no DMA re-alignment, i.e. the samples reached the DSP about one byte
  (48 dB) off in their slot. The TAS settings matched the Data Manual text, but the DM only illustrates 3-byte
  reversal for 20-bit data (figure 2-9) and 24-of-32 left-justification in I2S mode (figure 2-8), and the stock
  firmware never used 3 bytes. 0.2.1 uses 4-byte subslots so that DMA bytes = data bits = slot length (32): no
  justification is left to the C-port. Cost: 384 instead of 288 bytes per ms (49 × 8 = 392 max packet).
  0.3.0: 5.1 back as alt 1 in the stock 16-bit format, 2.1 24-bit added (alt 2), stereo 24-bit kept (alt 3).
  0.4.0: one format per user-selected audio mode (5.1 16-bit or stereo 24-bit routed 2.1); the 3-channel alt is gone.
- Servo (stock AdjustAcgFrequency 0x0813): every SOF the MCLK capture is compared with the nominal count,
  every 4 frames the ACG1 word gets a PI correction; DMA re-alignment with BADDATA (P1.7) when the buffer
  content is not a whole number of frames. Details and the differences with stock in `audio/servo.c`.

### 44.1 kHz — clock analysis (historical: 44.1 kHz removed in 0.5.1)
**0.5.1: 48 kHz only** (user decision 2026-11-02; the host resamples). Clock Source 5 is internal fixed (bmAttributes
0x05, frequency control read-only), RANGE = one subrange 48000, SET CUR stalls; the servo, `stream_set_rate()` and the
44.1 kHz constants are gone (−190 bytes of code including the guard below, data-area layout 8). `R` is still sent
(the layout byte), always with 48000.
**Level guard (0.5.1)**, hardware result 2026-11-02 with 0.5.0 at 44.1 kHz in mode 5.1: stereo content wandered
across the 6 TDM slots, ISO buffer 154..162 bytes (= 960 + ~155, wrapped), no re-alignment counted. Mechanism: with
more than ~605 bytes in the 960-byte ring at SOF (5.1; ~640 in 2.1), the next packet, written at up to 1.5 bytes/µs,
catches the DMA read pointer; the DMA sees "empty" with its read pointer in mid-frame and resumes at the next C-port
frame start, so every slot shifts; the whole-slot check cannot see it. The level drifted there because the servo ran
away at 44.1 kHz (offset 132 → 1146, phase error −575 in 10 s; not explained statically, path removed). The 0.5.0
interface renumbering does not touch mode 5.1 (AC 0, AS 1 as before; the rate request went to Clock Source 5 on the
AC interface). Now `dma_realign()` also re-aligns (BADDATA, DMA off 4 frames, count in INFO "DMA re-alignments")
when the count latched at SOF exceeds 600 bytes (normal: ~576..588 in 5.1, ~384 in 2.1), which restores the frame
alignment at both modes.

Evidence: all system clocks come from the TAS (manual "USB Controller Oscillator and Clock Generation");
the bit clock and frame sync are CSCLK/CSYNC of the codec port, derived from MCLKO1 (DM figure 2-1,
CPTCNF4.DIVB); the DSP runs from the separate, never-servoed MCLKO2 and its McASP1 is a clock **slave**
(`re/dsp/notes/hardware.md` §5); the ACG accepts 22.5792 MHz (DM table 2-3: `6A 4B 20`).
**Implemented on the TAS side**: SET_CUR 44100 → ACG1 = 22.5792 MHz → 11.2896 MHz bit clock, 44.1 kHz frames;
the servo uses the exact 22579/22580 cycles-per-frame pattern; the DSP is told with a new frame
`R [00 AC 44 L]` / `R [00 BB 80 L]` (both DSP firmwares answer `ERROR` to unknown commands, harmless).

**`R` frame (0.4.0)**: `R [rate, 24-bit big-endian Hz][layout]`, 4 payload bytes, sent when the host sets a rate,
on **every SET_INTERFACE of interface 1** (so also right after a mode change: the re-enumeration sets alt 0), and
once after each DSP handshake. Layout: **bit 0 = a stream is open, bit 1 = audio mode 2.1** — an explicit flag, so
the DSP knows the mode even while idle:
| L | Mode / stream | USB data in TDM slots 0..5 | What the DSP does (firmware/dsp, other agent) |
|---|---|---|---|
| 0 | 5.1, idle | zeros | 5.1 routing (as stock) |
| 1 | 5.1, streaming 16-bit | L R C LFE Ls Rs | 5.1 routing (as stock) |
| 2 | 2.1, idle | zeros | 2.1 routing |
| 3 | 2.1, streaming stereo 24-bit | L R 0 0 0 0 | **2.1 routing**: each satellite plays its channel on both drivers; the subwoofer gets the bass of L + R, plus the LFE slot if it ever carries data (it does not in this mode) |
Rule for the DSP: 2.1 routing ⇔ bit 1. (0.3.0 sent layouts 1 / 2 / 3 = alternate settings; 2 is now "idle in
2.1".) A DSP that reads only 3 bytes (current `i2c_slave.c`: `len >= 3`) still gets the rate. The DSP should treat
an unchanged rate as no rate change (no fade). After a mode change the TAS stops the C-port for a few µs to
rewrite BPTSL (DM 6.5.4), at the first SET_INTERFACE alt 1: McASP1 sees a short frame (possible sync error flag); the DSP should
tolerate it (re-sync / ignore one frame) and may mute briefly on `R`. [I] Whether the codec / AUX path shares
CSCLK / CSYNC (then AUX gets the same µs gap) is unverified.
[I] Assumption to verify (test step 6): the codec U4000 master clock is MCLKO1 (manual: "Only one clock
domain exists"). If it were MCLKO2, the codec's LRCK/MCLK ratio would be wrong at 44.1 kHz.

**What the DSP side must do** (TODO in `firmware/dsp`):
1. Handle `R`: store the rate (reply nothing, or `06`).
2. Keep PLL and McASP1 as they are (CPU clock = MCLKO2 unchanged; McASP is slave).
3. Recompute everything that depends on fs: all biquad coefficients (EQ, crossover, loudness, Bose
   corrections are designed for 48 kHz: at 44.1 kHz every corner frequency drops by 8.1 %), time constants of
   the volume smoothing, meters, limiter and dynamics, the tone generator frequencies, and the frame period
   (96 samples = 2.18 ms instead of 2 ms).
4. The AUX input (codec ADC) follows the same clock, so it runs at 44.1 kHz too while USB is at 44.1 kHz.

### Channel formats (0.3.0 study, still valid): what limits the throughput
The stock firmware streamed **5.1 at 16-bit, 48 kHz** (descriptor at 0x059F of `u6002_tas.bin`: IT 6 channels,
wChannelConfig 0x003F; FORMAT_TYPE 6 ch, 2-byte subframe, 16 bits; EP 0x01 wMaxPacketSize 576; InitAcg 0x11AB:
CPTCNF2 0x0D, DMATSH0 0x40, DMATSL0 0x3F; InitEndpointRegs 0x0E91: OEPCNF1 0xCB, 960-byte buffer).

**ISO buffer rule.** At each SOF the DMA starts moving the previous packet (DMABCNT0, DM 2.2.7.4.1, 6.5.2.4) at the
C-port rate *r* while the UBM writes the next packet at up to 12 Mb/s = 1.5 B/µs. With the packet at the start of
the frame, the circular buffer must hold `P + P·(1 − r/1.5) + 1 frame`; it must also stay below 1024 bytes
(OEPBSIZ bit 7: 1056 bytes failed on hardware, 2026-11-01), and the endpoint space has 1056 bytes left for it.
| 5.1 option (48 kHz) | Frame | Packet (max) | Buffer needed | Verdict |
|---|---|---|---|---|
| 24-bit, 4-byte subslots | 24 B | 1152 (1176) | — | **impossible**: over the 1023-byte full-speed iso maximum (USB 2.0 5.6.3) |
| 24-bit, 3-byte subslots | 18 B | 864 (882) | 864 + 366 + 18 = **1248** | **not feasible**: > 1016 (BSIZ < 0x80) and > 1056 free; 44.1 kHz: 810 max → 1209, same; and the 3-byte C-port path put the samples one byte off on hardware (0.2.0) |
| 20-bit | 18 B | same | 1248 | **same as 24-bit**: FMT-2 subslots are whole bytes, 20 bits still take 3 |
| **16-bit, 2-byte subslots** | 12 B | 576 (588) | 576 + 355 + 12 = **943** ≤ 960 | **chosen** (= stock; DM 6.4.2 sizes the space for "6 channels, 48 kHz … 16 bits per sample or 4 channels … 24 bits") |
| 2.1 24-bit, 4-byte subslots | 12 B | 576 (588) | 943 | feasible (0.3.0 alt 2); 0.4.0 streams stereo 24-bit (8 B) in mode 2.1 instead |
The ceiling is 12 bytes per frame (13 with a 1016-byte buffer, which no format uses). [I] A 49-frame packet (588)
arriving at full line rate right after SOF would need 962 bytes; with the clock locked to SOF the host sends 48
frames at 48 kHz (576, the stock wMaxPacketSize) and at most 45 at 44.1 kHz (540 → 901).

**Not the limit**: the bus (1023 B/frame), the C-port (8 × 32 bits × 48 kHz = 1.5 MB/s, slots 6..7 unused), the DMA
(it follows the C-port). The limit is the endpoint buffer: < 1024 bytes of circular buffer in a 1304-byte space.

**Hacks studied (research only, none implemented):**
| Idea | Finding | Verdict |
|---|---|---|
| Other XDATA / RAM as ISO buffer | XDATA = code RAM 0x0000..0x177F (program memory) + 0xFA10..0xFFFF; 0x1780..0xFA0F reserved (DM figure 6-2). Buffer base = 8-bit offset × 8 from 0xF800 (DM 6.4.2): the UBM / DMA only reach 0xF800..0xFFFF, of which only the 1304-byte space exists. Shrinking HID (2 × 64 → 2 × 8) and the 96-byte variable block gives at most 1016 (BSIZ < 0x80) | **No**: 1016 < 1248 |
| Smaller / more frequent DMA bursts | DMA moves BPTS bytes per slot at the C-port rate; DMABCNT0 is latched per SOF; full-speed iso is one packet per 1 ms frame (bInterval 1 is the minimum). The peak comes from the packet burst (864 B in ~0.6 ms), not from the DMA | **No lever** |
| Lighter packing (20-bit) | Subslots are 1..4 whole bytes (FMT-2 2.3.1.6); 20-bit = 3 bytes = 864 B/ms | **No gain** |
| Lossless / low-latency compression decoded by the 8052 | Standard 8052, 24 MHz, 12 clocks per machine cycle = 2 M cycles/s (DM: Timer = MCU clock / 12). 6 × 48 000 = 288 000 samples/s → **6.9 cycles per sample** at 100 % CPU; one MOVX takes 2. Copying 864 B/ms alone needs ~8 cycles/byte ≈ 6 900 cycles/ms against 2 000; a Rice / FLAC-style decoder needs ~30–50 cycles/sample (≈ 9–14 M cycles/s, 5–7× the CPU). The MCU also cannot feed the DMA path (UBM write pointer is hardware, "made available to the MCU for debug purposes", DM 2.2.7.4.1), and no OS sends compressed PCM over UAC2 (usbaudio2 Type III = IEC 61937 AC-3 / DTS / WMA bitstreams: lossy, decoding would fall on the DSP) | **Impossible** |
| Code overlays from the EEPROM to free RAM for buffers | Frees code RAM, which the UBM / DMA cannot address (program memory); and the code RAM is read-only at run time (DM 6.1) | **0 bytes of ISO buffer; overlays impossible** |

### Board behaviour (ported from stock, cited in the code)
| Function | Stock reference | Here |
|---|---|---|
| Power-up order: GLOBCTL 0x84, timers, USB, VREN, 40 ms, DSP reset released, SENSOR on | main 0x0B18 | `main.c`, `board/power.c` |
| DSPRESET\ = GLOBCTL.RESET (pin 41), VRAWON = GLOBCTL.VREN (pin 42) | 0x0B81, 0x0B8F | `power_audio_on` |
| DSP handshake: P1.6 released, wait until the DSP pulls it low, X/M/U/F, 50 ms, M | InitDspLink 0x0C60 | `dsp_link_start` + `dsp_link_poll`: never blocks, no DSP traffic until DSPINT is low; redone after every DSP restart ("DSP restarts" below) |
| P1.6 driven low while USB is active (here: also requires the configuration = "USB host present") | PollUsbVbusAndDspPower 0x0FD7 | `power_poll_usb` |
| USB presence P1.2 → D+ pull-up | 0x0FD7 | `power_poll_usb` |
| Pod mute P1.3: 1 ms sampling, 5 equal samples | DebounceMuteInput 0x13CD | `POD_DEBOUNCE_TICK` (pod.h) |
| Pod presence (mute line high after power-up) | DetectMuteLine 0x13FD | `pod_detect` |
| Pod mute → DSP `M` | SendPodMuteIfChanged 0x15A2 | `dsp_link_poll` |
| SENSOR = P1.5 (1 = off) | 0x0B96, EnterSuspend 0x125F | [I] see `board/power.c` |
| BADDATA = P1.7 | AdjustAcgFrequency 0x0931 | `servo.c` |

### Boot-time escape (safety net before any EEPROM flash)
"Mute held at power-on" cannot be detected: the pod mute is a touch sensor whose output **toggles** and always
starts *muted* when SENSOR is powered (manual "Sensor"), and the stock code detects the pod precisely by the line
going high. The escape is therefore: **after power-on, touch the pod mute 4 times within 8 s** (4 debounced
changes after the pod has been detected). The firmware then enters the ROM DFU in RAM mode (05A7:1020), exactly
like `REBOOT_DFU`, without any EEPROM write. It is checked from `main_idle()`, so it also works while the DSP
boots or if the DSP never answers. Without a pod there is no escape.

### USB Audio Class 2.0 (decided 2026-11-01, doc.md, decisions section)
Why: the pod knob must change **this speaker only**, and the OS slider must follow it. A HID consumer control acts
on the OS default output device (rejected). With UAC2 the device tells the host "Feature Unit CUR changed" through
the AudioControl interrupt endpoint; the host reads CUR and updates this device's slider. Status interrupts are not
documented for the Windows UAC1 driver `usbaudio.sys`.

**OS support of Feature Unit interrupts** (sources read 2026-11-01):
| OS | Support | Evidence |
|---|---|---|
| Windows 10 1703+ | **Yes, documented** | learn.microsoft.com "USB Audio 2.0 Drivers" (usbaudio2.sys, "Starting with Windows 10, release 1703"), table "Class requests and interrupt data messages": Feature Unit Mute = GET CUR, SET CUR, INTERRUPT; Volume = GET CUR, SET CUR, GET RANGE, INTERRUPT. Bound by compatible ID `USB\Class_01&SubClass_01&Prot_20` |
| Linux (snd-usb-audio) | **Yes, in the kernel source** | `sound/usb/mixer.c`: `snd_usb_mixer_status_create()` opens endpoint index 0 of the AC interface if it is interrupt IN (buffer = wMaxPacketSize); `snd_usb_mixer_interrupt()` walks 6-byte `uac2_interrupt_data_msg` records and drops vendor / endpoint ones; `snd_usb_mixer_interrupt_v2()` on `UAC2_CS_CUR` clears the cached value of that unit / control / channel and raises an ALSA control event (followed by PipeWire / PulseAudio) |
| macOS | **Unverified [I]** | Apple TN3190 "USB audio device design considerations" documents UAC2 (Clock Source frequency and validity, Feature Unit Mute and Volume on macOS, iOS, iPadOS) but says **nothing about the AC interrupt endpoint**; no Apple source found states that the Apple USB audio driver reacts to it. Audio and the slider → speaker direction are covered by TN3190; knob → slider must be checked on a Mac (test step 2c) |

**Full speed**: ADC-2 is speed-independent, and Apple TN3190: "ADC2 is designed to support all bus speeds of the
USB transport; Full Speed, High Speed and SuperSpeed". The Microsoft page states no speed restriction.

**Descriptors** (`data/ee_data.c`, stored in the U6002 data area; one configuration per audio mode, 200 / 184 bytes,
every length checked by `scripts/mkeedata.py`):
| Descriptor | Content | Spec / host requirement |
|---|---|---|
| Device | bcdUSB 2.00, class EF/02/01, EP0 8 bytes | IAD class triple (USB IAD ECN); device_qualifier stalled, full-speed only (USB 2.0 9.6.2) |
| IAD | interfaces 0..1, function class AUDIO, protocol 0x20 | ADC-2 4.6, table 4-3 |
| AC interface 0 | class 1 / subclass 1 / protocol 0x20, 1 endpoint | ADC-2 4.7.1 |
| AC header | bcdADC 2.00, category 0x01 (desktop speaker), wTotalLength 80 (5.1) / 64 (2.1) | ADC-2 table 4-5, A.7 |
| Clock Source 5 | bmAttributes 0x07 (internal programmable, synchronised to SOF: the ACG servo locks MCLKO1 to SOF), bmControls 0x03 (frequency read/write, no validity control) | ADC-2 table 4-6; usbaudio2.sys: one clock source, GET CUR and GET RANGE required; Linux treats a clock without validity control as valid (`clock.c`) |
| Input Terminal 1 | 0x0101 USB streaming, clock 5; 5.1: 6 channels 0x3F (FL FR FC LFE BL BR = stock wChannelConfig 0x003F); 2.1: 2 channels 0x03 | ADC-2 table 4-9 |
| Feature Unit 3 | 34 bytes (5.1) / 18 (2.1): bmaControls(0) = 0x0000000F (mute and volume, host programmable), channel entries = 0 | ADC-2 table 4-13; usbaudio2.sys ignores master controls when channel controls exist, so only the master has controls. **UAC2 has no per-control "interrupt" bit**: the bit pairs only mean absent / read-only / programmable; interrupt capability is the AC interrupt endpoint itself (ADC-2 4.8.2.1, 6.1) |
| Output Terminal 4 | 0x0301 speaker, source FU 3, clock 5 | ADC-2 table 4-10 |
| Interrupt EP 0x83 | interrupt IN, wMaxPacketSize 6, bInterval 4 ms, only endpoint of interface 0 | ADC-2 table 4-25 ("used here to pass 6-byte interrupt information"); Linux takes endpoint index 0 |
| AS interface 1 | alt 0 without endpoint, alt 1 with one; AS_GENERAL: terminal link 1, Type I, bmFormats = PCM only, cluster of the mode (6 / 0x3F or 2 / 0x03); FORMAT_TYPE_I: 2 / 16 (5.1) or 4 / 24 (2.1) | ADC-2 table 4-27 (bNrChannels / bmChannelConfig per alternate setting), FMT-2 table 2-2; usbaudio2.sys: alt 0 first and without endpoint, ascending, exactly one bmFormats bit, subslot 1..4, resolution 8..32, same bTerminalLink |
| Iso OUT EP 0x01 | bmAttributes 0x09 (isochronous, adaptive, data), 588 (5.1) / 392 (2.1) bytes (49 frames), 1 ms; EP_GENERAL without controls | USB 2.0 5.12.4.1; FMT-2 2.3.1.1 (nominal ± 1 frame); usbaudio2.sys supports "Adaptive IN and OUT"; it requires an explicit feedback endpoint only for **asynchronous** OUT |
| HID interface 2 | unchanged | — |

4-byte subslots for 24 bits (not 3): see "Audio path", format history. Buffer sizing: "Channel formats".

**What each OS shows** (0.4.0: one streaming format per enumeration, so no OS has a choice to make):
| OS | Mode 5.1 | Mode 2.1 |
|---|---|---|
| Windows (usbaudio2.sys) | Advanced: "16 bit, 44100 / 48000 Hz"; Configure speakers: 5.1 (stereo sources are played on FL / FR, the DSP routes as stock) | Advanced: "24 bit, 44100 / 48000 Hz"; Configure speakers: Stereo |
| Linux (snd-usb-audio) | one format S16_LE, 6 ch (FL FR FC LFE RL RR); PipeWire / PulseAudio "Analog Surround 5.1" | S32_LE (24 valid bits), 2 ch; "Analog Stereo" |
| macOS | 6 ch 16-bit Integer | 2 ch 24-bit Integer |
After a mode change the OS sees another device (PID 0x0001 ↔ 0x0002, bcdDevice 0x0401), with its own endpoint and
settings: on Windows the first switch to each mode installs usbaudio2.sys for that device node (a few seconds).

Adaptive, as in the stock design, rather than asynchronous: the codec-port clock MCLKO1 is servoed to SOF
(`audio/servo.c`, stock AdjustAcgFrequency 0x0813); the TAS never runs a free clock that the host would have to
follow, so there is nothing to feed back and no feedback endpoint (which would also need an IN buffer that the
1304-byte space does not have). Apple TN3190 describes this case as "Synchronous" ("must use the USB SOF to
discipline and lock the audio clock"); usbaudio2.sys accepts both for OUT, and the stock adaptive declaration is the
one proven with Windows.

**Class requests** (`usb/uac.c`, interface recipient, AC interface 0, ADC-2 5.2): Clock Source 5 Sampling Frequency
GET CUR (4 bytes), GET RANGE (26 bytes: two discrete subranges 44100 and 48000 with RES 0, non overlapping, as
usbaudio2.sys asks), SET CUR 44100 / 48000 (ACG1 reprogrammed, `R` to the DSP); Feature Unit 3 master Mute GET / SET
CUR, Volume GET / SET CUR and GET RANGE (8 bytes). Everything else stalls: this function has no endpoint-recipient
controls (EP_GENERAL bmControls = 0; the UAC1 endpoint sampling-frequency request has no UAC2 equivalent) and no AS
interface controls. Control reads longer than 8 bytes go through `usb_ep0_send_code` (several packets, ZLP when
needed); hosts read the first 2 bytes of a RANGE first (Linux `format.c`): the reply is truncated to wLength.
SET CUR volume: 0x8000 (silence, must be accepted, ADC-2 5.2.5.7.2) and anything below MIN is stored as MIN,
anything ≥ 0 as MAX.

**Interrupt data message** (`usb/uac_notify.c`, ADC-2 6.1, table 6-1): `00 01 00 02 00 03` = class-specific,
from an interface, attribute CUR, CS 2 (volume), CN 0, interface 0, entity 3; `00 01 00 01 00 03` for the mute.
The endpoint NAKs while nothing is pending ("When there are no interrupts pending, the interrupt endpoint must
NAK when polled"). Pending changes are one bit per control: a fast knob turn keeps one message in flight and the
host reads the latest CUR.

### Volume and mute: pod → speaker → OS (`board/pod_sync.c`)
The OS volume and mute are the only master volume; the pod knob and the pod mute set it, for this device only
(doc.md, decisions section; confirmed 2026-11-02: 0.4.4, which left the knob to the DSP as the stock firmware does, was
rejected).
- **Knob** (0.4.6 = the 0.4.3 path): the TAS polls `S` every 20 ms while a host is active. The DSP's byte 3 (DSP
  bld_j+: the **stock-filtered** knob, one-pole 0.05 / 0.95 every 2 ms frame, snap / hold hysteresis 0.25 / 0.75 %,
  1 % steps, as stock 0x100162B4 / 0x10016420) then goes through the TAS's backlash hysteresis (`knob_follow`:
  every 1 % while turning one way, 3 % to reverse). The TAS median of 3 (0.4.3..0.4.7) was dropped in 0.4.8 for the
  code space of the DSPINT fix, the DSP filter replacing it (as agreed). Then
  Feature Unit volume CUR = −`knob_db`[knob] dB (0.4.7, below), `V` to the DSP, interrupt; the host reads CUR and
  moves its slider. INFO byte 21 = knob (DSP byte 3), 22 = last applied, flags1 b1 = knob valid.
- **Knob law = the stock pod curve** (0.4.7). User feedback on 0.4.6: the first third of the travel changed almost
  nothing. The old law spread the knob linearly over the whole Feature Unit range (−127..0 dB, 1.28 dB per %): 0..33 %
  sat below −85 dB, where nothing is audible (the DSP floor is −80 dB). Now the stock DSP's pod volume curve
  (function 0x1001B140 with the parameters of object 0x10010564, read from the stock RAM image `re/dsp/out/rt.bin`;
  port: `firmware/dsp/src/control/volume_curve.c` `VC_STOCK_POD`): breakpoints 99 / 35 / 4 %, cumulative slopes
  0.3 / 1.1 / 6 dB per % (0.3 dB/% above 35 %, 1.4 between 4 and 35 %, 7.4 below 4 %), the stock "stop when the
  rest is < 1 %" rule, floor −71.3 dB; rounded to the nearest dB (Feature Unit grid, inside its −127..0 dB range).
  The 100 values live in the data area (`knob_db`, fixed address 0x1862; one EEPROM byte read per applied step; on an
  I²C error nothing changes and the next poll retries); `scripts/mkeedata.py` recomputes the curve and refuses a
  table that differs. Hysteresis and median unchanged (in knob %). Above 35 % one knob % is 0.3 dB, so several knob
  steps give the same whole dB (no change for the host).
  | Knob % | Stock curve (dB) | 0.4.7 Feature Unit (dB) | 0.2.x..0.4.6 (dB) |
  |---|---|---|---|
  | 0 | -71.3 | -71 | -127 |
  | 2 | -71.3 | -71 | -125 |
  | 3 | -70.0 | -70 | -123 |
  | 4 | -62.6 | -63 | -122 |
  | 5 | -61.2 | -61 | -121 |
  | 10 | -54.2 | -54 | -115 |
  | 15 | -47.2 | -47 | -108 |
  | 20 | -40.2 | -40 | -102 |
  | 25 | -33.2 | -33 | -96 |
  | 30 | -26.2 | -26 | -90 |
  | 33 | -22.0 | -22 | -85 |
  | 35 | -19.2 | -19 | -82 |
  | 40 | -17.7 | -18 | -76 |
  | 45 | -16.2 | -16 | -70 |
  | 50 | -14.7 | -15 | -64 |
  | 60 | -11.7 | -12 | -51 |
  | 70 | -8.7 | -9 | -38 |
  | 80 | -5.7 | -6 | -25 |
  | 90 | -2.7 | -3 | -13 |
  | 95 | -1.2 | -1 | -6 |
  | 99 | 0.0 | 0 | 0 |
- **What the stock firmware does** (static analysis, `u6002_tas.bin` = the user's backup, byte-identical), for
  reference: the TAS never reads the knob (`PollDspStatus` 0x1128 is the only `S` user, every ~2 s while USB is
  inactive, stand-by bytes only), the Feature Unit volume (IDATA 0x2F:0x30) comes only from SET_CUR (0x028B), and
  the AudioControl interface has no endpoint: nothing is reported to the host. The DSP applies the filtered knob as
  its own pod volume (0x100100FC, pod curve 0x10010564) in series with the USB volume (`re/dsp/notes/control.md`).
- **Mute**: at the next pass after each debounced pod mute change (≤ 20 ms; 0.2.2 waited 600 ms), if the pod state
  differs from the Feature Unit mute: mute CUR = pod state, `m` to the DSP, interrupt. **Plus one report of the
  current pod state when the host becomes active** (power-up, re-enumeration, resume; 0.4.6). Reason (hardware
  result 2026-11-02, after a TAS reboot: pod muted, Windows unmuted): every TAS power-up / reboot starts **muted, as
  stock** (SENSOR switched on at power-up, stock main 0x0B96, resets the touch sensor to "muted", service manual
  "Sensor"; InitDspLink sends `M` = raw P1.3, 0x0CEE), and only changes were reported. A host mute / unmute is left
  as the user set it (0.4.5 forced the OS back to muted while the pod was muted: removed in 0.4.6). A DSP-only
  restart does not reset the sensor: the TAS resends `M` and `m` at the new handshake. A double press (DSP profile
  cycling) mutes then unmutes the OS too, net no change. The pod state is the TAS's own debounced P1.3; the `S`
  flag bit 1 is only the echo of the `M` frame the TAS sent. INFO bytes 23 / 24 time-stamp (256 ms units) the
  last interrupt message and the host's next Feature Unit request: everything beyond that is the host.
- **Knob mute (0.5.4)**: when the applied knob value reaches 0 % the TAS also sets the Feature Unit mute (`m` 1,
  AC interrupt), unless the OS was already muted; leaving 0 % (≥ 2 %; in practice the first step up, 3 % as a
  reversal) clears it only if the knob set it ("muted by knob" flag) and the pod is not muted. Evaluated only when the
  applied knob value changes, so an OS unmute while the knob rests at 0 % stays. The volume still follows the knob
  law (−71 dB at 0 %). Flag cleared when the host goes away.
  | Knob → | pod unmuted, OS unmuted | pod muted (OS muted by pod sync) | OS muted by the user |
  |---|---|---|---|
  | reaches 0 % | OS muted, flag set | nothing (already muted) | nothing (already muted) |
  | leaves 0 % (flag set) | OS unmuted | stays muted, flag cleared | — |
  | leaves 0 % (no flag) | — | stays muted | stays muted |
  A pod touch while knob-muted follows the usual pod sync (a pod unmute unmutes the OS).
- **No pod (0.5.2)**: hardware result 2026-11-02 with 0.5.0, pod unplugged: the app showed a pod with a 38 % knob,
  and that floating knob was pushed to the OS volume. Stock (static analysis): `DetectMuteLine` 0x13FD runs once from
  `InitDspLink` (0x0C64), 40 ms after SENSOR is switched on (main 0x0B96): pod present if P1.3 goes high within its
  poll loop (bit 0x08); the pod is never re-detected afterwards. Without a pod the stock TAS sends `M 0`, `X 0`, and
  the stock DSP forces the pod volume to 99 % (`X` handler, `re/dsp/notes/control.md` §3): only the USB volume acts.
  Ours detects the same way (`board/pod.c pod_detect`, same 40 ms + ~1 s window), but the knob push only checked the
  DSP's "knob valid" flag. Since 0.5.2 the TAS's own `pod_present` also gates it: no pod → no knob → OS volume, no
  pod-mute report, INFO flags b6 = 0 and knob_ok = 0 whatever the DSP says. The `S` poll keeps running (DSP restart
  detection). As in stock, a pod plugged or unplugged after power-up is not seen until the next power cycle.
- **Pod handling vs stock (audit 0.5.3)**, `u6002_tas.bin`:
  | Item | Stock | Ours | Status |
  |---|---|---|---|
  | SENSOR (P1.5, 1 = off) | reset value (off), on at main 0x0B96 after VREN, 40 ms, DSP reset release | off again in `power_early_init` (P1.5 = 1), on in `power_audio_on` in the same order | same; the explicit "off" is deliberate (reload without power cycle, 0.2.3) |
  | Presence | `DetectMuteLine` 0x13FD from `InitDspLink` 0x0C64: 40 ms, then P1.3 polled (≤ 50 000 loops) for a high level, once | `pod_detect`: 40 ms, then P1.3 every 4 ms for 1 s, once at cold boot | same rule |
  | Mute debounce | `DebounceMuteInput` 0x13CD from the 1 ms Timer1 ISR (0x0C42): 5 equal samples, then change flag | `POD_DEBOUNCE_TICK` in the 1 ms Timer1 ISR, 5 equal samples | same |
  | `M` at handshake | raw P1.3, 50 ms after the link is up (0x0CEE) | same (`link_finish`) | same |
  | `M` on change | `SendPodMuteIfChanged` 0x15A2, not while the link starts | `dsp_link_poll`, only with the link up | same |
  | No pod | `M 0`, `X 0`; DSP forces pod volume 99 % | same frames; no knob push, no pod-mute report (0.5.2) | knob push is ours |
  | Knob / IVOL | never read by the TAS (DSP ramp converter + filter, pod volume curve in the DSP) | DSP `S` byte 3 → OS volume (user design) | deliberate difference |
  | Stand-by SENSOR pulse | `EnterSuspend` 0x127B / 0x1284 (off, 80 ms, on), wake on unmute 0x10A6 | not implemented (TODO) | — |
  | Re-plug | never re-detected | 0.5.3: a high P1.3 with no pod → present, `X 01` | ours (safe: only a powered pod can drive it high) |
  | Hot-unplug | not detected | not detectable on P1.3 (absent = unmuted = low); relies on the DSP clearing "knob valid" when IVOL floats | DSP side |
  No difference in the TAS touches the knob value itself: its jitter comes from the DSP's IVOL conversion.
- **Pod detection** (stock DetectMuteLine 0x13FD): the line must go high within 1 s after the pod's SENSOR supply
  comes on (the sensor powers up "muted"). `power_early_init()` now turns SENSOR off first (P1.5 = 1, its reset
  value): after a reload through the ROM DFU without power cycle (0.2.2) the sensor stayed powered, kept its
  unmuted state, `X 0` told the DSP "no pod" and the knob was never reported. `X` is resent at every handshake.
- **History of the knob → OS volume path** (0.4.4 removed it, 0.4.6 restored it). Measurements (app closed, knob untouched,
  `oc_tas.py info` every 0.6–0.7 s):
  | Firmware | Rule | Knob readings | Last applied | Volume |
  |---|---|---|---|---|
  | 0.4.1 | apply any change ≥ 2 % | 80 79 79 79 78 78 78 79 | 80 → 78 → 80 | −25 ↔ −28 dB |
  | 0.4.2 | backlash, reversal ≥ 3 % | 78 79 78 80 79 78 78 (span 77..80) | 77 → 80 → 77 | −29 ↔ −25 dB |
  | 0.4.3 | median of 3 + backlash | — | stable at rest | still audible steps mid-travel (user) |
  | 0.4.6 | DSP stock filter + median of 3 + backlash | to measure (test 8a') | | |
  Under 0.4.2 "last applied" sat at 77 while most readings were 78 / 79: after a move down every further 1 % down
  was applied at once, so a single low outlier became the applied value at the bottom of the noise span, and a
  single high outlier (80) was then exactly 3 % away, a reversal. 0.4.6 relies on the DSP's stock filter for a
  clean knob value (bld_j), the TAS guard staying as a second line.
- **USB host present / absent → DSP**: the existing DSPINT level (P1.6), no new I²C frame. After `U`, the TAS
  drives it low while a host is active; the OpenCompanion DSP switches to HOST mode (OS volume) when low and to
  LOCAL mode (knob applied by the DSP) after 50 ms high (`firmware/dsp/include/volume.h`, dsp/README "Volume, pod
  and the TAS protocol"); the stock DSP uses the same pin (0x10015700). "Host active" also requires the USB
  configuration (`power_poll_usb`), so USB 5 V without a host keeps the knob local.
- **"No pod = 99 %"** is dropped on the DSP side; the TAS still sends the stock `M 0`, `X 0` at start-up without a
  pod. Without a pod nothing is reported: the DSP reports the knob as not valid.
- **Open points** ([I]):
  1. Headphones: the pod drives the same MUTE line while headphones are plugged in (service manual "Headphone
     Amplifiers"), so plugging headphones also mutes the OS (accepted by the user).
  2. Volume law: with the DSP's plain dB law (profiles without `PF_VOL_STOCK`) the knob is linear in dB over
     −127..0 dB, i.e. quiet over its lower half (the DSP clamps at −80 dB); with `PF_VOL_STOCK` the dB shown by the
     OS is not the gain applied (already the case with UAC1). Making both agree is a DSP volume-law decision.
  3. macOS reaction to the interrupt (above).
  4. 0x8000 (−∞) from a host is stored as MIN (−127 dB), which the DSP dB law plays at −80 dB, not silence.

**What the DSP side must provide** (all present in `firmware/dsp` at the time of writing: `include/volume.h`,
`src/control/volume.c`, `src/hw/i2c_slave.c`):
1. `S` reply in **one frame**: `[standby][fault][flags][knob][events]`, flags bit 7 = 1 (extended), bit 3 = knob
   valid (pod present and ramp converter settled), knob 0..99 % **after the stock filter** (one-pole 0.05 / 0.95
   per 2 ms frame, snap / hold 0.25 / 0.75 %, 1 % steps; bld_j). The TAS reads it 4 ms after the request; a later
   reply is simply used by the next poll (20 ms).
2. HOST / LOCAL from DSPINT as above; in HOST mode the knob must not change the gain (the TAS maps it onto `V`);
   in LOCAL mode the knob is the master volume. Avoid a gain jump when the mode changes (ramp).
3. `M` keeps its meaning (speaker mute + headphone EQ, both modes); `m` = OS mute; `V` = OS volume.
4. No "99 % without pod".
5. `R` is sent again after each handshake (done: `dsp_link_start` sets the rate flag); since 0.3.0 it carries the
   layout (4th byte, "R frame" above); since 0.4.0 bit 1 = audio mode 2.1, and the routing must follow it.
The TAS ignores bytes 0, 1 (stand-by is not implemented) and 4 (it compares positions, not events), and flag
bit 1 (it reads the pod mute itself, P1.3).

### DSP restarts (`board/dsp_link.c`)
boot1 in U7001 (never rewritten in the field) jumps to garbage when the TAS reads a reply frame from 0x29 during its
~0.1 s boot window (`firmware/dsp/boot/docs/RESCUE.md`, boot1 `cmd.c link_on_reply_read`). `oc install dsp`
restarts the DSP several times (RSC / UPD from our DSP firmware, REBOOT from rescue or the updater), and the TAS
polls `S` every 20 ms. So when the DSP restarts, the TAS stops **all** DSP traffic ('S' polls, 'V' 'm' 'M' 'R', the
HID relay answers status 04), releases DSPINT and waits, exactly as at cold boot, until the DSP pulls DSPINT low;
then it redoes X / U / F / M and resends 'R', 'V' and 'm'. Every image after a restart pulls DSPINT low: our DSP
firmware on a cold boot (`mcasp0_gpio.c`), rescue mode and the updater (`rescue_run → dspint_ready`); each releases
it on 'U'.
**Wait timeout (0.4.8), made safe for the loaders (0.4.9).** Hardware result 2026-11-02 with 0.4.7: after a speaker
restart the TAS kept DSPINT released while the host streamed, so the DSP fell to LOCAL mode and muted the USB channels
(stock rule) until the DSP was reset. Static analysis: the stock `PollUsbVbusAndDspPower` 0x0FD7 drives P1.6 low
whenever USB is present and not suspended, except during the cold-boot `InitDspLink` (bit 0x04); ours also releases
it while `dsp_link_busy`, i.e. after every link-down, until the DSP requests the handshake. A DSP that runs but does
not request it (warm start by JUMP, a false 3-NACK trigger) left the TAS waiting forever. Rules since 0.4.9 (DSP agent
review: the boot1 rescue / updater link does not keep the current message's command, so a frame injected inside a
multi-frame `E` message makes its last frame run alone, data bytes as opcode; and a `U` releases the loader's DSPINT
for good):
- **Request** = DSPINT low *after it was seen released* since the link-down (a loader that never got `U` keeps it low:
  a level would re-trigger at once while the DSP reboots).
- **Timeout** = no request for `LINK_WAIT_256MS` × 256 ms ≈ 30 s. The HID relay is refused while the link is down
  (status 04), so this is also ≥ 30 s after the last relayed message.
- **Probe first**: the handshake starts with `E 00` (loader INFO, one frame, reply read at once). A rescue / updater
  answers `06 'O' 'C' 'L'`: the link comes up for the relay but **no X / U / F / M is sent** (never `U` to a loader).
  Our firmware and the stock one answer their console error: normal handshake. No acknowledge: link down, wait again.
- **Atomic relay**: `dsp_transact()` writes all frames of a message and reads its reply without returning to the
  main loop, the only place that sends other DSP frames (`main_idle()` never does), so no frame is ever inserted
  inside a relayed message or before its reply is read.
- The probe's reply uses the vendor HID IN buffer: only while it is free.
Also (0.4.8, kept): every SOF clears `usb_suspended`. Code space came from `dsp_transact()` in assembly (−120 bytes,
same behaviour step for step) and a compact `ee_write()` range test; 0.4.8 also dropped the TAS knob median.
**Reply matching (0.4.10).** Hardware result 2026-11-02, TAS 0.4.9 with the stock DSP restored: every relayed console
command returned `[S] 00 00`. The stock TAS reads a reply once, 15 / 20 ms after its request (`DspTransact` 0x0425,
delay table DATA 0x0D), and never polls `S` while USB is active, so it never meets a stale reply. Ours polls `S` every
20 ms and reads 4 ms later; the stock DSP answers more slowly, so its `S` reply was still pending when the relayed `E`
was read. Now the first frame of a reply must carry the request's command byte; otherwise it is re-read every 10 ms,
~200 ms in all, then status 11 (`DSP_ERR_READ`): a reply of another command is never returned. No `S` poll or probe
can come between a relayed request and its reply (atomic relay, above).
| Trigger | Why it is enough |
|---|---|
| 3 I²C transfers in a row without acknowledge | every reboot (RST, RSC, UPD, REBOOT, watchdog, crash) goes through the ROM boot loader and boot0 / trampoline, during which 0x29 does not acknowledge, before boot1 opens its window (RESCUE.md "Rescue on demand"); at one poll per 20 ms the link is down after ~60 ms |
| relayed `E 06` (loader / rescue / updater REBOOT) acknowledged with `06` | taken down at once (boot1 reboots only after that reply has been read) |
Not triggers: JUMP (`E 03`) does not go through boot1; the updater it starts pulls DSPINT low and answers 'S' anyway,
and a DSP firmware started by JUMP from RAM keeps its warm DSPINT state and would never redo the handshake.
Matching the console texts RST / RSC / UPD in the relay as well cost 118 bytes and did not fit; the NACK trigger
covers them. DSPINT cannot reveal a restart while the link is up (the DSP released it on 'U' and the TAS drives it low
while USB is active). INFO byte 5 b5 = waiting for DSPINT. Host side: during the wait every relayed command gets
status 04, which `oc-install` (`dsp/enter.rs wait_runtime`, `runtime.rs detect`) treats as "no answer yet" and
retries (10 to 20 tries of ~1.4 s). Recovery if a running DSP ever NACKs 3 times in a row (link down for good):
`oc_tas.py reboot` or `dfu` (both reset the DSP), or a power cycle.

### Not implemented (TODO)
- **Stand-by** (USB inactive and AUX < −70 dB for 2 min): does not fit. Port ready from stock: every 2 s while
  USB is inactive send `S` (reply after 20 ms); reply byte 0 ≠ 0 → stand-by, byte 1 ≠ 0 with reply cmd `S`
  → stand-by locked (PollDspStatus 0x1128); enter = DSP reset, MCLKO2 and codec port off, VREN off, SENSOR
  pulse 80 ms (EnterSuspend 0x125F); leave when USB is present or the pod is present and unmuted
  (ResumeFromStandby 0x109A). The level detection itself is done by the DSP. Our DSP never asks for it today.
- Memory peek command.

## Deploying 0.5.0 (identity 05A7:1020, data-area layout 7)
**From 0.4.x through the app or `oc install tas`** (our firmware's EE session, then a ROM reboot): the app finds the
0.4.x speaker on 1209:0001 / 0002 (still accepted as "ours, old"), writes the code, the data area (layout 7) and the
final header (firmware-version byte 0x50). After the ROM reboot the speaker comes back as **05A7:1020, bcdDevice
0x0500, "Companion 5 (OpenCompanion)"**; the app and `oc_tas.py` find it by that rule, the stock-firmware path
(`transport::Companion`) ignores it. The mode byte 0x17FF is kept. The app and tools must be built from this tree.
```
make PYTHON="py -3"                     (oc_tas.dfu: header firmware version 0x50; oc_tas_data.bin: layout 7)
oc install tas                          (or the app's update flow)
uv run oc_tas.py info                   (firmware 0.5.0, protocol 4)
uv run oc_tas.py mode 2.1               (writes 0x17FF; the speaker re-enumerates, still 05A7:1020, audio on MI_01)
```
**RAM test over an installed firmware** (no EEPROM code change): `oc tas data write` (through our firmware: EE
session, data area only, header closed as the stock Bose header DataType 1), `uv run oc_tas.py dfu`, then in an
elevated PowerShell `tools\tas_dfu_run.ps1 opencompanion\firmware\usb\build\oc_tas\oc_tas.dfu`. Do not unplug or
re-enumerate between `data write` and `dfu` (the running firmware would serve the new layout with its old code). A
firmware only enumerates with its own data-area layout (0.4.1 = 5).

## DFU workflow (RAM only)
The firmware needs the U6002 data area (above). Write it once per layout change, **while the stock Bose firmware
runs** (header DataType 1), then switch to DataType 2 (ROM boots in DFU, see "Developing without desoldering"):
```
make PYTHON="py -3"                                   (firmware/usb: oc_tas.dfu + oc_tas_data.bin)
oc tas data info                                      (checks the blob: layout, length, CRC-32)
oc tas data write --dry-run                           (reads 0x1800..0x1FFF, lists the pages to write)
oc tas data write                                     (type WRITE: 0x1800..0x19E7 only, each page read back)
oc tas mode dfu --yes                                 (header DataType 2; power-cycle the speaker)
```
(`oc` = `opencompanion/app`, `cargo run -p oc-cli --`.) `oc tas data write` never writes below 0x1800: header and
stock code stay intact, the stock firmware still boots with DataType 1 (it ignores the area). Then:
```
powershell -ExecutionPolicy Bypass -File tools\tas_dfu_run.ps1 opencompanion\firmware\usb\build\oc_tas\oc_tas.dfu
```
`tools/tas_dfu_run.ps1` still matches: it looks for the DFU device 05A7:1020 (also the identity announced by
`REBOOT_DFU` and the boot escape) and waits for `VID_1209` after the bus reset; pass the image path (its default
is `build/oc_tas_test.dfu`). Host client: `tools/oc_tas.py` (uv).

## Hardware test plan (RAM only, never flash)
Before each audio step: **OS volume at 10 % or less, pod knob fully down, nothing playing until the step says
so**. The knob is now **absolute**: once reporting runs, the first knob move sets the OS volume to the knob
position, so keep the knob low and move it slowly. The DSP ceiling stays at −12 dB. Only step 0 writes the
EEPROM, and only the data area 0x1800..0x19E7.
Test tone file (−60 dBFS, 1 kHz, 24-bit stereo, 10 s):
```
py -3 -c "import wave,struct,math;r=48000;w=wave.open('tone60_48k.wav','wb');w.setparams((2,3,r,0,'NONE',''));a=round(0.001*8388607);w.writeframes(b''.join(struct.pack('<i',int(a*math.sin(2*math.pi*1000*i/r)))[:3]*2 for i in range(r*10)))"
```
(same with `r=44100` → `tone60_44k.wav`). For music use a file normalised to −40 dBFS peak or less.

0. **Prerequisite**: data area written while the stock firmware runs (`oc tas data write`, "DFU workflow"), then
   header DataType 2. Power-cycle; Device Manager must show 05A7:1020 (ROM DFU). If after loading `oc_tas.dfu` no
   1209:0001 device appears, the data area is missing or invalid (the firmware stays off USB by design).
1. **Shared USB core regression** (silent: DSP in reset): `tas_dfu_run.ps1` (default image
   `build\oc_tas_test.dfu`), then `uv run tas_romdump.py --info` → `OC-TAS-TEST v0.1`. Power-cycle afterwards.
   Run this first: `common/usb_core.c` now passes class control reads longer than 8 bytes to the application
   (the test firmware stalls them as before).
2. **Enumeration, Windows** (10 1703 or later): `tas_dfu_run.ps1 ...\oc_tas.dfu`. Expect "Companion 5 dev" under
   *Sound, video and game controllers*, driver **usbaudio2.sys** (Properties → Driver → Driver Details), plus one
   vendor-defined HID device; no "HID-compliant consumer control device". Sound settings → speaker → Advanced: in
   mode 5.1 only "16 bit, 44100 / 48000 Hz" and Configure speakers 5.1; after `oc_tas.py mode 2.1` only "24 bit,
   44100 / 48000 Hz" and Stereo (step 13). If the device shows a yellow mark: Event Viewer →
   Microsoft-Windows-USB-USBHUB3 / usbaudio2 events, and report them (descriptor issue). `uv run oc_tas.py info`:
   flags `usb_present usb_active configured dsp_link_up pod_present`, firmware 0.4.9, protocol 4.
   Pod LEDs: green (VREN) and red (SENSOR) as with the stock firmware.
2b. **Enumeration, Linux** (the firmware stays in RAM while the speaker stays powered: after step 2 move the USB
   cable to the Linux machine; the TAS re-enumerates on the bus reset): `lsusb -v -d 1209:0001` parses the
   descriptors without "invalid descriptor" warnings and shows `bInterfaceProtocol 32` and the interrupt endpoint
   0x83; `dmesg` without snd-usb-audio errors; `cat /proc/asound/cards` lists "Companion 5 dev";
   `amixer -c <card> contents` shows a playback volume of −127.00..0.00 dB and a switch; `cat
   /proc/asound/card<n>/stream0` lists one format: mode 5.1 S16_LE 6 ch (FL FR FC LFE RL RR), mode 2.1 S32_LE
   2 ch, at 44100 and 48000 Hz. Then `alsactl monitor` (or `amixer -c <card>
   events`) and turn the knob a little: one event per change, and `amixer` shows the new value.
2c. **Enumeration, macOS** (same cable move): System Information → USB shows the device; Audio MIDI Setup shows
   "Companion 5 dev"; its Format menu should list 6 ch 16-bit, 3 ch 24-bit and 2 ch 24-bit at 44.1 / 48 kHz
   (report the list). Turn the knob a little: **check whether the macOS volume slider
   follows** (not documented by Apple, README "USB Audio Class 2.0"); report the result. The slider and mute must
   in any case control the speaker (macOS → device direction).
   Move the cable back to Windows for the following steps.
3. **EEPROM read**: `uv run oc_tas.py ee-read 0 18` = the current header (`6a 12 12 34 ...`).
4. **HID relay to the DSP**: `uv run oc_tas.py console SVM` → `D01.01.01UOC-0.4.9` (stock DSP: proves the
   `F` frame); `uv run oc_tas.py dsp P` → `ALIVE`.
5. **Volume / mute PC → speaker** (nothing playing): move the Windows slider and mute → `info` shows the
   volume in dB (whole dB, −127..0) and `usb_mute`. Then step 6 checks it audibly.
6a. **Sample format check, mode 2.1 = stereo 24-bit (first, nothing audible needed)**: `oc_tas.py mode 2.1`;
   volume at 10 %, play a 1 kHz sine at −40 dBFS, 24 bit, 48 kHz, on both channels (`tone40_48k.wav` below); `info`
   must show "alt 1"; while it plays run
   `uv run oc_tas.py console LVI` and `... console LRI` (DSP input peak / RMS, `firmware/dsp/README`). Expected
   `info` "ISO buffer" ≈ 384 bytes (one packet at 48 kHz; 0.2.1 with the 1056-byte buffer read 36).
   USB L and R: peak −40 ± 1 dBFS, RMS −43 ± 1 dBFS (sine: RMS = peak − 3 dB), the other USB slots −99, AUX
   unchanged. Diagnosis if not: RMS ≈ peak − 3 but 48 dB low = samples one byte off in the slot; RMS close to
   0 dBFS (loud noise) = byte order; a large crest factor (RMS far below peak − 3) = packing / alignment; one
   channel −99 = L/R slot shift. Then the same with `tone40L_48k.wav` (left only): R must read −99.
   ```
   py -3 -c "import wave,struct,math;r=48000;w=wave.open('tone40_48k.wav','wb');w.setparams((2,3,r,0,'NONE',''));a=round(0.01*8388607);w.writeframes(b''.join(struct.pack('<i',int(a*math.sin(2*math.pi*1000*i/r)))[:3]*2 for i in range(r*10)))"
   py -3 -c "import wave,struct,math;r=48000;w=wave.open('tone40L_48k.wav','wb');w.setparams((2,3,r,0,'NONE',''));a=round(0.01*8388607);w.writeframes(b''.join(struct.pack('<i',int(a*math.sin(2*math.pi*1000*i/r)))[:3]+b'\0\0\0' for i in range(r*10)))"
   ```
   Windows itself must not process the stream: speaker Properties → Advanced → disable audio enhancements, and
   play with an application volume of 100 % (only the device slider at 10 %, which is the hardware volume).
6b. **Mode 5.1 = 5.1 16-bit** (nothing audible needed, volume 10 %): `oc_tas.py mode 5.1`, "Configure speakers" =
   5.1; play a 6-channel −40 dBFS file where each channel has its own frequency (below). `info`: "alt 1", "ISO
   buffer" ≈ 576, DMA re-alignments not increasing. `console LVI` / `LRI`: USB L R C LFE Ls Rs each
   peak −40 ± 1 dBFS (16-bit: same scale as 24-bit), RMS −43 ± 1; the `console` frequency check (or a one-channel
   file) confirms the order L R C LFE Ls Rs.
   ```
   py -3 -c "import wave,struct,math;r=48000;f=[500,700,1000,60,1300,1700];a=round(0.01*32767);w=wave.open('tone40_51_48k.wav','wb');w.setparams((6,2,r,0,'NONE',''));w.writeframes(b''.join(struct.pack('<6h',*[int(a*math.sin(2*math.pi*x*i/r)) for x in f]) for i in range(r*10)))"
   ```
6. **Audio 48 kHz** (mode 2.1): Sound settings → speaker → Advanced: 24 bit, 48000 Hz. Play `tone60_48k.wav`
   → faint 1 kHz tone. `info` every few seconds: alt 1, 48000 Hz, servo offset stable (a few units),
   phase error small, DMA re-alignments not increasing. Change the Windows volume and mute during playback.
   Then music at −40 dBFS for 2 min: no clicks. Also check the AUX input still plays.
7. **Audio 44.1 kHz**: set 24 bit, 44100 Hz, play `tone60_44k.wav` (then repeat 6b at 16 bit 44100 Hz: "ISO
   buffer" 528 / 540, no DMA re-alignment storm): `info` shows 44100 Hz and an ACG1 near
   22.5792 MHz. Pitch must be the same as in step 6 (1 kHz). With the stock DSP the EQ is ~8 % off (designed
   for 48 kHz): expected. If there is no sound or a distorted one, the codec is not clocked by MCLKO1 ([I]):
   go back to 48 kHz and report. Check AUX at 44.1 kHz too.
8. **Mute pod → speaker**: touch the pod mute: `info` `pod_mute` toggles and the speaker mutes / unmutes.

Pod → OS (UAC2 interrupt). Works whether or not the speaker is the Windows default device. Start every step with
**nothing playing, the knob fully down, Windows volume at 10 %**, and turn the knob slowly by small amounts.
8a. **Knob read**: `uv run oc_tas.py info` → flags `knob_ok`, `pod knob N %` following the knob (0..99), and "last
    applied" equal to it once the knob moved. Also `uv run oc_tas.py dsp S --delay 20` → reply `S` + 5 bytes, byte 2
    has bit 7 set.
8a'. **Knob at rest (0.4.6 + DSP bld_j)**: app closed, knob untouched mid-travel for 60 s, `uv run oc_tas.py info`
    every 0.6 s: "pod knob" constant (DSP filter), "last applied" and the volume constant. Then turn slowly up: the
    volume rises by 1–2 dB per knob %, without waiting; stop, then turn down: the first change after 3 %, then every 1 %.
8b. **Knob → Windows** (nothing playing): make another device the Windows default first. Turn the knob up a few
    millimetres → the slider **of this speaker** rises, the default device's volume does not change; `info` volume =
    the 0.4.7 table above (stock pod curve). Down → falls. Move this speaker's slider with the mouse → the knob does nothing and
    the slider stays; the next knob move sets the slider back to the knob position (absolute knob, expected).
8c0. **Start muted (0.4.6)**: power-cycle the speaker (or `uv run oc_tas.py reboot`): the pod comes back muted (stock
    behaviour), and within ~1 s of enumeration Windows shows this speaker **muted** (`info` `usb_mute`). Touch the pod
    → both unmuted. Unmute in Windows while the pod is muted → Windows stays unmuted (left as the user set it; the
    speaker stays silent until the pod is touched).
8c. **Pod mute → Windows**: one touch → this speaker shows muted, `info` `usb_mute`; touch again → unmuted.
    `info` "host ... reaction" = time from our interrupt to Windows' GET CUR (256 ms resolution); our own part
    is ≤ 25 ms. Double touch → the DSP changes profile, Windows mutes and unmutes (net no change). Mute in Windows,
    then touch the pod → pod and Windows end up consistent, no repeated toggling.
8d. **Headphones** (accepted behaviour): plug headphones into the pod at low volume → Windows shows this speaker
    muted; note whether the headphones play after unmuting in Windows.
8e. **No host → local knob**: with a tone playing on AUX at low level, put the PC to sleep (USB suspend) or unplug
    USB: after ~50 ms DSPINT is high and the knob changes the AUX volume directly (DSP LOCAL mode; judge by ear).
    Wake / replug: `console POD` shows `MD:H`, the Windows volume is the master again, and the knob moves made
    meanwhile do not change it until the knob moves again.
9. **DFU re-entry**: `uv run oc_tas.py dfu` → 05A7:1020 appears within ~1 s (no power cycle);
   `tas_dfu_run.ps1 ...\oc_tas.dfu` again: the download must finish without waiting on a long poll timeout
   (MANIFEST = 256 ms), and 1209:0001 comes back.
10. **ROM reboot**: `uv run oc_tas.py reboot` → with DataType 2 the ROM comes back in DFU (05A7:1020).
11. **Boot escape**: run `tas_dfu_run.ps1 ...\oc_tas.dfu` and, as soon as the pod's red LED lights, touch the mute
    pad 4 times within 8 s → 05A7:1020 instead of 1209:0001. Negative check: 2 touches → stays 1209:0001.
12. **DSP restart through our TAS** (nothing playing): `uv run oc_tas.py info` → `dsp_link_up`. Then
    `uv run oc_tas.py console RSC` (DSP reboots into rescue): within ~0.1 s `info` shows `dsp_wait_dspint`, no
    `dsp_link_up`, and `oc_tas.py dsp P` answers status 04; a few seconds later `dsp_link_up` again and
    `oc_tas.py dsp E --data 00` (loader INFO) answers `06 'OCL' ... 'R'` (rescue). Then `oc_tas.py dsp E --data 06` (REBOOT):
    the link goes down at once, comes back with the DSP firmware (`console SVM`), and `console POD` shows `MD:H`
    with the Windows volume restored (`V` resent). Finally `oc install dsp --dry-run` end to end.

13. **Audio mode switching** (nothing playing, volume 10 %; the app and `oc_tas.py` from this tree):
    `uv run oc_tas.py mode` → 5.1; Device Manager shows `USB\VID_1209&PID_0001`. `uv run oc_tas.py mode 2.1`: the
    speaker leaves the bus and comes back as **1209:0002** (first time: Windows installs usbaudio2.sys for the new
    device node, a few seconds); a second "Speakers (Companion 5 dev)" endpoint exists (Sound settings; the 5.1 one
    is now "not present"), Advanced offers only "24 bit, 44100 / 48000 Hz". Read only, PowerShell:
    `Get-PnpDevice | ? InstanceId -like '*VID_1209*'` (PID_0002 OK, PID_0001 Unknown), and under
    `HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\<endpoint>\Properties` the value
    `{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0` of the new endpoint = 2 ch, 24 bits (bytes 2..3 = `02 00`, 14..15
    = `20 00` or `18 00`, mask `03`). `uv run oc_tas.py mode` → 2.1, `ee-read 0x17ff 1` → `02`; `info`: DSP link still
    up. Play `tone40_48k.wav`: sound, DSP input L, R at −40 dBFS; the DSP console shows the last `R` with layout 3
    (other agent). The app reconnects by itself (HID and OS volume link, PID 0002). Back with `mode 5.1`: 1209:0001
    again, the 5.1 endpoint with its 6 ch 16-bit format, `ee-read` → `01`, 5.1 test file plays. Power-cycle in each
    mode: mode and PID kept. Linux: `lsusb` shows 1209:0001 or 1209:0002, `/proc/asound/card<n>/stream0` the one
    format. macOS: Audio MIDI Setup shows the device of the current mode.

## To verify on hardware
- Codec-port output at 32 data bits with BYOR = 1 and DMA 4 bytes per slot (stock: 16 bits, 2 bytes): test 6a.
- ISO OUT 960-byte circular buffer: INFO "ISO buffer" ≈ 384 bytes at 48 kHz in mode 2.1, ≈ 576 in mode 5.1.
- 5.1 16-bit (stock C-port and DMA values) end to end; BPTSL switch after a mode change with CPTEN = 0 for a few µs
  (DSP and AUX keep running): tests 6b, 13.
- Re-enumeration after `MODE` without reboot, as 1209:0002 / 1209:0001 with separate Windows endpoints (test 13);
  the mode byte at 0x17FF kept across power cycles and updates.
- Descriptor lookup through `desc_tab_51` / `desc_tab_21` (7 EEPROM reads at most per GET_DESCRIPTOR).
- Servo gains at 44.1 kHz and the ±1 % clamp; DMA re-alignment.
- Codec master clock = MCLKO1 (44.1 kHz).
- P1.5 = SENSOR (red LED off during the 200 ms before power_audio_on would confirm it).
- Pod detection window (1 s, stock ~1 s estimated from the loop).
- DSP restart handling (test 12): no I²C NACK from a running DSP (3 in a row take the link down until the next
  DSPINT handshake).
- ROM DFU re-entry from a running application (ROM state assumptions of NOTES.md 3.2).
- UAC2 at full speed with usbaudio2.sys (descriptors, clock RANGE over 4 EP0 packets, GET CUR before SET CUR).
- The interrupt endpoint: NAK while idle, one 6-byte message per change, host GET CUR afterwards (Windows, Linux);
  macOS reaction unknown.
- The host's SET CUR right after enumeration (Windows restores its stored volume) reaches the DSP as `V`.
- Descriptors streamed from U6002 (8 bytes per I²C read) within the host's control-transfer timeouts; the boot
  check (~30 ms of byte reads) before the connection.
- Stock `T` writes at 0x1800..0x1FFF (doc.md: no address restriction documented for `T`).

## Safety rules
- Never write an "application" header (DataType 1) with an untested firmware; flash only after the whole test
  plan passed, and only with the EEPROM update sequence (DataType 3 during writes).
- Endpoint buffer memory: 1304 bytes from 0xFA10. 6 channels at 24 bits is impossible.

## oc_tas_test (test firmware)
Enumerates as a vendor HID device (1209:0001, bcdDevice 0x0001), keeps VREN and DSP reset low (silent).
Commands `[cmd][addr lo][addr hi][len]` → `[cmd][addr lo][addr hi][len][data…]` or `[cmd|0x80][error]`:
0x01 READ_CODE (0x8000..0x9FFF = boot ROM), 0x02 READ_XDATA, 0x03 READ_IDATA, 0x10 INFO, 0x7F REBOOT_ROM, and
**0x20 RESTORE_HEADER** (`test/restore_header.c`): the only write, puts the EEPROM header back to stock if and
only if it is exactly the DataType-2 header `6a 12 12 34 05 a7 10 20 0a 04 04 01 08 20 02 00 15 e4`.
Host: `tools/tas_romdump.py` (`--info`, `--reboot`, `--restore-header [--yes]`).

### Developing without desoldering (ROM DFU mode, DM 2.2.2)
- EEPROM header byte 14 `DataType`: 0x01 = application (boots the EEPROM firmware), 0x02 = "device EEPROM"
  → the ROM boots in **DFU** and loads the downloaded firmware **into RAM**. The EEPROM code stays intact.
- Switch to 0x02 with the stock firmware's HID `T` command (`oc tas mode dfu --yes`); back to 0x01 with
  `oc_tas_test` `RESTORE_HEADER`, or `oc tas mode app` when the stock firmware runs.
- After a DFU download the host must reset the bus at once (`pnputil /restart-device`, tools/tas_dfu_run.ps1);
  the ROM then starts the RAM code at 0x0000.
- Recovery: power cycle (with DataType 2 the ROM comes back in DFU; nothing is persistent).
