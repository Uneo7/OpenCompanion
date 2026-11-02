# Rescue mode, updater, and what the TAS / DSP firmwares must do

## When boot1 enters rescue mode
In this order (`boot1/src/main.c`):
1. **Software request**: RAM 0x1002DFF0 = 0x5352434F (`OCRS`) and 0x1002DFF4 = its complement. The flag is
   cleared as soon as it is read. Written by our DSP firmware just before its RTI-watchdog reboot.
   **[HW]** depends on RAM being retained across the RTI reset; if it is not, the request is lost and the image
   boots normally (never worse).
2. **TAS request** during the **boot window** (~0.1 s, `WINDOW_POLLS` = 400,000 polls, to be measured):
   I2C0 slave 0x29 is up, DSPINT is *not* pulled low. An `E` message `7E 'O' 'C' 'R'` gets the reply
   `06 'O' 'C' 'R'` and boot1 stays in rescue. `P` and `S` are answered normally; any other `E` gets `15`.
3. **Image check failed**: no header, header CRC/version/length/flags/entry invalid, body opcode/range/length
   invalid, body CRC mismatch, EEPROM I/O error.
4. **Trial exhausted** (`HF_TRIAL` images only): two starts without confirmation.

The reason is reported by `INFO` (table below). Rescue never requests standby and never writes the EEPROM.

## Rescue protocol (TAS `E` relay, unchanged RAM loader protocol)
Frames, timing and the host side are those of the RAM loader (`../README.md`, RAM loader section, `app/crates/oc-ramload`):
the existing host loader works against rescue mode as is.

| Sub-command | Request | Reply | Rescue | Updater |
|---|---|---|---|---|
| INFO | `00` | `06 'O' 'C' 'L' 01 mode reason boot1-version` | yes | yes |
| WRITE | `01 addr:u32 data…` | `06` | yes | yes |
| CRC | `02 addr:u32 len:u32` | `06 crc32` | yes | yes |
| JUMP | `03 entry:u32` | `06`, then jumps | yes | yes |
| READ | `04 addr:u32 n:u8` | `06 data…` | **no** (size) | yes |
| FILL | `05 addr:u32 len:u32 byte` | `06` | yes | yes |
| REBOOT | `06` | `06`, then RTI reset | yes | yes |
| EE_READ | `10 addr:u16 n:u8 (≤ 60)` | `06 data…` | no | yes |
| EE_WRITE | `11 addr:u16 data (1..56, one page)` | `06` / `15 code` | no | yes |
| EE_CRC | `12 addr:u16 len:u32` | `06 crc32` | no | yes |
| EE_UNLOCK_BOOT | `13 'U' 'N' 'L' 'K'` | `06` | no | yes (migration) |
| EE_WRITE_BOOT0 | `14 ram:u32 crc32:u32` | `06` / `15 code` | no | yes (migration) |
| RESCUE | `7E 'O' 'C' 'R'` | `06 'O' 'C' 'R'` | yes | yes |

- `mode`: `R` rescue (boot1), `U` updater. `reason`: 0 none, 1 software request, 2 TAS request, 3 no header,
  4 bad header, 5 bad body, 6 body CRC, 7 EEPROM I/O, 8 trial exhausted, 9 updater.
- RAM writes, fills and jumps are refused in [0x1002AF00, 0x10030000) (boot chain). EEPROM writes are refused
  outside [0x0100, 0xDB00) unless unlocked; the last page is never writable. Every EEPROM write is read back
  (`15 03` on mismatch). Codes: 1 refused, 2 I2C error, 3 read-back mismatch, 4 staged-data CRC mismatch.
- A page write (transfer + tWR ≤ 5 ms + read-back) ends well inside the 20 ms the stock TAS waits before it
  reads an `E` reply (`doc.md`, HID `E`).
- **Rescue pulls DSPINT low** (AXR0[9], stock GPIO values), and releases it on `U`, like the stock firmware.
  Reason: the stock TAS `InitDspLink` (`u6002_tas.bin` 0x0C60) does `SETB P1.6` then loops on `JB P1.6` with
  no timeout until the DSP pulls the line low; until then it relays nothing.

## Updater
RAM-only build of the same link code plus the EEPROM commands (`updater/`), loaded at 0x10001000 with WRITE +
CRC and started with JUMP, either from rescue mode or, for the migration, from the stock firmware through the
RAM loader hijack. It is not immutable: EEPROM-writing bugs are fixed by shipping a new updater.

## What the TAS (USB) firmware must do
The stock TAS firmware works with all of this (rescue does the DSPINT handshake). Our TAS firmware should:
1. **Rescue on demand**: after releasing DSPRESET\ (TAS pin 41), *without waiting for DSPINT*, write the frame
   `45 FF 04 7E 4F 43 52 00` to 0x29 every ~10 ms, then read one 8-byte reply frame 20 ms later. Until boot1's
   window opens (ROM boot0 + trampoline read of 9,344 bytes at the ROM's I2C speed + PLL, **[HW]** to measure,
   expect < 1 s) the DSP does not acknowledge 0x29: retry on NACK, with a bounded number of attempts (3 s).
   Success = reply frame `45 FF 04 06 4F 43 52 00`. Then do the usual `InitDspLink` (wait for DSPINT low,
   `X`/`U`/`F`). Triggers worth offering: a HID command "restart DSP in rescue", and pod mute held at power-up.
2. **Never wait forever** for DSPINT or on an I2C NACK (the stock `InitDspLink` does; a dead image then also
   freezes USB). Time out after ~3 s and keep the HID interface alive so the host can still request a reset
   into rescue.
3. **No DSPRESET\ during an EEPROM session**: do not enter standby while the host has an update session open,
   and only reset the DSP when the host asks. (The stock TAS goes to standby only when the DSP's `S` reply
   asks for it; rescue and the updater never do.)
4. Keep the `E` relay delay ≥ 10 ms (page write + read-back < 10 ms); 20 ms like the stock is fine.
5. Do not drive P1.6 low before the DSP has signalled ready; boot1 does not use it as an input.

## Changes needed in the DSP firmware (all applied 2026-11-01: `../../README.md` "Boot chain integration")
1. `include/profiles.h`, `src/profiles.c`: profile base 0xE000 → **0xC200**, config copies 0xF800/0xF880 →
   **0xDA00/0xDA80** (map comments too).
2. `include/eeprom.h`: `EE_WRITE_LO/HI` 0xE000/0xF900 → **0xC200/0xDB00**; additionally allow exactly the pages
   0x0100 (header invalidation) and 0x0180 (trial confirmation). Nothing else, ever (boot0/boot1 protection).
3. Declare the layout so the host tool accepts the image: e.g. in `src/main.c`
   `#pragma RETAIN(oc_eeprom_layout_v2)` + `const uint32_t oc_eeprom_layout_v2 = 2;` (`oc-bootimg` refuses an
   ELF without this symbol, because an old firmware would write its profiles over boot1).
4. `lnk.cmd`: cut `IRAM_HI` at 0x1002AF00 (it is declared up to 0x1002FFFF but unused today); never load
   anything in [0x1002AF00, 0x10030000).
5. Console commands:
   - `UPD` — invalidate the header (write 128 zero bytes at 0x0100, read back), then reboot: boot1 comes back
     in rescue mode (reason 3) without relying on RAM retention. Used to start an update.
   - `RSC` — write the rescue flag (0x5352434F, 0xACADBCB0 at 0x1002DFF0) and reboot (for tests and recovery).
6. Trial confirmation (only needed for images built with `--trial`): once the TAS link is up (DSPINT done) and
   audio frames have been flowing for ~10 s, read the header; if `HF_TRIAL` is set and the state record is not
   confirmed for this sequence, write `OCBS, seq, attempts, 0xA5, 0, 0, CRC-32` at 0x0180 (format in
   `LAYOUT.md`). Until this exists, build images without `--trial`.
7. Keep the cold-boot path as is: boot1 hands over with McASP0 at reset values and I2C0 in reset, so
   `mcasp0_gpio_init()` sees a cold boot and does the DSPINT handshake.
8. Never relay a raw EEPROM write behind the I2C `D` command.

Host side (`app/crates/oc-ramload`): after the migration, refuse to RAM-load a firmware ELF without
`oc_eeprom_layout_v2` (an old firmware saving a profile would overwrite boot1 at 0xE000..0xF8FF).
