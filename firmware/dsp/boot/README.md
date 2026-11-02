# firmware/dsp/boot — brick-proof boot chain for U7001

Two immutable stages in the DSP boot EEPROM (U7001), written once during a migration from the stock EEPROM,
plus a RAM-only updater. After the migration, an interrupted or corrupted firmware update always ends in a
**rescue mode** reachable over USB (TAS `E` relay), never in a DSP that cannot boot.
Analysis this implements: `re/dsp/notes/eeprom_safety.md` (with the changes listed under "Deviations").

| Doc | Content |
|---|---|
| `docs/LAYOUT.md` | EEPROM map, image header, boot-state record, body format, RAM map, profile area proposal |
| `docs/RESCUE.md` | rescue triggers, rescue / updater protocol, what the TAS firmware and the DSP firmware must do |
| `docs/UPDATE.md` | migration, normal update, recovery, power-loss analysis of every step |

## Boot sequence
1. ROM (I2C1 master AIS) reads **boot0** (0x0000, 2 pages): Section Load of a 228-byte trampoline at
   0x1002AF00, Jump_Close.
2. **Trampoline** (`boot0/trampoline.asm`): resets I2C1 status (ROM clock settings kept), reads 9,344 bytes at
   0xDB00 into 0x1002B000, invalidates L1P, branches.
3. **boot1** (`boot1/`): PLL (stock sequence), I2C1 at ~394 kHz; software rescue flag; ~0.1 s I2C0 window for
   a TAS rescue request; header check; body loaded in one pass (LZ4, range-checked, CRC-32 over the stored
   bytes); optional trial counter; jump. Any failure → rescue mode: I2C0 slave 0x29, DSPINT handshake, RAM loader
   protocol (the existing host loader works unchanged).
4. **Updater** (`updater/`, RAM only, loaded through rescue): EEPROM read / write / CRC, migration commands.

## Build (CGT 7.4.24, `-mv67p`)
```
make            # build/tramp.out, build/boot1.out, build/updater.out (+ .map); warnings are errors
make clean
```
Host tool (Rust, `app/crates/oc-ais`, module `eeprom` + binary `oc-bootimg`; never talks to the device):
```
oc-bootimg sizes  --tramp build/tramp.out --boot1 build/boot1.out --updater build/updater.out --app ../build/opencompanion_dsp.out
oc-bootimg build  --tramp … --boot1 … --app … --dump u7001_dsp.bin --seq 1 -o eeprom.bin
oc-bootimg verify dump.bin --tramp … --boot1 …
```
`build` refuses a firmware ELF without the `oc_eeprom_layout_v2` symbol (see RESCUE.md, firmware changes):
`--allow-legacy-app` exists for host tests only.

Host tests (no hardware): `ZIG=… CARGO=… sh test/run_host_tests.sh` builds an EEPROM image, runs boot1's real
`image.c / stream.c / lz4.c / crc32.c / trial.c` (32-bit x86 build, DSP RAM window mapped at 0x10000000) on it,
checks that the RAM equals the firmware ELF byte for byte and that nothing else was written, then corrupts the
image 17 ways (each must end in rescue) and checks the trial counter. `cargo test -p oc-ais` covers the header,
LZ4, body and update-plan code.

## Sizes (2026-11-01, `oc-bootimg sizes` on the current build)
| Part | Size | Limit |
|---|---|---|
| boot0 trampoline | 228 bytes (boot0 = 252 bytes in its 2-page, 256-byte area) | 232 |
| boot1 | 9,328 bytes (code 9,216 + const/switch 112), 12 bytes free | 9,340 (0xDB00–0xFF7B) |
| boot1 RAM variables / stack | 484 bytes / 2 KB | 0x1002E000–0x1002FFFF |
| updater | 6,850 bytes (RAM 0x10001000) | — |
| current firmware body (LZ4) | 44,872 bytes (123,284 bytes loaded incl. zero fills; 91.3 %, 4.2 KB free) | 49,152 |

## Deviations from `eeprom_safety.md` (with evidence)
- **boot1 at 0xDB00 (9,344 bytes), not 0xF000 (3,968)**: measured C size 9,136 bytes then (9,328 today), even at `-O3 -pm -ms3`
  (C67x+: no compact instructions). It must stay above the stock image end (0xDAD8) so that it is written
  while the stock firmware still boots; hence profiles move to 0xC200–0xDAFF and the body is LZ4-compressed
  (34.6 KB instead of 45.6 KB, area 48 KB).
- **EEPROM writing is not in boot1**: rescue only offers the RAM protocol; the EEPROM commands are in the
  RAM-only updater. The immutable code is smaller and a bug in the write path can be fixed later.
- **TAS rescue request = an I2C0 message during a boot window**, not a GPIO level: the stock TAS drives P1.6
  (DSPINT) low whenever USB is present after the link is up (`PollUsbVbusAndDspPower`, `u6002_tas.bin`
  0x0FD7), and P1.7 (BADDATA) follows the audio stream (`doc.md`), so neither level is a safe strap. Only our
  TAS firmware sends the message.
- **Trial counter in the EEPROM** (opt-in `HF_TRIAL`) instead of a RAM counter armed by the RTI watchdog:
  works across power cycles and needs no watchdog support in the firmware.

## Unverified on hardware
- Trampoline: that the ROM leaves I2C1 idle after Jump_Close, ICSAR = 0x50, and a usable bus speed (the Bose
  stub resets I2C1 at the same point and the ROM keeps working, `hardware.md` §1).
- boot1 I2C1 master (copied from `src/eeprom.c`, itself untested on hardware), ACK polling, ~394 kHz.
- Boot window duration (loop count), time from DSPRESET\ release to the window (TAS firmware timing).
- RAM retention across the RTI reset (software rescue flag only).
- McASP0 GPIO / DSPINT in rescue after a cold boot (values measured on the stock firmware).
- tWR of this AT24C512 revision (window figure in UPDATE.md).
- Nothing here has run on the DSP yet. Test order: boot1 in RAM through the stock RAM loader (UPDATE.md M1,
  no EEPROM write), then the updater's EEPROM read commands, then writes in the free area, then the migration.
