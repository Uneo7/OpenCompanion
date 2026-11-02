# Parametric base: format and tooling (spec, draft 1)

Status: stage 1 done (host: `oc-device` `base/`, `oc base export|check`; golden test bit-exact against `bg_f`);
firmware stages not started. Contributor-facing. Owner of the numbers below: this document; sources
of truth once implemented: `app/crates/oc-device/src/base/` (host) and `firmware/dsp/include/base.h` (DSP).

## 1. Goal and scope
The DSP runs **one base program** with a **fixed topology** (compiled into the firmware) whose **parameters**
(filter sections, gains, delays) live in the EEPROM and can be replaced from the PC. The first and, for now,
only base is **"Bose d'origine"**: the stock Companion 5 program, converted on the user's machine from the
user's own U7001 backup, as faithfully as the topology allows (today: bit-identical to the `bld_e` firmware).
Profiles (user EQ, loudness, trims, sub level) stay as today and run on top of the installed base.

Non-goals (this draft): several bases on the speaker, an app UI for bases, a built-in "Neutre" base (the
built-in neutral program stays as the fallback, section 6), editable topology.

Terms: *section* = one biquad, Bose convention (section 3.3); *set* = a set gain plus N sections applied to one
or more channels; *block* = a named group of parameters in the diagram below; *topology* = wiring and fixed
constants, identified by `topology` = 1 (`c5-stock-1`).

## 2. Block diagram (topology 1)
Signal levels: float ±1.0 = full scale at the inputs and at the DAC. All parameters are given at 48 kHz; the
firmware re-maps sections and scales delays at 44.1 kHz (`rate.c`, unchanged). **[F]** = fixed in the
firmware (not in the base), **[B]** = base parameter (JSON, EEPROM).

```
USB L R C LFE Ls Rs ──[F] USB mute ramp, input level 0.340408
                    ──[F] BADDATA selector (all-pass / LR4 low-pass, cross-fade)
AUX L R ───────────────────────────────┐
                    ──[B] aux_mix      L = u·L + a·AUX L,  R = u·R + a·AUX R
                    ──[F] loudness     band-pass + direct / band gains (loudness.c, profile flag)
                    ──[B] input_gain   6 gains (L, C, R, LFE, Ls, Rs)
                    ──[F] spatial      detector (HPF, L+R, L−R, block power) + control law (bose_spatial.c)
                                       → per side: direct d·L, array a·L, third t·L (smoothed)
                    ──[B] centre_mix   direct' = k_d·direct + k_c·C   (per side)
                    ──[B] surround_mix surround = k_s·Ls + k_t·third  (per side)
                    ──[F] mode stage   normal mode 'N' constants
                    ──[B] bass_sum     bass = Σ w_i · {direct' L, direct' R, array L, array R, Ls, Rs, LFE}
                    ──[B] driver_eq    set "direct"   → direct' L, R      (8 sections)
                                       set "array"    → array L, R        (8 sections)
                                       set "surround" → surround L, R     (8 sections)
                                       set "bass"     → bass              (8 sections)
       bass ────────[B] bass_phase     1 all-pass section + gain ─────────────────────────────→ bass out
       surround ────[B] surround_net   2×2 lattice: delays, 3 filter stages, cross delays ──┐
       direct, array[B] front_net      direct filter, array cross-feed lattice, delays ─────┤
                    ──[F] output summing (unity sums) → inner L, inner R, outer L, outer R ─┘
then, unchanged and outside the base: profile output EQ bank, trims, sub level, post-stage protection
(band clippers, compressors, look-ahead limiter 0.95, bass clipper, VRAW ceiling, −6 dBFS limiter).
```

The profile global EQ bank runs on L / R **before** the base (after the USB / AUX inputs), the profile output
EQ bank on its 5 outputs **after** it (as `BGP_WRAP_EQ` today).

### 2.1 Fixed parts [F] and why
| Part | Reason |
|---|---|
| USB mute ramp, input level | control path (`volume.c`), shared with the neutral program |
| BADDATA selector (sections and smoother) | signal-integrity path, shared with the neutral program (stays in the image body) |
| Loudness band-pass, gains, law | the loudness feature (profile flag `PF_LOUDNESS`), shared with the neutral program |
| Spatial detector, control law, smoother pole | behaviour code (`bose_spatial.c`), not a filter set |
| Mode stage, output summing, unity mixers inside the networks | wiring, not parameters |
| Post-stage protection | locked to the stock limits; no base can change it |

### 2.2 Removed from the stock wiring
The stock DC blocker (1 section × 6 channels after the AUX mix) writes wires that the loudness band-pass
overwrites before anyone reads them: its output never reaches an output or a meter (op 7 vs op 8 in
`bose_graph_data.c`; confirmed by the voicing analysis, reach = none). Topology 1 drops it: output identical,
6 channel-sections less per frame. **Gate**: the firmware bit-identity test (section 9) must pass with it removed.

### 2.3 Parameter blocks [B]
Names are the JSON keys. "Stock" values are those of firmware D01.01.01 (for orientation only; the converter
reads them from the backup).

| Block | Parameters | Count | Stock |
|---|---|---|---|
| `aux_mix` | `usb` [L, R], `aux` [L, R] (linear) | 4 | 1, 1.86209 (+5.4 dB) |
| `input_gain` | [L, C, R, LFE, Ls, Rs] (linear) | 6 | 3.3338 ×5, LFE 6.6676 |
| `centre_mix` | `direct` [L, R], `centre` [L, R] | 4 | 0.755355, 0.5 |
| `surround_mix` | `surround` [L, R], `third` [L, R] | 4 | 1, 0 |
| `bass_sum` | `direct` [L, R], `array` [L, R], `surround` [L, R], `lfe` | 7 | 1, 1, 1.25893 ×4, 1 |
| `driver_eq.direct` | set: gain + 8 sections | 33 floats | |
| `driver_eq.array` | set: gain + 8 sections | 33 | |
| `driver_eq.surround` | set: gain + 8 sections | 33 | |
| `driver_eq.bass` | set: gain + 8 sections | 33 | |
| `bass_phase` | set: gain + 1 section (all-pass) | 5 | gain 0.940840 |
| `surround_net` | `delay_in` [L, R]; `pre` (gain only); `stage1` (8 sections); `cross1` delays [2]; `stage2` (8); `cross2` delays [2]; `stage3` (2) | 76 floats + 6 delays | delays 0, 60, 37, 37, 8, 8 |
| `front_net` | `direct` (1 section, on direct L / R); `array_a` (1); `array_x` (2, cross-feed); `stage_b` (1); `stage_b_x` (1, cross-feed); `delay` [2]; `stage_c` (2); `stage_c_x` (1) | 43 floats + 2 delays | delays 1, 1 |

Totals: 281 floats + 8 delays. Every set in `surround_net` / `front_net` is one coefficient set applied to both
channels of its stage (stock: one set per module).

### 2.4 The voicing sections (driver_eq) and their reading
All voicing stays in the base (decision 2026-11-01): on the satellites no section is exactly movable to the
profile output bands (sets direct / array / surround reach several outputs through the cross-feed networks,
with no common factor); on the bass only two are, and moving them gains nothing. The "Bose d'origine"
profile EQ is therefore **flat**. The roles below are an interpretation (pole / zero positions, from the
voicing analysis); they are written as `about` annotations by `oc base export` and ignored on import.

| Set | Voicing (tone shaping) | Cut-off (driver protection / band limit) |
|---|---|---|
| `direct` (inner + outer direct path) | s0, s1 broad tilt (real poles); s2 dip 7.1 kHz; s3 12.6 kHz; s4 2.4 kHz; s5 +5 dB around 660 Hz | s6, s7: zeros 27 / 40 Hz on the unit circle, poles 233 / 194 Hz (≈ −85 dB at 40 Hz). **s6 zeros at r = 1.0018 (outside)** |
| `array` | s0 HF shelf (−13 dB, 8 kHz); s1 shelf 2.8 kHz; s2 dip 5.6 kHz; s3 dip 3.2 kHz; s4 around 600 Hz | s5 (zeros 20 Hz, poles 412 Hz), s7 (39 / 209 Hz); s6 shapes the +9.5 dB bump at 230 Hz (zeros 416, poles 255 Hz) |
| `surround` | s0 dip 3.2 kHz; s1 6.7 kHz; s2 7.6 kHz; s4 1.4 kHz / 735 Hz; s6 small, 600 Hz | s3 (27.6 / 955 Hz), s7 (40 / 215 Hz); s5 shapes the bump (428 / 267 Hz) |
| `bass` | s5 (+7 dB low shelf, 103 Hz), s6 (76 / 49 Hz) | s0 (real zeros: **r = 1.003 at DC**, 0.085; poles 218 Hz), s1 (zeros at DC, poles 96 Hz); s2, s3 upper low-pass (zeros 1.5 kHz / 876 Hz, poles 208 / 150 Hz); s4, s7 sub-sonic shaping (**s7 zeros r = 1.0005**) |
| `bass_phase` | none | all-pass: zeros at r = 1.041 / 1.017 by construction |

Consequences for the format: zeros may lie **outside** the unit circle and many sections are not RBJ shapes
(best RBJ fits are 0.1 to 3.8 dB off, several with > 90° phase error), so every section accepts the exact
`raw` and `zpk` forms besides the RBJ ones (section 3.3). Editing a cut-off section changes what the satellite
drivers receive below ~200 Hz; `oc base check` treats that as a safety rule (section 4.2).

## 3. JSON format (`*.ocbase.json`)

### 3.1 Top level
```json
{
  "format": "opencompanion-base",
  "version": 1,
  "topology": "c5-stock-1",
  "name": "Bose d'origine",
  "source": { "kind": "stock-backup", "firmware": "D01.01.01", "backup_crc32": "0x…" },
  "sample_rate": 48000,
  "aux_mix":      { "usb": [1, 1], "aux": [1.86208713, 1.86208713] },
  "input_gain":   [3.33380365, 3.33380365, 3.33380365, 6.66760731, 3.33380365, 3.33380365],
  "centre_mix":   { "direct": [0.755355239, 0.755355239], "centre": [0.5, 0.5] },
  "surround_mix": { "surround": [1, 1], "third": [0, 0] },
  "bass_sum":     { "direct": [1, 1], "array": [1.25892544, 1.25892544],
                    "surround": [1.25892544, 1.25892544], "lfe": 1 },
  "driver_eq": { "direct": SET, "array": SET, "surround": SET, "bass": SET },
  "bass_phase": SET,
  "surround_net": { "delay_in": [0, 60], "pre": SET, "stage1": SET, "cross1": [37, 37],
                    "stage2": SET, "cross2": [8, 8], "stage3": SET },
  "front_net": { "direct": SET, "array_a": SET, "array_x": SET, "stage_b": SET, "stage_b_x": SET,
                 "delay": [1, 1], "stage_c": SET, "stage_c_x": SET }
}
```
`source` is informative (no identifier of the user or the speaker; the CRC-32 of the backup only). Unknown keys are an
error, except `about` anywhere (annotation, ignored).

### 3.2 Set
```json
{ "gain": 1.29274762, "sections": [ SECTION, … ] }
```
- `gain`: linear, may be negative (polarity) or 0 (mute). Alternative `"gain_db": x` with optional
  `"invert": true`; exactly one of `gain` / `gain_db`.
- `sections`: exactly the count of the topology (8, 1, 2… see 2.3). Use `"identity"` for an unused slot.

### 3.3 Section forms
Bose convention (as `bg_f`, `params.h` raw bands), monic:
`H(z) = (1 + b1 z⁻¹ + b2 z⁻²) / (1 − a1 z⁻¹ − a2 z⁻²)`; the set gain carries the scale.

| Form | JSON | Notes |
|---|---|---|
| raw | `{ "raw": [b1, b2, a1, a2] }` | exact; numbers are float32 values written as the shortest decimal that reads back to the same float32. **The only form guaranteed bit-exact.** Export default |
| pole / zero | `{ "zpk": { "zeros": PAIR, "poles": PAIR } }` | `PAIR` = `{ "f_hz": f, "r": r }` (conjugate pair at angle 2πf/48000, radius r) or `{ "real": [x1, x2] }` (two real roots). Zeros: any radius (outside the unit circle allowed). Poles: see 4.1. Exact up to one float32 rounding |
| RBJ | `{ "peak": { "f_hz", "q", "gain_db" } }`, `low_shelf` / `high_shelf` (`f_hz`, `q`, `gain_db`), `lowpass` / `highpass` / `bandpass` / `notch` / `allpass` (`f_hz`, `q`) | Audio EQ Cookbook at 48 kHz, designed in f64 by the app; the leading coefficient b0 is folded into the set gain (sections stay monic) |
| identity | `"identity"` | b1 = b2 = a1 = a2 = 0 |

Units: `f_hz` Hz (1 ≤ f ≤ 23 999), `q` dimensionless (0.05 ≤ q ≤ 50), `gain_db` dB (−60 ≤ g ≤ +30 for RBJ),
delays in **samples at 48 kHz** (integers).

### 3.4 Export
`oc base export` writes `raw` sections with `about` annotations: role (table 2.4) and zeros / poles (f, r)
(`--no-about` leaves them out; the best RBJ approximation is not written yet). `--form zpk` writes pole / zero
forms instead (readable; read back within 4e-7 relative of each coefficient, not bit-exact).

## 4. Validation

### 4.1 Hard rules (host `oc base check` and DSP, before anything is applied)
| Rule | Bound |
|---|---|
| every number finite | |
| poles | radius ≤ 0.99999 (`RAW_POLE_MAX`); DSP test on the coefficients: \|a2\| ≤ 0.99998, \|a1\| < 1 − a2 |
| numerator | \|b1\| ≤ 8, \|b2\| ≤ 8 |
| set gain | \|g\| ≤ 16 |
| scalar gains (`aux_mix`, `input_gain`, `centre_mix`, `surround_mix`, `bass_sum`) | \|x\| ≤ 8 (+18 dB) |
| delays | integers 0..95 (shorter than one 96-sample frame; scaled at 44.1 kHz) |
| topology / counts | `topology` = `c5-stock-1`, section counts as 2.3 |
| encoded size | ≤ capacity (section 5) |

### 4.2 Safety rules (host only; `oc base check`, also run by `listen` / `install`)
Computed with the small-signal response model of the base (`oc-device` response code, same as the stock model):
- **Satellite low end**: for each satellite output, |H| at 20–80 Hz must not exceed the stock base by more than
  1 dB (the stock cut-off protects the small drivers). Without a stock backup at hand the reference is the
  built-in table of stock magnitudes at 20, 30, 40, 50, 60, 80 Hz (generated from the same converter, numbers
  only).
- **Peak path gain**: for every input → output pair, max |H| over 20 Hz–20 kHz ≤ stock max + 6 dB.
- `--expert` turns these two into warnings. The hard rules never relax.
The post-stage protection (stock limits) runs after every base and is not configurable.

## 5. Binary encoding (EEPROM layout v4)

### 5.1 Location
Layout v4 reuses the bytes no profile record uses (map unchanged, `boot/docs/LAYOUT.md`):
| Area | Addresses | Bytes |
|---|---|---|
| base page 0 (header + payload) | graph page 0 = 0xC480 | 16 + 112 |
| base pages 1..7 | graph pages 1..7 = 0xC780, 0xCA80, 0xCD80, 0xD080, 0xD380, 0xD680, 0xD980 | 7 × 128 |
| base tail A / B | 0xDA10..0xDA7F, 0xDA90..0xDAFF (config pages after the 16-byte `OCC1` record) | 2 × 112 |
Capacity: 1,232 payload bytes. Topology 1 uses **1,132** (281 × 4 + 8): 100 bytes spare.
The `.bose_tab` program tables (layout v3) move into the image body (fixed topology), so the graph pages are free.
Config writes stay 16-byte writes at the page start: an AT24C512 page write only changes the bytes sent.

### 5.2 Header (base page 0, little-endian)
| Off | Size | Field |
|---|---|---|
| 0 | 4 | magic `OCB1` |
| 4 | 1 | format version = 1 |
| 5 | 1 | topology = 1 |
| 6 | 2 | payload length (1,132) |
| 8 | 4 | sequence (previous + 1) |
| 12 | 4 | CRC-32 (zlib) of header bytes 0..11 then the payload |

### 5.3 Payload (topology 1)
Float32 little-endian, in this order, then the delays as u8:
`aux_mix` (usb L, aux L, usb R, aux R) · `input_gain` (6) · `centre_mix` (centre L, direct L, centre R, direct R)
· `surround_mix` (surround L, third L, surround R, third R) · `bass_sum` (7, order of 2.3) · `driver_eq` sets
direct, array, surround, bass (each: gain, then b1 b2 a1 a2 × 8) · `surround_net` pre (gain), stage1, stage2,
stage3 · `front_net` direct, array_a, array_x, stage_b, stage_b_x, stage_c, stage_c_x · `bass_phase` ·
delays: surround_net delay_in L, R, cross1 ×2, cross2 ×2, front_net delay ×2.
(The mixer pairs follow the stock storage order so that the firmware copies runs; the firmware holds the map
from payload offsets to its coefficient array as a short run table.)

### 5.4 Commit (power-loss safe)
Firmware job, from the staged and already validated payload:
1. write 16 zero bytes over the header (page 0), read back;
2. write pages 1..7 and the two tails, read each back;
3. write page 0 (header + first 112 payload bytes) in **one** page write, read back.
Power lost before step 3 ends: the header is zero or its CRC fails. Boot reads the header, the payload and
checks the CRC, the topology and the 4.1 rules; any failure → **no base**: profiles play on the built-in neutral
program, `OPI` flags bit 3 (as today), `OBI` reports why. Audio never depends on a half-written base. No A/B
copy: the app keeps the JSON and re-installs it (an A/B pair needs ~2.3 KB, which the EEPROM does not have).
Wear: ~11 page writes per install, AT24C512 endurance 10⁶ cycles per page: irrelevant.

## 6. Firmware behaviour
- Boot: base = EEPROM base if valid, else none. `PF_STOCK_GRAPH` means "play on the installed base"; every
  profile sets it, "Bose d'origine" included. No base → neutral program (built in, unchanged: its sections are
  the [F] ones, kept in the body).
- Headphones: neutral program, as today.
- Coefficients: the payload floats are copied into the program coefficient array, then `bose_graph_rate()`
  re-maps them as today: identical numbers give identical audio (bit-identity target, section 9).
- Delay lines: 96 samples reserved per line (8 lines, `.bose_ram`, not in the image) so any delay 0..95 fits.
- Console (OP protocol 6):

| Command | Reply | Notes |
|---|---|---|
| `OBW <off:LE16> <1..55 bytes>` | `06` | write into the RAM staging buffer (header + payload, 1,248 bytes) |
| `OBA` | `06` / `15 <code>` | check the staged header + payload (CRC, topology, 4.1), then apply: 2 ms fade out, swap, fade in. Not saved ("listen") |
| `OBV` | `06` / `15 01` | save the applied base (job 5.4, asynchronous; poll `OBI`) |
| `OBX` | `06` | drop a listened base: back to the EEPROM base (or neutral) |
| `OBI` | `06` + status | source in use (0 none, 1 EEPROM, 2 listened), header of the base in use (sequence, CRC, topology), last check / job result, EEPROM busy |
| `OBR <off:LE16> <n>` | `06` + n bytes | read back the base in use (for `oc base status --dump`) |

Codes as `OP*` (`15 07` bad argument / rule, `15 04` CRC). One `E` round trip ≈ 40–50 ms: 1,148 bytes = 21
`OBW` ≈ 1 s, apply < 2 frames, save ≈ 0.15 s.

## 7. CLI (`oc base …`)
| Command | Does |
|---|---|
| `oc base export [--backup <u7001.bin>] [-o base.ocbase.json] [--form raw\|zpk]` | converts the stock program of the user's backup (default: the oldest verified stock backup in the OpenCompanion backups folder, as the response curves) into a base JSON. Offline |
| `oc base check <file> [--backup <u7001.bin>] [--expert]` | schema, 4.1, encoded size, bit-for-bit comparison with the stock base. Offline. (4.2 and the per-output deviation from stock: device stage) |
| `oc base listen <file>` | check, `OBW` ×n, `OBA`: plays it now, not saved; `oc base listen --stop` = `OBX` |
| `oc base install <file>` | check, upload, `OBA`, `OBV`, poll `OBI`, read back with `OBR` and compare. Progress on the console |
| `oc base status [--dump <file>]` | `OBI`; `--dump` reads the base in use back to JSON (raw form) |
Exit codes: 0 ok, 2 invalid file, 3 safety rule, 4 device / link error.

## 8. Split base / profile / firmware
| Item | Where |
|---|---|
| Voicing and cut-off sections, spatial / centre / surround / bass mixes, array networks, delays | base |
| User EQ (global before the base, outputs after), trims, sub level, loudness on / off, link L/R, bypass, name, colour | profile (format unchanged, `params.h`) |
| "Bose d'origine" profile | flat EQ, loudness on (`profile_bose.c`) |
| Loudness law, spatial law, BADDATA, input level, protections, sub-level source, device options | firmware |

## 9. Validation plan
1. **Converter golden test** (`oc-device`, `cargo test`, `--target-dir target/base`): backup → base → payload
   floats equal, bit for bit, to `bg_f` at the mapped indices; the stock tables generated by
   `re/dsp/gen_bose_graph.py` from the same backup are the reference. Needs the local reference backup
   (`backups/2026-10-31/u7001_dsp.bin`, never committed); skipped with a message if absent.
2. **JSON round trip**: export → import → payload identical (raw form); zpk form within one float32 ulp.
3. **Firmware host test**: `test/host_engine.c` and `re/dsp/bose_graph_check.py` with the program built from
   the payload: output samples bit-identical to the `bld_e` build (DC blocker removed included), then the existing
   stock_sim criteria.
4. **Hardware** (RAM loader first): `OBA` / `OBX` with the exported base (no audible change, `CPU` unchanged or
   lower), then `OBV`, power cycle, `OBI` = EEPROM; corrupt the header page on a test unit → neutral fallback.

## 10. Firmware impact (estimates, to be measured with `oc-bootimg sizes`)
Image body (LZ4): `bg_f` stock values replaced by a mostly-zero template −1.0 KB; `bose_tables.c` −0.45 KB;
`BG_BASS` constants become base data; topology tables back in the body +0.6 KB; loader / checks +0.4 KB;
`OB*` commands and save job +0.5 KB. Net ≈ 0 ± 0.3 KB against a 252-byte margin (bld_e): the console part may
need cuts elsewhere. CPU: unchanged per frame (same coefficients, same kernels), −6 channel-sections with the
DC blocker gone.
