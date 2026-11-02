# Update procedures and power-loss analysis

All EEPROM writes go through the **updater** (RAM-only, `E` 10..14) and are read back page by page. Plans are
produced by `oc_ais::eeprom::update`. One `E` round trip ≈ 40–50 ms (HID + 20 ms TAS delay + I2C), so a
128-byte page = 3 messages ≈ 0.15 s. Never use the stock HID `D` command to write U7001: the stock DSP asks for
standby after 120 s of silence, the TAS then resets the DSP and Q7100 cuts U7001's power in the middle of a
write (`eeprom_safety.md`, "Migration initiale").

Definitions: *recoverable* = the speaker reaches a state where the host can talk to the DSP over USB again,
without opening it. Two such states exist: the stock firmware running (before the switch), and boot1 rescue
mode (after it). A *brick* needs a programmer on U7001.

## 1. One-time migration from the stock EEPROM
Run from our code in RAM. Preconditions: USB connected, PC kept awake, volume low.

| # | Step | EEPROM state if power is lost during / after the step | Recovery |
|---|---|---|---|
| M0 | Read the whole EEPROM (`d` on stock, or updater `EE_READ`) and compare with the backup (`u7001_dsp.bin`); also save any profiles at 0xE000–0xF8FF | unchanged | — |
| M1 | Stock RAM loader hijack, then load the **updater** at 0x10001000 (WRITE/CRC), JUMP. Optional dry run: load **boot1** at 0x1002B000 and JUMP: it must answer INFO `R`, reason 3 (stock bytes at 0x0100 are not `OCI1`), which exercises the boot window, the EEPROM read path and rescue; then load the updater from rescue | unchanged | stock boots |
| M2 | `EE_UNLOCK_BOOT`; write boot1 (73 pages, 0xDB00–0xFF7F); `EE_CRC` over the area = CRC of the expected image | only 0xDB00–0xFF7F changed; it is **above the stock image (ends 0xDAD8)**, so the stock boot is intact at every instant | stock boots; redo M1–M2 |
| M3 | Stage boot0 (256 bytes) at RAM 0x10020000 with WRITE, check with CRC, then **one** `EE_WRITE_BOOT0 0x10020000 crc`: the updater writes 0x0000 then 0x0080 back to back and reads both back | **critical window** (below) | before / after the window: see below |
| M4 | Write the image like a normal update (§2, starting with the header invalidation), erase the boot-state page and the profile area 0xC200–0xDAFF (0xFF), header last | boot0 + boot1 valid, header not valid | boot1 → rescue (reason 3/4/5/6) → load updater, redo M4 |
| M5 | `REBOOT`; check that the firmware answers `SVM`; `oc-bootimg verify` on a fresh read | — | if it does not boot: rescue (CRC) or trial (if `--trial`) |

Every step except M3 is retried as often as needed. If M3's read-back fails, the updater returns `15 03` and the
host re-issues `EE_WRITE_BOOT0` at once (the DSP is still running from RAM).

### The critical window (M3)
States of pages 0x0000 / 0x0080 during M3:
1. Both stock: the stock image boots. Any power loss **before** the STOP condition of the page-0 write is safe:
   an AT24C512 page write is only committed at STOP; an interrupted transfer changes nothing.
2. Page 0 in its internal write cycle (tWR ≤ 5 ms): content undefined → **brick**.
3. Page 0 new, page 1 stock: the new Section Load covers 0x0010–0x00F7, so the trampoline tail is stock data →
   **brick**. (Writing page 1 first is no better: the stock AIS stream runs through page 1.)
4. Page 1 in its write cycle → **brick**.
5. Both new: boot0 → boot1 → rescue (no header yet): recoverable.

Window = tWR(page 0) + read-back of page 0 (~3.5 ms) + transfer of page 1 (130 bytes at ~394 kHz ≈ 3.3 ms)
+ tWR(page 1) ≈ **17 ms worst case** (tWR max 5 ms per the AT24C512 family datasheets; **[HW]**: check the
datasheet of this die revision).
Only a power loss or a DSP reset inside these ~17 ms bricks the speaker:
- the DSP cannot reset itself: interrupts are off, nothing arms the watchdog;
- the TAS only resets the DSP on standby, which happens only if the DSP's `S` reply asks for it; the updater
  never does (`doc.md`, `PollDspStatus`). Our TAS firmware must also hold off (RESCUE.md);
- a USB suspend of the host could make the stock TAS run its standby logic: keep the PC awake.

### Making the window as small as possible
Done:
- boot0 is 2 pages, not more: boot1 (9 KB) is written **before**, outside the stock image, so it is not in the
  window. Writing boot1 after the switch would put 73 page writes (~0.4 s+) inside it.
- Both pages are written by **one updater command** from data already in RAM and CRC-checked: no USB round trip,
  no host scheduling, no TAS delay between the two pages. Polled I2C at ~394 kHz, ACK polling ends each
  write cycle as soon as the EEPROM is ready (typical tWR is below the 5 ms maximum).
- Each page is read back right after its own write cycle. For page 0 this read (~3.5 ms) falls inside the
  window; it is kept because it stops the sequence before page 1 if page 0 is wrong. Dropping it is the only
  remaining software-side reduction.
Possible further reductions (not done, need hardware knowledge):
- **1-page boot0**: patch only bytes 0x14..0x7F of the stock stub so that the ROM's own read pointer jumps to
  0xDB00. Needs the ROM AIS reader's state in 0x10000400–0x10000FFF: dump the ROM read-only with `BRR`
  0x00000000–0x0003FFFF first. Window would drop to one tWR (≤ 5 ms).
- Higher I2C1 speed during M3 (the AT24C512 is a 400 kHz part at 2.7 V; 1 MHz parts exist, not this one).

### Restoring the stock firmware later
Implemented in `app/crates/oc-install/src/restore/dsp` (`oc install restore --dsp`), from the user's own backup
(read from the speaker before the install; no Bose image is shipped). Order: R1 header invalidated + stock body
pages, R2 boot-state page, profiles and header page (boot0/boot1 still ours: any interruption ends in rescue),
R3 stock boot0 in one `EE_WRITE_BOOT0` (same ~17 ms window as M3), R4 stock bytes over 0xDB00..0xFF7F (after the
switch: the stock AIS stream ends at 0xDAD8, so the stock firmware already boots), R5 full 64 KB verification,
`REBOOT`, `SVM` = D01.01.01. boot1 is restored after boot0, not before: with our boot0 in place every boot1 page
write would be a brick window. A better option (not implemented): keep boot0/boot1 and store a converted stock
image in the body area (needs inflate + PLL in boot1: does not fit).

## 2. Normal update (firmware running)
1. Host → firmware console `UPD` (`src/storage/bootctl.c`): the firmware zeroes the header page, reads it back
   and reboots (`BST` shows a failure if it does not reboot).
   Without `UPD` (current firmware): our TAS firmware's rescue request, or `RSC`.
2. boot1 finds no header → **rescue** (INFO `R`, reason 3).
3. Host loads the updater (6,850 bytes, ~6 s) and JUMPs to it.
4. `update_plan`: header page := zeros (no-op if `UPD` did it), body pages from 0x0200, `EE_CRC` of the body =
   header CRC, then the header page, `EE_CRC` of the header page.
5. `REBOOT`. boot1 checks header + CRC, loads, jumps. With `--trial`, the firmware confirms after ~10 s.

Power loss at any point of 1–4: the header is invalid (zeros, or never written) or the CRC of the body does not
match → boot1 always ends in rescue mode → the host resumes at step 3. After step 4 the new image is committed.
A power loss **during** the header page write leaves a page that fails the header CRC (or the magic) → rescue.
Proof of each case on the real boot1 code: `test/run_host_tests.sh` (corrupted header fields, body bytes,
truncated bodies, zeroed header → rescue every time; intact image → RAM identical to the ELF).

## 3. Recovery
| Situation | What happens | Host action |
|---|---|---|
| Update interrupted, image corrupted, header missing | boot1 → rescue at every power-up | INFO shows `R` + reason → load updater → §2 step 4 with the bundled firmware |
| Image intact but the firmware hangs before DSPINT (stock TAS freezes in `InitDspLink`) | with `--trial` images: 2 power cycles → rescue (reason 8). Without: our TAS firmware's rescue request | power-cycle twice, or send the TAS rescue request |
| Firmware runs but misbehaves | `RSC`, or `UPD` | reload a good image |
| boot1 or boot0 damaged (only possible by a bug writing outside the window, or during M3) | ROM cannot boot | programmer on U7001 (the only brick) |

## Why the immutable parts stay intact afterwards
- boot1 and the updater refuse EEPROM writes outside 0x0100–0xDAFF unless explicitly unlocked (updater only,
  migration command); the last page is never writable. The firmware's own window (RESCUE.md, change 2) excludes
  boot0/boot1 too.
- The hardware does not help (WP tied to ground): any old firmware with the old profile addresses
  (0xE000–0xF8FF) **would overwrite boot1**. Hence the `oc_eeprom_layout_v2` symbol required by `oc-bootimg`,
  and the same check needed in `oc-ramload`.
- No stock firmware ever runs again after M3, so the stock HID `D` raw-write path is gone.

## Hardware log
- 2026-11-01, M1 dry run (no EEPROM write): boot1 loaded in RAM through the stock RAM loader hijack
  (`tools/dsp_ramload.py --image boot/build/boot1.out`), INFO answered `06 'O' 'C' 'L' 01 'R' 03 01`
  (rescue, reason 3 = no OpenCompanion header, boot1 v1): boot window, I2C1 EEPROM read path and rescue
  protocol validated. REBOOT from rescue returned the DSP to the stock firmware (SVM D01.01.01) with the stock
  TAS still running.
