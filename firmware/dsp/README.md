# firmware/dsp — TMS320DA705 (C67x+)

Bare-metal C firmware for the Companion 5 DSP: boot, clocks, TDM audio in/out, TAS link, debug console, user EQ
and profiles, the stock speaker protection, the stock loudness and the boot-chain hooks (UPD, RSC, trial
confirmation). No DSP/BIOS, no ROM calls, no dependency on the Bose firmware.

Reverse-engineering references: `../../../re/dsp/SPEC.md`, `../../../re/dsp/notes/hardware.md`,
`../../../re/dsp/notes/control.md`, `../../../re/dsp/notes/bose_profile.md`, `../../../doc.md`.
Boot chain: `boot/README.md`, `boot/docs/{LAYOUT,RESCUE,UPDATE}.md`.

## Toolchain — important
The DA705 core is a **C67x+**. Only **TI C6000 CGT 7.4.x** (or earlier) can target it (`-mv67p`).
CGT 8.3 (installed in `ti-cgt-c6000_8.3.7/`) dropped C67x/C67x+: it can only build for C674x, which may
emit C64x+-only instructions (compact 16-bit, SPLOOP, ...) the DA705 cannot execute.

- `make` — real build, requires CGT 7.4.24 in `<project>/ti-cgt-c6000_7.4.24/`
  (download: https://www.ti.com/tool/download/C6000-CGT/7.4.24). `make STAGE=1|3|4` must stay warning-free.
- `make CHECK_ONLY=1` — compile/link check with CGT 8.3 (`-mv6740 --no_compress`). **Never load this image.**
- `make clean`

**Versions**: every delivered DSP image has its own semantic version (`FW_VERSION` in the Makefile, reported by
`SVM` as `OC<version>+<UTC build stamp>`), like the TAS firmware: a minor bump for a feature set, a patch bump for
each fix delivered after it; never two delivered images with the same version. 0.1.0 = up to bld_j; 0.2.0 =
profile switch behind the output ramp, `PF_ADVANCED`, raw biquad pairs removed, filtered pod knob byte in `S`;
0.2.1 = direction-aware hysteresis of the pod knob byte; 0.2.2 = an open stream ('R' bit0) forces HOST mode
and USB active whatever DSPINT reads (silent speaker after a restart with TAS 0.4.7); 0.2.3 = app sub level as a
device setting (`DEVL`, profile protocol 8); 0.2.4 = 48 kHz only, maximum level = stock (no −12 dB ceiling, no
−6 dBFS limiter), every hot loop pipelined again; 0.2.5 = a structural batch of OPW (band types, flags, EQ mode, profile written
by OPW) goes behind one output ramp, ended by `OPX` or 50 ms without a write; the output ramp moved to the
INPUT of the post-stage (with it at the DAC, the step of a switch was still in the 320-sample look-ahead delay
and reached the DAC as the ramp reopened: full-level pops on every profile / mode switch and unmute), switches
wait until the delay has flushed; a non-finite sample resets the signal path and never reaches the DAC (PCM
clamp maps NaN to 0). Host check `test/host_switch.c`. 0.2.6 = the band compressors cannot latch: their gain is multiplicative, so an
absurd input (NaN / Inf, or a filter blowing up near the float range) drove it to 0 (flushed denormal) and the
satellites stayed silent for good with a clean-looking output; a compressor gain below -100 dB (0.2.7; 0.2.6: -59 dB,
reached by the most extreme legal EQ) now resets the path like a non-finite sample, and every switch behind the ramp restarts the compressors at 1. Host check
`test/host_avance.c` (Avance edits, +12 dB bands, drags, an absurd frame) and `test/host_avance_full.c` (the
whole path with the DSP math, Bose graph, loudness, the app's OPW / OPX sequences).

Output: `build/opencompanion_dsp.out` (ELF) + `build/opencompanion_dsp.map`. EEPROM image:
`oc-bootimg build` (`app/crates/oc-ais`, see `boot/README.md`).

Build options that matter for the size (Makefile): `--opt_for_space=3` on every file except the hot block kernels
(`SPEED_SRCS` = `dsp/k_vec.c`, `dsp/k_biquad.c`: software pipelined, "CPU" below), `-O1` on the control-plane files (`CTRL_SRCS`: start-up, console, storage,
profiles, one-time hardware set-up), `--mem_model:data=far_aggregates` (scalars DP-relative), `--ram_model` with
our own start-up instead of the RTS one (`hw/reset.asm`, `hw/start.c`), RTS library linked last.

## Memory map (`lnk.cmd`)
| Range | Use |
|---|---|
| 0x10000000–0x10000FFF | unused (0x10000400–0x10000FFF reserved by the on-chip bootloader while loading AIS) |
| 0x10001000–0x10022FFF | firmware (today up to ~0x1001F1A0: vectors, code, data, then .bss/.far zeroed at start, 8 KB stack) |
| 0x10023000–0x10024FFF | reserved for the RAM loader (tail of the stock .cinit table); loader handoff record at 0x10024E00 |
| 0x10025000–0x1002AEFF | `IRAM_HI`: stock graph engine work RAM `.bose_ram` (11 KB from 0x10025000, uninitialised); rest free |
| 0x1002AF00–0x1002FFFF | boot chain (trampoline, boot1, rescue flag 0x1002DFF0, boot1 stack): never loaded by the firmware |

## Architecture (`src/`, one folder per area)
| File | Role |
|---|---|
| `main.c` | init order and main loop |
| **hw/** | |
| `reset.asm`, `start.c` | reset entry `_c_int00` (SP, DP, FADCR/FMCR as the RTS), zeroes .bss/.far, calls `main`; replaces the RTS start-up (~2.3 KB). Initialised data is loaded in place (`--ram_model`) |
| `vectors.asm`, `isr.c` | interrupt table (INT8 dMAX, INT9 McASP0 frame start, INT14 I2C, NMI), ISR wrappers |
| `pll.c` | PLL x20 → CPU 245.76 MHz, exact Bose sequence |
| `mcasp1.c`, `dmax.c`, `audio_io.c` | McASP1 TDM 8 × 32 bits, dMAX ping-pong (Bose configuration), frame handoff |
| `mcasp0_gpio.c` | McASP0 GPIO setup (measured stock values), DSPINT handshake, AXR0[14] drive / release |
| `i2c_slave.c` | I2C0 slave 0x29, TAS frames P/S/V/m/M/X/U/F/E/D/d/R |
| `ramp.c` | McASP0 ramp converter (stock 0x10019418, 0x1001A360, 0x100162B4, 0x10016420): pod knob %, bass knob, VRAW |
| `gpio_in.c` | McASP0 GPIO inputs: thermistors → thermal gain (0x1001B638), clip flags, BADDATA low-pass (0x10010758), final gain |
| **audio/** | |
| `audio.c` | frame sequence: input meters, control (modes, master gain + loudness, standby), engine, post-stage |
| `audio_out.c` | the single post-stage: stock protection, final gain, fault detector, full-scale guard, rate-switch fade, PCM |
| `loudness.c` | master gain and the stock dynamic loudness (0x1001AF08), the only loudness (PF_LOUDNESS) |
| `eq.c`, `eq_chain.c`, `eq_design.c` | user EQ banks (4 lanes per pass; stock crossover in the neutral output lanes), gains; click-free coefficient glide; RBJ design and band compilation |
| `biquad.c` | biquad cascades (Bose coefficient convention), 4 at once through the 4-lane kernel |
| `level_detect.c` | stock-scaled channel powers (loudness and standby detectors) |
| `rate.c` | 48 kHz only: the input layout switch (5.1 / 2.1 mode of the `R` frame) behind the output ramp |
| `meters.c` | peak / RMS meters (the `TON` test tone was removed for EEPROM space, layout v3) |
| **dsp/** | block kernels (`include/kern.h`): every per-sample loop of the audio path, no per-sample branch |
| `k_vec.c`, `k_biquad.c` | hot kernels, software pipelined: gain / multiply-add (scalar or per-sample gain), peak + power, 4-lane biquad section |
| `k_dyn.c`, `k_ctl.c` | lighter loops, built for size: stock soft clipper and compressor (4 lanes), smoother, delay line, limiter curve, PCM |
| **engine/** | `engine.c`: the single audio engine: preset choice, inputs, program run, user EQ banks, trims, sub level, headphone feed |
| **bose/** | the graph interpreter and its two programs ("Engine" below) |
| `bose_graph.c` | interpreter of a module program (gain, mixer, sum, delay, biquad, meter, user EQ bank ops) |
| `bose_progs.c` | the two presets: the stock program (Bose) and the built-in neutral program |
| `bose_graph_data.c` | GENERATED by `re/dsp/gen_bose_graph.py` from the stock RAM image: 66 ops, stock wires and parameters |
| `bose_tables.c` | program tables (`.bose_tab`) from the EEPROM graph data pages, CRC check (`include/bose_tables.h`) |
| `bose_spatial.c` | stock spatial control 0x10027978 (`re/dsp/notes/spatial.md`) |
| **protect/** | |
| `protect_band.c` | stock satellite band soft clippers, compressors, bass soft clipper (0x1002D0A8, 0x1002CB4C) |
| `protect_limit.c` | stock satellite look-ahead limiter (0x1002B020, 320-sample delay, ceiling 0.95) |
| `protect_supply.c` | stock VRAW ceiling (0x1001BB6C) and supply / clip fault latch (0x1001B820, 0x10015910) |
| `standby.c` | silence timer (stock 120 s; delay and threshold from the device options) → standby request in the `S` reply |
| **control/** | |
| `volume.c` | master volume (HOST / LOCAL mode), USB volume law, pod knob and pod mute reporting (`S` status) |
| `volume_curve.c` | stock percentage → dB curve (0x1001B140), USB and pod parameters |
| `spk_mode.c` | stock speaker / headphone mode sequencer (0x1001BF14, MD) and the AXR0[14] amplifier control (0x100157E8, 0x10015910) |
| `handoff.c` | adopts the TAS link state captured by the RAM loader (`include/handoff.h`) |
| `params.c`, `profile_bose.c` | active profile (layout `include/params.h`), validation; built-in "Bose d'origine" (Bose base, flat EQ) |
| `sub_level.c` | sub level and its source: rear knob (default, −12..+6 dB) or the profile's `sub_level_cdb`, smoothed |
| **storage/** | |
| `eeprom.c` | non-blocking I2C1 master for U7001, page policy (profiles + header / boot-state pages only) |
| `profiles.c` | 4 profile slots, A/B copies, CRC, boot slot, pod mute double press |
| `bootctl.c` | boot chain: `UPD`, `RSC`, trial confirmation (`OCBS`) |
| `crc32.c` | CRC-32 shared by profiles and boot records |
| **console/** | |
| `console.c` | `E` console: BRR/BWW and the general text commands |
| `console_params.c` | binary `OP*` profile commands |
| `console_inputs.c` | read-only `POD`, `PRT` (`VRW`, `GPI`, `STB`, `BAS`, `RMP` removed for EEPROM space: their values in `PRT`, or `BRR` of `g_supply`, `g_gpio`, `g_standby`, `g_ramp`) |
| `console_dev.c` | `DEV`: device settings (sub level source, device options); replaces `BSQ` / `BSS` / `R2Q` / `R2S` |
| `console_util.c` | shared text formatting |
| **util/** | `divf.c` (EABI float division, RCPSP + Newton, replaces the 672-byte RTS member), `mathx.c` (compact exp2 / log2 / sincos / sqrt replacing the RTS math, ~5 KB smaller), `libc_min.c` (byte-loop memset / memcpy, control code only) |

## Console commands (TAS `E` relay, replies < 58 bytes)
Text commands end with `\r`; replies end with `\r>` (input commands: `>`), errors are `\r!`. Levels are integer
dBFS, `-99` = silence.

| Command | Reply | Notes |
|---|---|---|
| `BRR` / `BWW` | `06` + data / `06`, `15` | binary memory read / write, Bose format |
| `SVM\r` | `D<dsp version>U<tas version>` | versions |
| `CPU\r` | `LD:<%> MX:<%> T:<ticks> LF:<n>` | **measured** CPU load (RTI counter 1, `audio_io.c`): average and maximum (since the previous `CPU`) time in `audio_process()` as % of the frame period, frame period in RTI ticks, frames lost (a frame arrived before the previous one was processed). `OPI` CPU % is the same average |
| `SYS\r` | `FR:<frames> CL:<clips> VOL:<dB> MU:<mutes> PK:<in>/<out>` (`\r`-separated) | `VOL` = USB volume after its law; `MU` bit0 USB, bit1 pod; `PK` highest input / output peak |
| `LVI\r` / `LVO\r` | 8 / 6 comma-separated peaks | peak hold since the previous read, then reset. Inputs: USB L,R,C,LFE,Ls,Rs, AUX L,R. Outputs: inner L, inner R, outer L, outer R, bass A, bass B |
| `POD\r` | `PR:<0/1> KN:<knob %×10> MD:<H/L> RK:<reported %> EV:<events> DB:<master dB×10> LB:<loudness dB×10>` | pod present, raw knob after the stock hysteresis, HOST / LOCAL mode, knob as reported in `S`, event counter, master volume before the ceiling, loudness boost |
| `DEV\r` | `SR:<0/1> KN:<dB×10> AC:<0/1> LY:<1/2> OP:<0..255>` | device settings (`OCC1` config record, "Device settings" below): sub level source (0 = rear knob, default; 1 = app: the profile's `sub_level_cdb`), the rear knob as a sub level in dB×10 (−120..+60, exactly the gain applied in knob mode; always read), whether 2.1 routing is in effect now (TAS 2.1 mode, neutral preset), input layout (1 5.1, 2 2.1 mode), device options byte |
| `DEV<k><v>\r` | as `DEV` + ` R:<result>` | sets one device setting, applied at once and saved in the config record: `DEVB0` / `DEVB1` sub level source (smoothed), `DEVO<0..255>` options byte (decimal). `R:0` saving (poll `OPI`), `R:5` applied but not saved (config area not ours). `\r!` = busy (EEPROM job running) or bad argument: nothing changed |
| `PRT\r` | `FT:<0/1> VR:<mV> T:<t1><t2> CC:<n> BC:<n> CG:<dB×10> LG:<dB×10>` | supply / clip fault latched, VRAW = 5.1 + 3.3·R/0.43 V, thermistors (1 = hot), frames with CCLIP / BSCLIP (amplifier clip flags), lowest compressor gain, lowest look-ahead limiter gain (last frame). Since proto 5 it carries what `VRW` / `GPI` gave the app (`VRW`, `GPI`, `STB`, `LRI`, `LRO` removed for EEPROM space) |
| `UPD\r` | `UPD\r>` or `\r!` (busy) | only if 0x0100 holds a valid `OCI1` header (refused on the stock EEPROM, `BST` R:5): invalidates it (128 zero bytes, read back), then reboots: boot1 → rescue, reason 3 |
| `RSC\r` | `RSC\r>` | writes the rescue flag (0x5352434F, 0xACADBCB0 at 0x1002DFF0) and reboots |
| `BST\r` | `BS:<state> R:<result> T:<trial> C:<cold>` | boot-chain status: state 0 idle; result 0 none, 1 busy, 2 OK, 3 I/O, 4 verify, 5 refused (no OpenCompanion header); trial 0 waiting, 1 no header, 2 not a trial image, 3 already confirmed, 4 confirmation written, 5 skipped (RAM-loaded), 6 error; cold 1 = started by boot1 / ROM |
| `RST\r` | `\r>` | reboot to the ROM boot loader (EEPROM image) |

## Signal path (one engine, `src/engine/`, `src/bose/`)
```
inputs (USB 6 slots, AUX L/R) -> preset program (graph interpreter):
   Bose:    the stock graph 0x100106C8 .. 0x10011408 (user global EQ bank before it on L / R, output EQ bank after it)
   neutral: USB mute -> 5.1 downmix matrix -> BADDATA selector -> + AUX x aux gain -> master volume + stock loudness
            -> global EQ bank (L, R) -> routing L, R, L, R, LFE + (L + R) / 2 -> output EQ bank (+ stock crossover)
-> trims; bass: x sub level (knob or app) -> bass A = bass B
-> post-stage, once: satellite band soft clipper -> compressor -> look-ahead limiter 0.95 (320 samples);
   bass compressor -> soft clipper (neutral: same 320-sample delay) -> final gain (VRAW ceiling x thermal)
   -> fault detector -> full-scale guard -> output ramp -> DAC
```
`bypass` keeps the preset: it empties the user EQ banks and turns the loudness off; crossover, protection,
limiters, ceiling and mutes always stay. Coefficient changes cross-fade over one frame (2 ms).

## Engine: presets, programs, banks
One data-driven engine for every profile: `engine.c` picks a **preset**, a program run by the graph interpreter
(`bose_graph.c`, ops gain / mixer / sum / adder / router / delay / biquad / meter / spatial / user EQ bank):
- **Bose** (`PF_STOCK_GRAPH`, graph tables present): the stock program, op tables in the EEPROM graph data pages
  (`bose_tables.c`, layout v3), coefficients `bg_f` in the image body (`bose_graph_data.c`, generated by
  `re/dsp/gen_bose_graph.py`). The user EQ banks run around it (global EQ on the inputs L / R, output EQ on its
  5 outputs); the bass is not delayed (stock timing, 6.7 ms ahead of the satellites). Missing or wrong tables:
  the profile plays on the neutral preset, `OPI` flags bit 3.
- **neutral** (otherwise, and while the headphone feed is on): `bose_progs.c`, 16 ops, built in; its coefficient
  sets are the stock ones taken from `bg_f` (BADDATA LR4 low-pass, loudness band-pass, selector pole). The input
  matrix is a run-time weight table (5.1: L + 0.707 C + 0.707 Ls...; 2.1 mode: L, R only); "routage 2.1" (2.1
  mode) is a bank variant: the outer lanes use the inner bands. The bass gets the 320-sample limiter delay.
- **User EQ banks** (`eq.c`): bank 0 = L, R (target 0), bank 1 = inner L, inner R, outer L, outer R, bass
  (targets 1..5). Lanes run 4 at a time through the 4-lane section kernel. On the neutral preset each output lane
  ends with the stock crossover (satellites: high-pass 0x10009D6C twice; bass: low-pass 0x10009C58), always on.
  The user crossover frequency (60..250 Hz LR4) is gone: `crossover_hz` is reserved (0 = stock). Raw biquad pairs
  (types 7 / 8) are gone (profile protocol 6): params_sanitize() turns them off. No CPU cap of the user EQ any more (`BOSE_EQ_SECTIONS` removed, proto 5): the
  banks run 4 lanes per pass, 12 bands on every target cost ~8 % of the frame.
- **Click-free EQ updates** (`eq_chain.c`): a new coefficient set glides from the running one over 16 frames
  (32 ms), along a smoothstep, the states running on: section coefficients linearly (the stability triangle of
  (a1, a2) is convex: every step is stable), the cascade gain in the log domain (a linear gain path with a
  low-pass appearing gave +100 dB at DC halfway). Added / removed sections glide from / to the identity. A new
  write during a glide restarts it from the current coefficients (last write wins). Glides only between sets
  of the same structure: when a section count changes (band on / off) or a whole profile is installed (load,
  pod double press, `OPK`, `OPD`), sections are re-packed and a glide between unrelated sections boosts the bass
  (the "pocs" heard on hardware with bld_e): the switch goes behind the output ramp instead (old set plays on
  while it closes, then the new set is installed at rest, ramp opens; `engine_profile_swap`). Host check
  `test/host_profile_swap.c` + `test/profile_swap_check.py` (two profiles with very different bass EQ, 40 / 50 /
  60 Hz sine): corner 5.4 / 2.2 / 3.7 behind the ramp (bound 10), 198 / 140 / 187 with a glide. No cost while nothing
  changes; preset and rate switches, done while the outputs are silent, end the glide at once. Host check
  (`test/host_eq_glide.c` + `test/eq_glide_check.py`, 50 Hz sine, flat -> +12 dB bell at 50 Hz Q 1, then moved to
  63 Hz): largest second difference 6.1 and 2.3 times that of the steady tone (bound: 10; a corner of ~-72 dB
  re the tone); the former update (new set started from copied states, 2 ms cross-fade) gave 14.9 and 3.8.
- Preset switch (profile or headphones): 2 ms fade out, engine reset to the stock power-up state, fade in.
- Control inputs of both programs: USB mute ramp (0x100106C8), master volume + loudness gains (`loudness.c`,
  0x100103FC / 0x10010430), BADDATA (`g_gpio.sel` -> mixer 0x10010818); mode mixers fixed to 'N'.
- **Bass compensation**: the stock knob stage 0x10010938 (all-pass + polynomial(knob) x LPF4) is not run any
  more; only its fixed all-pass branch 0x100108C0 stays in the Bose program, because it sets the bass phase
  against the satellites (without it the host check shows the bass in opposite polarity). The knob is the sub
  level ("Sub level" below). Deviation from stock: at the knob centre the stock adds -0.01 x LPF4 (-40 dB).
- Validation (host, `re/dsp/bose_graph_check.py`: firmware C built with gcc vs `re/dsp/stock_sim.py`):
  satellites unchanged (max sample error <= -62.5 dB re peak, magnitude <= 0.013 dB, phase <= 2 deg); bass
  -42 dB (the dropped -0.01 x LPF4 term), magnitude within 0.16 dB (-20 dB window), phase 0.7 deg. The neutral
  program with the real EQ banks (`test/host_engine.c`) matches a numpy model of the chain within -94 dB.
  The graph meter now uses two partial sums (the spatial detection rounding differs in the last bits).
- Level: the USB channels enter the Bose program at the stock input margin 0.340408 (`usb_level` of the program),
  so both presets play at about the same level for the same OS volume.
- RAM: 11 KB of graph work buffers (`.bose_ram`) and the block scratch buffers / limiter lines (`.dsp_ram`,
  `kern.h` `DSP_SCRATCH`) in IRAM_HI, uninitialised, not in the image.

## Volume, pod and the TAS protocol (OpenCompanion design)
The pod knob reaches the volume through the TAS: the `S` reply's knob byte is the **stock-filtered** knob
(`volume_curve.c` `pod_filter`, the stock 0x100162B4 / 0x10016420: one-pole 0.05 / 0.95 per 2 ms frame, x 99 %,
snap / hold 0.25 / 0.75 %, 1 % steps), then a direction-aware hysteresis (`knob_step`: 1 % steps while the
knob turns the same way, 4 % of travel to turn back; since 0.2.1, the former +-0.75 % band around the rounded
value let +-2 % of noise walk the byte over 76..79 % on a still knob), never the raw IVOL reading. The same value
is the volume in LOCAL mode. Host check `test/host_knob_byte.c`. The standby detector scales with the USB volume only, as stock (detector after the USB gain).
**Knob valid** (`S` flags bit 3, 0.2.4): only with `X` != 0 (cleared at once by `X 00`), a settled filter, fresh
ramp frames, and a plausible IVOL reading (`ivol_plausible`: no jump over 12 counts per frame, never above full
scale + 6; invalid within a frame of a floating input, valid again 0.5 s after it settles; host check
`test/host_ivol.c`). A floating input that settles at a constant level cannot be told from a pod at rest: there
the TAS (`X`, MUTE line) decides. The RAM-loader handoff no longer adopts the stock pod presence.
The OS volume and mute are the **single master volume**. The stock rule "no pod = pod volume 99 %" is gone, and
in USB mode the DSP never multiplies the pod percentage into the gain.

- **HOST mode** (a USB host is active): master = USB volume `V` (UAC dB, in every profile; the former stock %
  curve of `PF_VOL_STOCK` is ignored since layout v3). The pod knob and the pod mute are **reported** to the TAS, which
  forwards them to the host; the host moves the OS slider / mute and sends the usual UAC SET_CUR back (`V`, `m`).
- **LOCAL mode** (no USB host, e.g. AUX only): master = pod knob through the stock pod curve (0x1001B140 with
  the 0x10010564 parameters: 99/35/4 %, floor −71.3 dB). Without a pod the master is the last USB volume.
- **Open stream first** (0.2.2): while the last `R` frame says the stream is open (layout bit0, TAS >= 0.4.0),
  the mode is HOST and the USB channels play, whatever DSPINT reads. Otherwise:
- **Mode detection** = the stock signal: after `U`, the TAS drives DSPINT (P1.6 → DSP AXR0[9]) low while USB is
  active (TAS `PollUsbVbusAndDspPower` 0x0FD7); the stock DSP reads the pin every frame (0x10015700..0x100157D4)
  and mutes the USB channels while it is high (0x10010108 = 1.0). Here: pin high for 1 s → LOCAL, low → HOST;
  the USB channels are muted at once while it is high (stock). Before `U` the mode is HOST. Works with the stock
  TAS and with ours, no new message.
- **Mutes**: the USB mute `m` mutes the USB channels only (stock 0x100106C8 path), so **the USB mute does not mute
  AUX** (stock behaviour; the earlier OpenCompanion firmware muted everything). The pod mute `M` (mute touch or
  headphones) mutes the speakers in both modes (stock mode selector 0x1001BF14). The master volume acts after
  the USB + AUX mix, so the OS volume does control AUX.
- **Pod presence** = `X` byte ≠ 0 (stock 0x1001494C). `X`, `U`, `M`, `m`, `V` are only sent at link start-up and on
  change (doc.md, InitDspLink): a firmware started by the RAM loader never received them, hence `PR:0` and the
  forced 99 % seen on 2026-11-01. The RAM loader now copies the stock state (0x10006D61 pod present,
  0x10006D60 USB up, 0x10010100 USB %, 0x10010108 USB mute, 0x10010114 pod mute) to a handoff record at
  0x10024E00 before anything is overwritten (`include/handoff.h`), and the firmware adopts it once at start.
  **Rebuild the loader** (`loader/`, `make`) before the next RAM test.
- **Knob conversion**: instruction-for-instruction identical to stock 0x100162B4 / 0x10016420 (leading ones from
  slot 1 over up to 5 slots, full-scale count from the calibration frame, clamp to full scale, one-pole 0.05 / 0.95,
  ×99, snap below 0.25 % / hold 0.25..0.75 % / 1 % up to 1 %). `KN:215` vs stock 12.47 % is not explained by the
  code; compare the same knob position with `tools/verify_hw.py` (stock ramp object 0x100136B0: IVOL ratio
  0x100137A4, knob % 0x100137AC, full scale 0x10013794) and `POD` (+ `BRR` of `g_ramp`) under OpenCompanion (test plan step 3).
  Bass knob scale also identical to stock (0x10016690..0x100167B0: value = −5/3 × (BEQ− − BEQ+)/(BEQ− + BEQ+),
  ±1 outside |ratio| ≤ 1).

**`S` status reply** (one 8-byte frame, the TAS reads it as before):

| Byte | Content |
|---|---|
| 0 | standby request (stock 0x10006D5E): TAS enters standby if ≠ 0 |
| 1 | fault (stock 0x10006D5F): supply / clip fault latched (also standby) |
| 2 | flags: bit0 pod present, bit1 pod muted (`M`), bit2 LOCAL mode, bit3 knob valid, bit7 = 1 (extended OpenCompanion status; the stock DSP replies 2 bytes) |
| 3 | pod knob 0..99 % (stock conversion + 1 % hysteresis) |
| 4 | event counter, +1 (mod 256) whenever byte 3 or the pod mute changes |

**What the TAS firmware must do (host side, not implemented here)**: poll `S` every ~50–100 ms while USB is
active (the stock TAS only polls every ~2 s while USB is inactive, PollDspStatus 0x1128); when byte 4 changes,
report the change to the host: either a HID Consumer Control interface (Volume Increment 0xE9 / Decrement 0xEA
steps towards the knob position, Mute 0xE2 toggles when the pod mute differs from the Feature Unit mute;
`firmware/usb/README.md` "Volume and mute reported to the OS"), or a UAC status interrupt (UAC2 hosts only), or
the OpenCompanion HID interface with an app that sets the OS volume to the absolute knob %. The OS then sends
SET_CUR → `V` / `m` as usual. After `U`, the TAS should also re-send `R` (the current rate), since the DSP only
learns the rate from that frame.

## Sampling rate (48 kHz only) and input layout
48 kHz only since 0.2.4 (TAS 0.5.1 dropped 44.1 kHz): the `R` frame is still accepted, its rate is ignored (a
44.1 kHz `R` changes nothing), and the 44.1 kHz support (coefficient re-mapping, re-timed smoothers, `FS4` /
`FS8`) is gone.
- Layout byte (TAS 0.4.0): bit0 stream open, bit1 **2.1 mode**; the mode alone selects the layout (`rate.c`;
  TAS 0.3.0 values 1 / 2 / 3 map to 5.1 / 2.1 / 2.1). 5.1 mode: 6 x 16-bit slots L, R, C, LFE, Ls, Rs. 2.1 mode:
  24-bit stereo (L, R); C, LFE, Ls, Rs are not used.
- Bose preset: the stock graph in both modes (stock routing). Neutral preset: 5.1 = downmix L + 0.707 C +
  0.707 Ls, R + 0.707 C + 0.707 Rs, LFE + (L + R)/2 to the bass; 2.1 mode = **2.1 routing**: each satellite plays
  one signal on both drivers (the outer lanes use the inner bands, each driver keeps its trim) and the bass gets
  (L + R)/2, through the crossover.
- The former "routage 2.1" device setting is gone (protocol 5: `OPU` and `DEVR` removed, config record +10
  reserved): the TAS `R` frame is the only source of the 2.1 mode.
- While the TAS switches modes it holds DSPINT high ~0.5 s: the HOST / LOCAL volume mode needs 1 s of a stable
  pin (`volume.c`), so the volume source does not flip; the USB channels are muted at once while it is high.

## Boot chain integration (`boot/docs/RESCUE.md`, "Changes needed in the DSP firmware")
| # | Item | Status |
|---|---|---|
| 1 | profiles at 0xC200, config 0xDA00 / 0xDA80 | done (`include/profiles.h`) |
| 2 | EEPROM write policy | `storage/eeprom.c`: 0xC200..0xDAFF, plus the single pages 0x0100 (header: zero bytes only) and 0x0180 (boot-state); a write never crosses a page; nothing else |
| 3 | `oc_eeprom_layout_v2` | exported (`storage/profiles.c`), `oc-bootimg build` accepts the ELF |
| 4 | `IRAM_HI` cut at 0x1002AF00 | `lnk.cmd` |
| 5 | `UPD`, `RSC` | `storage/bootctl.c`, console table above |
| 6 | trial confirmation | after `U` and 10 s of audio frames (5000), only on a cold boot (never from a RAM-loaded image): reads the header (`OCI1`, CRC), if `HF_TRIAL` reads `OCBS`, writes `OCBS, seq, attempts, 0xA5, 0, 0, CRC-32` at 0x0180 and reads it back; `BST` shows the outcome |
| 7 | cold-boot path | unchanged (`mcasp0_gpio_init()` sees McASP0 at reset values and does the DSPINT handshake) |
| 8 | no raw EEPROM write behind `D` | `D` / `d` answer `15` |
`boot/test/run_host_tests.sh` passes with this ELF (RAM image identical, 17 corruptions → rescue, trial counter).

## Output level and safety
- Warm start without the TAS handshake (JUMP from rescue, DSP-only reset; the TAS does not resend `V`, `M`, `X`,
  `R`): the master volume is −30 dB (`VOL_SAFE_DB`) until the first `V`. A RAM-loader start (warm, McASP0 not
  reset) adopts the stock volume from the handoff record instead; a cold boot never does. Pod mute off, 48 kHz, layout 5.1 (unused slots carry zeros).
- Maximum level = stock (0.2.4): no OpenCompanion ceiling or limiter below the stock chain any more (the former
  −12 dB master ceiling and −6 dBFS limiter made the speaker ~12 dB quieter than stock at full volume, with peaks
  held at −6 dBFS). At OS 100 % (UAC 0 dB): USB input margin x 0.3404 (stock), master 0 dB (stock: USB curve 0 dB,
  pod 99 % = 0 dB, also the stock value without a pod), stock loudness, stock graph, final gain −1 dB above
  10.5 V, stock protection (band clippers, compressors, look-ahead limiter 0.95). Only a full-scale guard (0.999,
  = the PCM clamp) remains. Warm start without 'V': −30 dB (`VOL_SAFE_DB`) until the TAS sends 'V'.
- Stock protection ported (DAC-referred thresholds, float ±1.0 = full scale as for the whole stock graph):
  satellite ~250 Hz band soft clipper (knee 0.18, ceiling 0.20 before the −1 dB final gain, i.e. ~−16 dBFS band
  peak), satellite compressors (threshold 0 dBFS, 100 dB/s release, ≤ −10 dB per sample), look-ahead limiter 0.95,
  bass compressor (−1 dB, 40 dB/s) + soft clipper (knee 0.398, ceiling 0.794), VRAW ceiling (−1 dB above 10.5 V,
  less below, −5 dB per dB under 7 V), supply ripple / clip fault → mute + standby after 10 s. The laws were
  checked against a cycle-level emulation of the stock code (residuals < 5e-7).
- The bass also gets the 320-sample delay (stock: none, satellites 6.7 ms late): the crossover alignment of the
  profiles is unchanged. Total latency +6.7 ms.

## CPU
Measured with `CPU` and, in the profiling build (`make PROF=1`, RAM load only), `PRF0` / `PRF1` (stages in 0.1 % of
the 2 ms frame, `include/prof.h`) and `PRF2` (`PC:` = program cache mode CSR.PCC, 2 = enabled). There is no
cycle counter on the C67x+; the time base is RTI counter 1.
- 2026-11-01, before this work: Bose 155 %, neutral 87 %, against "BMIPS 42 %" for the whole stock firmware; every
  stage took ~3x its instruction count. Probable cause: **the program cache was off**. The ROM boot loader leaves
  CSR.PCC in bypass while it loads; the Bose boot driver sets PCC = 010b again before it starts the application
  (0x10001DD0) and the stock `c_int00` writes CSR = 0 (read as 010b on the C672x), but nothing on our boot chain
  did. `reset.asm` now sets PCC = 010b (a TI E2E thread reports ~3x on a C6727 from the same fix). Check: `PRF2`
  -> `PC:2`.
- Block kernels in the stock structure (`include/kern.h`, `src/dsp/`): scalar gains kept in registers (stock
  vector multiply 0x1002D880, ii = 1; the former code filled a gain array first), 4-lane biquad sections
  everywhere (satellite band filters: 3 passes for the 4 satellites instead of 12 single-lane ones; user EQ banks
  and crossover 4 lanes), branchless soft clipper and 4-lane compressor, per-block peaks, delay lines in straight
  segments, no `memcpy` byte loop (a copy loop is compiled as a call to the byte-loop memcpy: `k_copy` multiplies
  by a run-time 1). The full-scale guard works per frame (gain = min(release, 0.999 / frame peak), no overshoot) and
  the output meters reuse its block peaks. Settled hooked mixers use a constant gain; moving ones the exact stock
  smoother.
- Expected (static estimate, cache on, 440 Hz -20 dBFS): Bose ~37 % (graph ~22 %, satellite protection ~4 %,
  look-ahead limiter ~3 %, the other stages ~1 % each), neutral ~18 %. To be measured.
- Pipelined (`k_vec.c`, `k_biquad.c`): k_scale, k_mac, k_scalev, k_peaksq, k_smooth, k_i2f, the 4-lane biquad.
  Size-built (`k_ctl.c`, `k_dyn.c`, ~2-4x their cycles): k_macv, k_pcm, delay lines, peak follower, limiter
  curve, soft clipper, compressor. Measured on bld_d (2026-11-01, Bose, 5.1, music): LD 60 %, idle 23 %; the
  static estimate (37 %) had counted 6-10 cycles per sample for size-built loops, they take 15-25 (k_mac had
  also lost its trip-count hint: 7 cycles per sample instead of 2). Expected now: Bose ~45-50 %, neutral ~25 %.

## Channel mapping (McASP1 TDM slots ↔ buffers)
Buffers are `rx[pp][serializer][slot][sample]` and `tx[pp][slot][sample]`, exactly as Bose
(dMAX RX 0x00600802 / indexes 0x03000001, 0xFD60FFFF, 0xFA61FFFF; TX 0x00600801 / 0, 0x60, 0xFD61, set at
0x10018DC4..0x10018E88). Bose does no software remap:
- USB channel k (L, R, C, LFE, Ls, Rs) = `rx[pp][0][k]` (stock frame task 0x10015B70..0x10015C1C,
  address `base + (pp·16 + k)·96` words); AUX L/R = `rx[pp][1][0]` / `rx[pp][1][1]` (0x10015C44, 0x10015CE8).
- Output channel k (inner L, inner R, outer L, outer R, bass, bass) = `tx[pp][k]` (staging `[k][96]` at
  0x10015DD0..0x10015E4C, copied by memcpy 0x1001DAC0 at 0x10015E64); slots 6..7 zero.

**Bug fixed (2026-10-31): +1 slot shift on receive.** McASP1 and dMAX configuration were already identical to
Bose (registers measured in `re/dsp/notes/stock_regs.json`). The difference is the starting state: Bose starts
from a cold reset, we start after the stock firmware ran. The RAM loader disables the dMAX events (`DEDR`)
*before* stopping McASP1, so the McASP1 events 6/7 stay latched in `DEFR` (SPRU795D 2.3.4: latched even when
disabled, serviced as soon as `DEER` enables them). Enabling then fired one stale RX transfer (2 elements into
slot 0 of both serializers) and one stale TX transfer while McASP1 was still in reset, so every slot landed one
index late (USB L in index 1, R in 2, index 0 silent; the shift is exactly one because DEFR holds one flag per
event). `dmax_enable_audio()` now writes `DEFR = 0xC0` between `DEPR` and `DEER`. The transmit stream had the
same one-element offset (expected: wire slot k carried buffer k+1).

Hardware check (low level, PC sine ≤ −40 dBFS, tones −60 dBFS): PC left only → `LRI` ≈ −43 in index 0, index 1
at the noise floor (≤ −90); right only → −43 in index 1. `LRO` left only ≈ −73,−99,−73,−99,−99,−99 (level
depends on the volume), right only ≈ −99,−73,−99,−73,−99,−99. (The `TON` per-output tone check of that time no
longer exists: use a panned test file and `LVO`.) `BRR` of 0x6000001C (DEFR) normally reads bits 6/7 = 0.

## Parameters and profiles (binary console commands, little-endian, no `\r`)
**Client pacing.** The `E` relay carries one message at a time and the DSP keeps one reply: a new message
replaces the previous reply, so a client must wait for each reply (or its timeout) before sending the next
(~25 ms per short command through the TAS; a 55-byte `OPW` is 12 chunks). `OPW` is applied at once and never
answers busy: `15 07` only for an offset / length outside the profile; values are clamped, not refused. The
EQ glides to each new set from wherever it is, so rapid writes are click-free and the last one wins: coalesce
slider moves to the latest value, about 20-30 writes per second at most. Busy (`15 01`) exists only for EEPROM
jobs (`OPV`, `OPL`, `OPB`, `OPS`, `OPC`, `DEV<k>`): poll `OPI` and retry.

The active profile is a 616-byte structure (`include/params.h`), read and written by byte offset.
Replies: `06` + data, or `15 <code>` (1 busy, 2 I/O, 3 verify, 4 CRC, 5 protected, 6 empty slot, 7 bad argument).

| Command | Reply | Notes |
|---|---|---|
| `OPI` | `06` + 14 bytes | proto (2 = with `OPQ`, `OPS`, `bass_milli`; 3 = with `OPU` and flags bit3; 4 = with `OPC`, the options byte in the `OPU` reply and the band enabled bit; 5 = one engine: `OPQ` / `OPS` = sub level source, `crossover_hz` and `bass_milli` reserved, 2.1 routing from the TAS mode), profile size (LE16), slots, active slot (FF = none), flags (bit0 modified, bit1 EEPROM busy, bit2 last job failed, bit3 graph tables missing: the Bose base plays on the EQ path), boot slot, valid mask, writable mask, last result, biquad sections (LE16), CPU %, profile flags |
| `OPR <off:LE16> <n 1..56>` | `06` + n bytes | read the active profile |
| `OPW <off:LE16> <1..55 bytes>` | `06` | write the active profile; clamped and applied at once |
| `OPD` | `06` | "reset to flat": active profile = flat defaults, neutral base (`OPV` saves it to a slot) |
| `OPL <slot>` | `06` | load slot 0..3 (asynchronous, poll `OPI`); slot 0 never saved = the built-in Bose base (immediate) |
| `OPV <slot>` | `06` | save the active profile to slot 0..3 (asynchronous, ~50 ms) |
| `OPB <slot or FF>` | `06` | profile applied at boot (FF = the built-in Bose base, flat EQ) |
| `OPN <slot>` | `06` valid color seq(LE32) name[16] | slot directory (cached at boot) |
| `OPQ` | `06` src app:LE16 knob:LE16 value:LE16 | sub level source (0 rear knob, 1 app), the profile's `sub_level_cdb`, the rear knob as a sub level (exactly the gain applied in knob mode, −1200..+600), level in use; signed 1/100 dB |
| `OPS <0/1>` | `06` | sub level source, applied at once (smoothed) and saved in the config record (asynchronous, poll `OPI`); `15 01` busy: nothing changed; `15 05` applied but not saved; `15 07` bad argument |
| `OPK` | `06` | "reset to the Bose original": active profile = "Bose d'origine" = Bose base, flat EQ, stock loudness, stock AUX gain (volume = OS UAC dB, as every profile) (`src/control/profile_bose.c`); `OPV` saves it to any slot |
| `OPU` | | removed in proto 5 (routage 2.1 setting; the TAS `R` frame selects the 2.1 mode) |
| `OPC [<options>]` | `06` 0 in_effect layout options | (the former `OPU` reply; byte 1 now 0) device options byte ("Device settings" below): query, or set (applied at once, saved in the config record, asynchronous); `15 01` busy: nothing changed, `15 05` applied but not saved |

EEPROM U7001, layout v2 (`boot/docs/LAYOUT.md`: image body 0x0200..0xC1FF, boot1 0xDB00..0xFF7F):
slot `s` copy `c` at `0xC200 + (2s + c) × 0x300` (header `OCP1`, seq, length, version, CRC-32, then the
profile); config copies at 0xDA00 / 0xDA80 (`OCC1`, seq, boot slot, sub level source at +9, +10 reserved (was routage 2.1), device options at +11, CRC-32).
Layout v3: the last page of each copy (+0x280) is a graph data page, never part of a record and refused by the
driver; written by the host installer only. A save writes the older copy,
profile pages first and header page last, then reads it back. The driver refuses writes outside 0xC200..0xDAFF except
the header page 0x0100 (zeros only, `UPD`) and the boot-state page 0x0180 (trial confirmation); boot0, the image
body and boot1 are never written by the firmware. A copy is only written if it is blank or already ours
(on an EEPROM still holding the stock image, which ends at 0xDAD8, the area reads as foreign: saving is refused).
The ELF exports `oc_eeprom_layout_v2` (= 2) and `oc_eeprom_layout_v3` (= 3) (`src/storage/profiles.c`) for the host tooling.
All four slots are editable; slot 1 starts as the Bose base (flat EQ) until something is saved there. A double press on the pod mute (mute then unmute within
600 ms) loads the next valid slot.

## Sub level (rear knob or app)
The stock bass compensation is gone (all presets): bass tone goes through the EQ (sub target, global EQ). The rear
bass knob sets the **sub level** instead. Source = device setting (`DEVB<0/1>` / `OPS`, config record +9, the
former bass source byte, same values; default 0 = knob, as the stock speaker):
- **0, knob**: the knob alone sets the sub level: scale of `ramp.c` (−5/3..+5/3, stock hysteresis) -> u = −1..+1,
  dead zone |u| <= 0.05 = 0 dB (centre detent), then linear in dB: +6 dB fully right, −12 dB fully left
  (`sub_level.c`). The profile's `sub_level_cdb` is not used.
- **1, app**: a **device setting** since 0.2.3 (config record +10, 128 + dB×10, −12..+12 dB in 0.1 dB steps; 0 dB
  in records written before), set with `DEVL<dB×10>` (applied at once, smoothed; saved 1 s after the last change),
  read in `DEV` `SL` and `OPQ` `app`; it stays when profiles are switched and with the read-only Bose profile. The
  profile's `sub_level_cdb` is ignored. The knob is ignored but still reported.
Readouts: `DEV` `KN` (dB×10) and `OPQ` `knob` (1/100 dB): the exact level applied in knob mode, whatever the
source. Every change (source, knob, profile value, sub trim) is smoothed per sample (~40 ms one-pole): no click.

## Gain changes and the output ramp
Every gain change is ramped: master volume (OS `V` or pod knob in LOCAL mode) and loudness through the stock
gain smoothers (~16 ms per sample, `loudness.c`); sub level and sub trim ~40 ms (`sub_level.c`); EQ (glide,
above); USB mute 'm' (USB channels only, per-sample pole). The **output ramp** (`audio_out.c`, all outputs) is a
15 ms smoothstep (zero slope at both ends, like a raised cosine) from open to silent and back, for the pod mute
(speakers), standby, fault, every profile switch (whole profile installed, or a band count change), preset
(profile base) or rate / layout switch: the switch waits until the
ramp reports silence (`audio_out_silent()`), then the ramp reopens. The amplifier release / drive (AXR0[14])
happens 31 blocks (62 ms) after the mute request, so after the ramp has closed.
Profile layout: offset 36 (`bass_milli`) and `crossover_hz` (offset 22) are reserved and written as 0; older
profiles keep loading (the values are ignored); `PROFILE_VERSION` stays 1.

## Device settings (config record) and the band enabled bit
Device settings live in the `OCC1` config record (`include/profiles.h`), not in the profiles: +9 sub level source,
+10 reserved (was routage 2.1), +11 **device options** (`OPT_*`). Byte 11 was always written as 0, so every record written
before reads as the stock defaults; it is covered by the record CRC (bytes 0..11): no format change. Set with
`OPC <byte>` (binary, reply as `OPU`) or `DEVO<decimal>` (text); applied at once, saved asynchronously.

| Bits | Option | Values (0 = stock) |
|---|---|---|
| 0..2 | standby delay before the silence standby request | 0 stock 2 min, 1 off (never), 2 5 min, 3 10 min, 4 20 min, 5 30 min, 6 60 min (7 = 2 min, reserved) |
| 3 | pod double press (mute then unmute within 600 ms) | 0 cycles the profiles (as before), 1 does nothing |
| 4 | headphone EQ | 0 flat headphone feed (stock), 1 the active profile's global EQ (target 0, "Tout") also on the headphones. Stored now, applied once the headphone feed exists (below) |
| 5..7 | silence threshold (the app's "AUX wake threshold"), signed step n = −4..+3 | stock power × 4^n, 6.02 dB per step: −95.2, −89.1, −83.1, −77.1, **−71.1** (stock, 7.801579e-8), −65.1, −59.0, −53.0 dBFS, mean L/R block power after the master volume (`standby.c`); sound above it resets the timer and cancels a pending request |

The delay is counted in frames (2 ms). "Off" sets a delay the
counter never reaches; a supply / clip fault still requests standby. The delay is `g_standby.delay` (frames; `BRR`, address in the map; `STB` was removed).

**Band enabled bit** (`params.h` `BAND_DISABLED`, profile protocol 4): a band switched off used to be written
as `BAND_OFF`, and `params_sanitize()` then lost its type on reload. Now `type | 0x80` (0x81..0x86, peak..notch
only) is a disabled band that keeps its type, frequency, gain and Q: not compiled, not counted in the Bose-base
CPU cap, clamped like the others. `BAND_OFF` still means "no band". The Bose-base cap now disables the bands
beyond it (type kept) instead of writing `BAND_OFF`. Compatibility: profiles saved before have bit 7 clear
(unchanged, no version change); an older firmware sees `type >= BAND_TYPE_COUNT` and reads a disabled band as
`BAND_OFF`, so a profile saved here never plays differently there. Host side: `oc-device` `BandSlot::set_enabled`,
`is_enabled`, `kept_kind`.

## Headphones
The pod `MUTE` line (`M`) is asserted by the mute touch and by a plugged headphone, indistinguishably. The
headphone amplifiers U1000 are fed by DAC_LC / DAC_RC (U4000.27 / .28), i.e. TDM slots 1 / 2 = inner L / R, and
cannot be muted. As stock (mode selector 0x1001BF14, 0x100276E8): on `M` 01 the master is muted (smoothed) until
the sequencer has released the speaker amplifiers (`MD` 1 / 3), then the **headphone feed** fades in: the engine
runs the neutral program (stereo after the master volume, loudness and 5.1 downmix) and sends L / R to inner L /
R; no output EQ, crossover, trims, sub level or satellite band protection; outer and bass silent; the profile's
global EQ only with `OPT_HP_EQ` (device option bit 4, otherwise flat, stock). `M` 00: muted while the sequencer
switches (`MD` 0), speakers back from `MD` 2 (the Bose preset returns after a 2 ms fade). Not reproduced: the stock
headphone mixer gains (0x10010C7C / 0x10010CA4, −2 dB). The mute touch behaves the same (speakers off, inner
slots fed), as stock.

## Size (EEPROM body, `oc-bootimg sizes`)
| Build | `.text` | Loaded | LZ4 body | Body area |
|---|---|---|---|---|
| before (2026-11-01 morning) | 55,296 B | 107,198 B | 43,216 B | 87.9 % |
| now (all features above) | 54,528 B | 118,260 B | 42,152 B | 85.8 % (7.0 KB free) |
| before the bass source (2026-11-01) | | 124,076 B | 45,968 B | 93.5 % |
| with the bass source | 57,888 B | 125,952 B | 47,152 B | 95.9 % (2.0 KB free) |
| own start-up, `--ram_model`, small memcpy | 55,904 B | 123,792 B | 45,376 B | 92.3 % |
| + dead code (`ramp_gblctl`, stage-3-only `mcasp1_start_no_dma`) | 55,840 B | 123,728 B | 45,328 B | 92.2 % |
| + console labels as one `\0`-separated string per reply | 55,872 B | 123,508 B | 45,096 B | 91.7 % |
| + `-O1` on the control-plane files (2026-11-01) | 55,648 B | 123,284 B | 44,872 B | 91.3 % (4.2 KB free) |
| + stock graph engine, greedy LZ4 (2026-11-01) | 62,080 B | | 51,704 B | 105.2 % (does not fit) |
| layout v3 (tables in the graph data pages), optimal-parse LZ4, zero fills not stored, no RTS division helpers, built-in profile without raw bands, `LVL` / `AST` / `BSV` / `TON` removed, + routage 2.1, Bose base + EQ | see `oc-bootimg sizes` | | 48,192 B | 98.0 % (960 B free) |
| + speed kernels (`bose_dsp.c`, software pipelined), `CPU` measurement; `BGQ`, `BAS`, `RMP` removed | | 70,070 B | 49,048 B | 99.8 % (104 B free) |
| + device options (standby delay / threshold, double press, headphone EQ bit), band enabled bit, `OPC`; `DEV` replaces `BSQ` / `BSS` / `R2Q` / `R2S` | | 69,562 B | 48,924 B | 99.5 % (228 B free) |
| step (a): block kernels on the old structure, program cache on (RAM load only) | | 75,203 B | 52,480 B | 106.8 % (does not fit) |
| step (b): one engine (programs, EQ banks), bass compensation and LR4 removed, sub level, kernels split speed / size, frame-level −6 dBFS limiter | | 69,878 B | 49,100 B | 99.9 % (52 B free) |
| step (c): + headphone feed | | 69,878 B | 49,092 B | 99.9 % (60 B free) |
| `OPU` / `DEVR` (routage 2.1 setting) removed | | | 49,016 B | 99.7 % (136 B free) |
| `GPI`, `STB`, `VRW`, `LRI`, `LRO`, `BOSE_EQ_SECTIONS` removed (48,032 B, 1,120 B free), then hot loops back to speed builds, output ramp, EQ glide | | 69,566 B | 48,900 B | 99.5 % (252 B free) |
| bld_f: profile switch behind the output ramp | | | 49,088 B | 99.9 % (64 B free) |
| bld_h: `PF_ADVANCED` (app-only flag 0x20), raw biquad pairs removed (protocol 6) | | 68,758 B | 48,408 B | 98.5 % (744 B free) |
Layout v3 savings (LZ4 bytes, measured one by one): optimal-parse LZ4 encoder in `oc-ais` (standard LZ4 blocks,
boot1 unchanged) ~800; `.bss` / `.far` / `.stack` / `.bose_ram` NOLOAD (zeroed by `oc_start`, no zero fills in
the body) ~370; integer division / remainder / float-to-unsigned RTS helpers avoided ~740; own `divf` ~500; graph
tables out of the body ~720 (the loader costs ~500 of it back); built-in profile without raw bands ~330;
`-O1` on ramp / TAS link / EQ designer ~250; `TON` / `TOF` test tone ~900, `LVL`, `AST`, `BSV` ~350 removed.
Savings used: size build of everything but the biquad kernel, compact math instead of powf / sinf / cosf / log10f /
sqrtf, near scalars (`far_aggregates`), shared console formatting and packed labels, own start-up without .cinit,
small memset / memcpy, `-O1` control plane.
Tried and not kept: `--gen_func_subsections` (+300 B: every function padded to a fetch packet; only two dead
functions found), keeping the dMAX microcode compressed (LZ4 already gets 4,084 → 2,630 B; byte-plane or
big-endian transforms do not help, only an entropy coder would, and its decoder costs more than the ~0.6 KB gained),
re-ordering sections to merge ELF segments (no gain), `-O3` on the control plane (-96 B, less than `-O1`).
Init-only code in reusable RAM would free RAM, not EEPROM: the code still has to be in the body.

## Not ported (impact)
| Stock item | Impact |
|---|---|
| Spatial steering, array cross-feed / delays, C / Ls / Rs, LFE gain | ported in the stock graph engine ("Bose d'origine" only); the neutral preset keeps its own routing and the 5.1 downmix |
| Headphone mixer gains of the mode selector 0x1001BF14 (−2 dB) | headphone feed at the program level ("Headphones") |
| Stock bass compensation 0x10010938 (knob polynomial x LPF4) | replaced by the knob sub level; its all-pass branch is kept ("Engine") |
| All-pass of the BADDATA selector branch A (0x1000971C) | phase only |
| Console overrides VOM / BTO, `CKT`, `SNM`, stock `D`/`d` parameter protocol | none for audio |
| 0x10015700 standby-delay re-arm (rewrites 60000 every 120 s of USB inactivity) | none (our delay only changes with the device options) |

## Speaker amplifier control AXR0[14] (`spk_mode.c`)
Without it the amplifiers stay muted: the DAC data is correct but nothing is heard. Stock (re/dsp/notes/hardware.md 6bis):
- **driven low** (`PDCLR = 0x4000`, then `PDIR |= 0x4000`) = speakers on; **released** (`PDIR &= ~0x4000`, input)
  = speakers off. PDOUT bit 14 is never set. Playing stock firmware: `PDIR = 0x80004600`, PDOUT bit 14 = 0.
- Sequencer (stock mode selector, states = `MD`, 1 block = 96 samples): pod mute `M` 01 (pod mute touch or
  headphones) → 31 blocks → routing 8 blocks → release → 31 blocks → muted (`MD` 3). `M` 00 → 31 blocks →
  routing 8 blocks → drive low → 31 blocks → on (`MD` 4). From start-up: drive low after 8 blocks (~16 ms) when
  not muted.
- Also released once on the standby request and on the supply / clip fault (stock 0x100157E8, 0x10015910).
- **Not affected by the USB mute `m`** (stock: `m` only zeroes the USB volume curve).
- Start: cold boot = released (reset value) until the sequencer drives it; RAM-loader start = the stock pin
  state is kept (`mcasp0_gpio_init`, `ramp_init`) and the sequencer starts from the handed-over pod mute.
  Stock itself waits for the TAS `M` 00 (its pod mute starts at 1.0); OpenCompanion starts from `pod_mute` = 0
  because the TAS does not resend `M` after a DSP-only reset.
- Read-only check (`BRR`): McASP0 `PDIR` 0x44000014 = `0x80004600` (`0x80004640` during a calibration frame;
  bit 9 = DSPINT, inherited from stock on a RAM load, 0 after a `U`),
  `PDOUT` 0x44000018 bit 14 = 0. Pod muted / headphones: `PDIR` bit 14 = 0. `g_spk` (address in
  `build/opencompanion_dsp.map`): byte 0 = state (4 = on, 3 = muted), byte 6 = 1 when driven.

## RAM loader (`loader/`)
Runs a DSP image from RAM through the stock firmware, without writing any EEPROM (standby or unplug reboots the
stock firmware). Host side: `app/crates/oc-ramload`. Build: `make` in `loader/` (CGT 7.4.24).
- Area `0x10023000..0x10024DFF` (tail of the stock `.cinit` table, dead after boot; stack below `0x10024DF8`,
  handoff record at `0x10024E00`, `include/handoff.h`). The host checks it still matches stock (twice, 1 s
  apart) before writing; the loader refuses writes, fills and jumps into `0x10023000..0x10024E1F`.
- Hijack: output meter `0x10011C4C` (class `0x1002CDDC`, flags `0x3`), process pointer at `0x10011C60`, called
  every 2 ms by the audio task (priority 13) through `0x10026F20`. Flags and original pointer are checked first:
  any other stock version is refused.
- `stub_entry`: GIE = IER = 0, own stack, `loader_main`: copies the stock TAS link state to the handoff record
  (the TAS only sends `X`/`U`/`M`/`m`/`V` at start-up and on change), disables dMAX events (`DEDR`), stops
  McASP0/1/2 (`GBLCTL = 0`), resets I2C0 as slave `0x29` without interrupt or dMAX. Before `JUMP` it
  invalidates L1P (`L1PICR = 0x80000000`, as the stock boot driver at `0x1000295C`).
- TAS frames to `0x29`: `[cmd][seq 0,1,... | 0xFF last][len <= 5][5 data bytes]`; for `E` the TAS waits 20 ms
  then reads frames until `seq = 0xFF` (at most 61 data bytes, else `ERROR4`). End of transaction = `AAS`
  falling. While loaded: `P` -> `ALIVE`, `S` -> `[0, 0]` (no standby request), other commands -> empty answer.
- Protocol (payload of `E`, little-endian; error = `15`):

| Sub-command | Request | Answer |
|---|---|---|
| `INFO` | `00` | `06 'O' 'C' 'L' version` |
| `WRITE` | `01 addr:u32 data` (<= 56 bytes) | `06` |
| `CRC` | `02 addr:u32 len:u32` (<= 64 KB) | `06 crc32:u32` (zlib) |
| `JUMP` | `03 entry:u32` | `06`, jump once the TAS has read it |
| `READ` | `04 addr:u32 n:u8` (n <= 60) | `06 data` |
| `FILL` | `05 addr:u32 len:u32 byte:u8` | `06` |

- Failure modes: the DSP stops answering (`ERROR1` on `E`), a misbehaving I2C slave can hang the TAS; recovery
  by standby or unplug. Audio stops when the stub starts (click possible: keep the volume low). The loaded
  image must set up its own stack and interrupt table; the PLL is kept from the stock boot.

## Hardware test plan (RAM loader only; never flash first)
Volume at 10 % or less, pod volume low, AUX unplugged unless stated. Commands go through
`oc dsp ramload --image <ELF>` (RAM load) and the `E` console relay (`uv run oc_tas.py console <CMD>` or the stock HID
`E` tool you used tonight).
0. Rebuild: `make STAGE=4` here, `make` in `loader/`; then RAM-load as before (short fade-in at start).
   Before any listening: `BRR` McASP0 PDIR (0x44000014) must read `0x80004600` (section above).
1. `SVM`, `SYS` (FR rising), `PRT` `VR` (13 V class),
   `PRT` → `FT:0 VR:13xxx T:00 CC:0 BC:0 CG:0 LG:0`.
2. `POD` → `PR:1` with the pod connected (handoff from the stock firmware; the stock TAS never re-sends `X`),
   `MD:H`, `DB` = the Windows volume in dB×10. Move the pod knob: `KN` and `RK` follow, `EV` increments, **the
   level does not change** (HOST mode). Change the Windows volume: `DB` follows.
3. Knob check: put the knob at a fixed position, read `POD` (KN) and `g_ramp` with `BRR` (count[2] = IVOL count, cal; address in the map);
   power-cycle into the stock firmware, run `uv run verify_hw.py` and compare 0x100137A4 (IVOL ratio ≈ N3/C)
   and 0x100137AC (knob %) with `KN`/10 at the same position. Report both.
4. `DEV` (`KN`) at the knob centre and both ends; compare with 0x10010104 in `verify_hw.py`.
5. LOCAL mode: unplug the USB cable (AUX source playing quietly): `POD` → `MD:L`, the pod knob now sets the
   volume (`DB` follows the stock pod curve), USB channels muted. Replug: `MD:H`.
6. Low-level test signals from the PC (the `TON` generator was removed with layout v3): a −40 dBFS sine,
   `PRT` `CG`, `LG` stay 0, `LVO` follows the input. Bose base (`OPK`): `CPU` `LF:0`, `MX` < 85 %; `OPI` flags bit 3 = 0 (graph tables found).
7. Music at low level, Bose profile (`OPK`): check the loudness (`POD` `LB` positive at low volume, 0 when loud)
   and the absence of clicks; `PRT` stays `FT:0`.
9. Boot chain (only after the boot chain RAM tests of `boot/docs/UPDATE.md`): `BST` → `C:0 T:5` on a RAM load;
   `RSC` reboots (stock EEPROM: stock firmware comes back).
`UPD` writes the EEPROM (header page): it refuses a stock EEPROM (`BST` R:5), but use it only on a migrated
EEPROM, as the first step of an update.

## Hardware log
- 2026-11-01: full firmware run from RAM (stock TAS from EEPROM). After porting the stock AXR0[14] amplifier
  control (spk_mode.c), sound is audible. Guided output walk (`tools/tone_test.py`, key `w`, about -55 dBFS):
  user confirmed the 6 outputs are correct. Channel L/R input mapping, EQ (+/-6 dB at 1 kHz), pod detection,
  knob reading (matches stock), ramp calibration, VRAW and boot status verified by measurement.
  Known: with the STOCK TAS, `V` is a percentage but the firmware reads it as UAC dB, so music is nearly
  silent until our TAS firmware runs (tones bypass the volume).
