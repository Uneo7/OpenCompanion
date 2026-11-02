# U7001 EEPROM layout (OpenCompanion boot chain)

U7001 = AT24C512 (64 KB, 128-byte pages, I2C1 address 0x50, WP tied to ground). The DSP ROM boots from it in
I2C1-master AIS mode (`re/dsp/notes/eeprom_safety.md` §1). Sources of truth: `boot1/include/layout.h` (DSP) and
`app/crates/oc-ais/src/eeprom/layout.rs` (host). Both must change together, and boot1 cannot change after the
migration.

## Map
| Range | Size | Content | Written |
|---|---|---|---|
| 0x0000–0x00FF | 256 | **boot0**: AIS magic, Section Load of the trampoline (≤ 232 bytes, 228 today → RAM 0x1002AF00), Jump_Close 0x1002AF00 | once (migration) |
| 0x0100–0x017F | 128 | **image header** `OCI1` (48 bytes, rest 0xFF) | every update, **last** |
| 0x0180–0x01FF | 128 | **boot-state** `OCBS` (16 bytes): trial counter / confirmation | boot1 (trial images), firmware (confirmation) |
| 0x0200–0xC1FF | 49,152 | **image body** (AIS magic, LZ4 sections, Jump_Close) | every update |
| 0xC200–0xDAFF | 6,400 | **profiles**: slot *s* copy *c* at `0xC200 + (2s + c) × 0x300`; config A/B at 0xDA00 / 0xDA80; v3: the last page of each copy is a **graph data page** | firmware (graph pages: host) |
| 0xDB00–0xFF7F | 9,344 | **boot1**: code/const (≤ 9,340 bytes, 0xFF padded) + CRC-32 at 0xFF7C | once (migration) |
| 0xFF80–0xFFFF | 128 | last page, kept from the stock EEPROM (0xFFFF = stock `CKT` value 0x02) | never |

Constraints behind the numbers:
- **boot1 must lie entirely above the stock image**, which ends at 0xDAD8 (`u7001_dsp.bin`: Jump_Close at
  0xDAD0; every byte from 0xDAD8 to 0xFFFE is 0xFF). It is written while the stock firmware still boots, so
  writing it can never break the stock boot. Space above the stock image: 0xDB00–0xFF7F = 9,344 bytes.
- boot1 in C (CGT 7.4.24, `-mv67p -O3 -pm --opt_for_space=3`) is **9,328 bytes** (measured 2026-11-01; 9,136 when the layout was chosen). The 3,968 bytes
  planned at 0xF000 in `eeprom_safety.md` (estimate "2 to 3 KB") are not enough: the C67x+ has no compact
  instructions. boot1 is therefore at 0xDB00, the profiles move below it, and the body is LZ4-compressed.
- **boot0 is 2 pages**: magic (4) + Section Load header (12) + trampoline + Jump_Close (8) ≤ 256, so the
  trampoline is ≤ 232 bytes (it is 228 today, boot0 252 bytes). A 1-page boot0 would need a 104-byte trampoline: not
  possible without knowing the ROM internals (see `UPDATE.md`, "Making the window smaller").
- Body capacity 49,152 bytes. Current firmware (`build/opencompanion_dsp.out`, 2026-11-01, with the stock
  protection, loudness, rate switch, bass source and boot hooks): 123,284 bytes loaded (code/data + zero fills) →
  **44,872-byte body** (91.3 % of the area, 4.2 KB free). With the stock graph engine (layout v3, 2026-11-01):
  **48,192-byte body** (98.0 %, 960 bytes free) + 921 bytes in the graph data pages. Measure with
  `oc-bootimg sizes`.

### Layout v3: graph data pages (2026-11-01; `include/bose_tables.h`, `oc-ais` `eeprom/graph.rs`)
The 0x0000–0xFFFF map is unchanged; v3 only gives a role to bytes no one used. A profile record is 632 bytes
(header 16 + profile 616 = pages 0..4 of its 0x300-byte copy), so page 5 of every copy was never written by any
firmware. These eight pages are the **graph data pages**: page *k* (k = 2s + c) at
`0xC200 + k × 0x300 + 0x280` = 0xC480, 0xC780, 0xCA80, 0xCD80, 0xD080, 0xD380, 0xD680, 0xD980.
| Offset (concatenated pages) | Size | Field |
|---|---|---|
| 0 | 4 | magic `OCG1` |
| 4 | 2 | version = 1 |
| 6 | 2 | payload length (≤ 1,008) |
| 8 | 4 | RAM address of the payload (start of the firmware's `.bose_tab`) |
| 12 | 4 | CRC-32 (zlib) of the payload |
| 16 | n | payload: the bytes of the ELF segment `.bose_tab` (stock graph program tables, 921 bytes today); rest 0xFF |
- Why there: carving a new range is impossible (body, boot1 and boot0 are full; no free page), and the profile
  slots keep their addresses, size and A/B copies, so no profile is moved or lost and the four slots stay.
- Safety: a v2 firmware never reads or writes these pages (its records and its write lengths stop at +0x278), and
  classifies a copy as ours / blank from its first 632 bytes only. The v3 firmware refuses any write to them
  (`eeprom.c`), and checks the tables it reads against the CRC generated with them (`BG_TAB_CRC`), so tables of
  another build are rejected; missing or rejected tables only disable the Bose base (EQ-path fallback, `OPI`
  flags bit 3).
- Written by the host only: `oc-bootimg build` puts them in the full image (also over `--keep-profiles`);
  `oc install dsp` surveys them page by page and writes / CRC-checks the differing ones (step `Graph`), after the
  body and before the header; `oc-bootimg verify` reports them. The firmware ELF must define
  `oc_eeprom_layout_v3` when it has a `.bose_tab` segment.

### Profile area and firmware write policy (implemented: `include/profiles.h`, `src/storage/eeprom.c`)
Profile copies at `0xC200 + (2s + c) × 0x300`, config **0xDA00 / 0xDA80**; firmware write window
**0xC200..0xDAFF** minus the eight graph data pages (v3), plus exactly the pages 0x0100 (header invalidation by `UPD`, zero bytes only) and 0x0180
(trial confirmation). (The first firmware versions used 0xE000..0xF8FF, which overlaps boot1: `oc-bootimg`
refuses an ELF without `oc_eeprom_layout_v2`.)

## Image header (page 0x0100, little-endian)
| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `OCI1` |
| 4 | 2 | header version = 1 |
| 6 | 2 | header size = 48 |
| 8 | 4 | sequence number (previous + 1 at each update) |
| 12 | 4 | body length in bytes (multiple of 4, 12..49,152) |
| 16 | 4 | CRC-32 of the body (IEEE / zlib) |
| 20 | 4 | entry point (must equal the body Jump_Close target, inside application RAM) |
| 24 | 4 | flags: bit 0 `HF_TRIAL` (trial counter enforced); other bits must be 0 |
| 28 | 4 | firmware version (informative) |
| 32 | 4 | bytes loaded into RAM (informative) |
| 36 | 8 | reserved, 0 |
| 44 | 4 | CRC-32 of bytes 0..43 |
Bytes 48..127 are 0xFF. The header is the **commit record**: it is written last, in a single page write, after
the body has been written and its CRC checked in the EEPROM.

## Boot-state record (page 0x0180)
`OCBS`, sequence (LE32, = header sequence), attempts (u8), confirmed (u8, 0xA5 = confirmed), 2 reserved bytes,
CRC-32 of bytes 0..11. Only used when the header has `HF_TRIAL`: boot1 increments `attempts` before each start
of an unconfirmed image and stays in rescue mode after 2 unconfirmed starts. An invalid or blank record, or one
for another sequence number, counts as 0 attempts.

## Body
```
0x41504954                                        AIS magic
{ 0x58535980, address, raw size, compressed size, LZ4 block, 0-3 pad bytes }*
0x58535906, entry                                 Jump_Close
```
- `0x58535980` (LZ4 Section Load) is an OpenCompanion opcode; the body is read by boot1 only, never by the ROM.
- Every ELF segment and every zero fill (.bss/.far/.stack) is one LZ4 section (zeros compress to ~1/255).
- LZ4 = standard block format (lz4_Block_format.md). boot1 decodes straight to RAM, refuses a match offset
  before the section start and any output beyond `raw size`, and requires the block to use exactly
  `compressed size` bytes.
- Destinations must be inside application RAM 0x10000000..0x1002AEFF; the length must be exact (no byte
  after Jump_Close).

## RAM map at boot
| Range | Use |
|---|---|
| 0x10000000–0x1002AEFF | application image (and the RAM-only updater at 0x10001000) |
| 0x1002AF00–0x1002AFE7 | boot0 trampoline (loaded by the ROM; dead once boot1 runs) |
| 0x1002B000–0x1002D47F | boot1 code/const (+ CRC word) as read from 0xDB00 |
| 0x1002DFF0–0x1002DFF7 | software rescue request flag (`OCRS`, ~`OCRS`) |
| 0x1002E000–0x1002FFFF | boot1 variables (484 bytes) and stack (2 KB) |
The ROM itself only uses 0x10000400–0x10000FFF while it runs (SPRAA69D §10), which the trampoline avoids.
The firmware may use the whole RAM once it runs, but must never **load** anything at or above 0x1002AF00,
and must not keep data at 0x1002DFF0 except to request rescue (today it uses up to 0x10022FFF).
