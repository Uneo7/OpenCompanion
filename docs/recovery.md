# Recovery (advanced)

Only for a speaker that no longer works normally after an install or update that went wrong. For a normal
install, see [first setup](first-setup.md). Steps marked **not yet validated** haven't been tried on a real
speaker.

## Before you start

- Two chips, each with its firmware in its own memory: the **audio processor** (sound, memory **U7001**) and
  the **USB controller** (talks to the computer, memory **U6002**).
- **Recoverable** = the computer can talk to the speaker over USB again, without opening it. Almost every
  incident is; the exceptions are in [part 5](#5-last-resort-opening-the-speaker).
- Keep your **backups** at hand: `OpenCompanion/backups/<date>/`, files `u6002_tas.bin` and `u7001_dsp.bin`
  ([where](first-setup.md#keep-your-backup)).
- Turn the computer's volume and the knob all the way down before each attempt.
- **Power cycle** = unplug the speaker's power plug for 10 seconds, plug it back in, USB cable left connected.

## 1. Which fix?

Try them **in order**. Part 4 is the last-resort software fix.

| What you see | What it means | Go to |
|---|---|---|
| Wizard shows "Interrupted · Connection lost" | USB link cut while writing; nothing lost | [Part 2](#2-resume-in-the-app) |
| App offers "Update OpenCompanion", or speaker recognized but silent | Audio processor in **rescue mode** (firmware missing or damaged) | [Part 2](#2-resume-in-the-app), then [part 3](#3-audio-processor-rescue-mode) |
| Wizard shows "Interrupted · Recovery guide"; computer sees an **unknown USB device** `05A7:1020` or `FFFF:FFFE`; no sound; pod light off | USB controller in its built-in **DFU loading mode**: writing its firmware was interrupted | [Part 4](#4-usb-controller-dfu-mode) |
| After a power cycle, **no USB device** at all | USB controller firmware starts but doesn't connect, or a memory is damaged | [4.6](#46-startup-escape), then [part 5](#5-last-resort-opening-the-speaker) |

**Reading the USB ID** (vendor:product):

- Windows: Device Manager > the device > Properties > Details > "Hardware Ids" (`USB\VID_05A7&PID_1020` =
  `05A7:1020`). The normal Bose firmware also uses `05A7:1020`, but shows up as a speaker, not an unknown device.
- macOS: Apple menu > System Information > USB. Linux: `lsusb`.
- `oc install status` shows "TAS ROM in DFU mode (05a7:1020)" in this case.

## 2. Resume in the app

Fixes the vast majority of cases.

1. Power cycle.
2. Reopen the wizard (**Resume**, or **Settings > Firmware**). It finds the speaker and resumes where it stopped;
   what's already written is read back, not rewritten.
3. Don't unplug anything, keep the computer on.

## 3. Audio processor rescue mode

Nothing to turn on: if the audio processor firmware is missing, incomplete or damaged, the speaker **starts in
rescue mode** at every power-on. No sound, but the app can reinstall the firmware over USB.

1. Power cycle.
2. Open the app: it offers **Update OpenCompanion**. Accept.

**Validated on the speaker**, including with the USB controller on the OpenCompanion firmware.

Known limit: if the audio processor firmware **starts then freezes**, rescue mode doesn't turn on by itself.
Requesting rescue mode from the app is coming soon.

## 4. USB controller DFU mode

This loading mode is **burned into the USB controller chip** and can't be erased. The chip starts in it when
writing its firmware was interrupted, and accepts a firmware over USB.

> **This recovery uses the `oc` command-line tool**, not the app. Download `oc-<version>-<system>.zip` (Windows)
> or `.tar.gz` (macOS, Linux) from [Releases](https://github.com/Uneo7/OpenCompanion/releases), extract it,
> run commands from that folder (`.\oc.exe` on Windows, `./oc` on macOS and Linux). Not comfortable with a
> terminal? Ask the project for help first.

Two cases that look the same from outside:

- **After an interrupted install** (most common): what you send is **written directly to U6002** and survives
  a power cut (from analysis of the chip; **not yet validated**).
- **After the [startup escape](#46-startup-escape)** or in development: what you send goes to **temporary
  memory**, erased at the next power cut (**validated**).

The recipes below work in both cases.

### 4.1 Windows: WinUSB driver

Windows has no driver for this mode: you need **WinUSB** (generic, from Microsoft). Needs an **administrator**
PowerShell (right-click > "Run as administrator").

**Scripted (`05A7:1020` only).** `tools\tas_dfu_run.ps1` installs the driver, sends the firmware, restarts the
USB device, then **removes the driver**. See [recipe (a)](#a-reinstall-opencompanion-from-dfu-mode). It's part of
the development tree, not the release downloads (ask the project for it).

**Manual, with Zadig** (required for `FFFF:FFFE`). Get Zadig **only from https://zadig.akeo.ie**; OpenCompanion
doesn't host or distribute it.

1. Launch Zadig (asks for admin rights).
2. **Options > List All Devices**.
3. Pick the device whose USB ID is **05A7 1020** or **FFFF FFFE**. Double-check: never install WinUSB on another
   device.
4. Choose **WinUSB**, then **Replace Driver** (or **Install Driver**).
5. Check: `oc tas dfu list` shows `05a7:1020 DFU interface 0` (or `ffff:fffe ...`).

**Remove the driver afterwards.** The Bose firmware uses the same `05A7:1020`: with WinUSB left in place the
speaker wouldn't show up as a speaker.

- Admin PowerShell: `pnputil /enum-drivers`, find the package whose provider is "libwdi" (`oemNN.inf`), then
  `pnputil /delete-driver oemNN.inf /uninstall /force`.
- Or Device Manager: right-click the device > **Uninstall device**, tick "Attempt to remove the driver for this
  device", then power cycle.
- The script does this itself; the manual method is **not yet validated**.

### 4.2 macOS and Linux

No driver (**not yet validated**). On Linux, the release's `70-opencompanion.rules`
([install](install.md#linux-the-udev-rule)) already has the DFU lines. With an older copy of
`/etc/udev/rules.d/70-opencompanion.rules`, replace its content with:

```
KERNEL=="hidraw*", ATTRS{idVendor}=="05a7", ATTRS{idProduct}=="1020", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0001", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0002", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="05a7", ATTR{idProduct}=="1020", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="ffff", ATTR{idProduct}=="fffe", TAG+="uaccess"
```

The `hidraw` lines are for normal operation, the `usb` lines for DFU mode. Then
`sudo udevadm control --reload && sudo udevadm trigger`.

### 4.3 Recipes

You need:

- `oc_tas.dfu` and its data area `oc_tas_data.bin`, same version: both in the release's
  `opencompanion-firmware-<version>.zip`, and in the app's download folder `OpenCompanion/firmware/<release>/`
  (next to `backups/`);
- your backup `u6002_tas.bin`.

Always start with a power cycle, then `oc install status` (TAS line: "TAS ROM in DFU mode").

#### (a) Reinstall OpenCompanion from DFU mode

1. Send the firmware **and restart the USB device right away**:
   - **Windows, `05A7:1020`** (admin PowerShell, `bose` folder):
     ```
     powershell -ExecutionPolicy Bypass -File tools\tas_dfu_run.ps1 opencompanion\firmware\usb\build\oc_tas\oc_tas.dfu
     ```
     Expected: `binding WinUSB to the DFU device...`, `05a7:1020 DFU interface 0`, progress `.../...`,
     `download complete`, `restarting the USB device (bus reset)...`, `removing WinUSB package oemNN.inf`, then a
     device table. Log: `tools\tas_dfu_run.log`.
   - **Windows, `FFFF:FFFE`** (Zadig driver, admin PowerShell), **on one line** so the restart follows at once
     (**not yet validated**), then remove the driver ([4.1](#41-windows-winusb-driver)):
     ```
     $id = (Get-PnpDevice -PresentOnly | ? InstanceId -match 'VID_FFFF&PID_FFFE').InstanceId; oc tas dfu download oc_tas.dfu --to-ram --id ffff:fffe --yes; if ($LASTEXITCODE -eq 0) { pnputil /restart-device "$id" }
     ```
   - **macOS / Linux**: `oc tas dfu download oc_tas.dfu --to-ram --yes` (the tool sends the restart itself;
     **not yet validated**).

   If `oc` prints "warning: USB reset request failed … replug the cable", **don't unplug the cable**: on Windows
   that's normal, the script (or `pnputil`) does the restart ([4.5](#45-pitfalls)).
2. After a few seconds the speaker should appear as `05A7:1020` "Companion 5 (OpenCompanion)" (0.5.0+; 0.4.x
   appeared as `1209:0001` "Companion 5 dev"), and `oc install status` shows OpenCompanion.
   - **Nothing appears?** Power cycle. If the speaker then shows up as OpenCompanion, the firmware was written to
     memory: go to step 3. If it comes back in DFU mode, or nothing appears, the data area is missing (without it
     the firmware doesn't connect): go to [recipe (b)](#b-back-to-the-bose-firmware-from-dfu-mode).
3. Finish the install: the app's wizard (**Resume**), or `oc install tas` (type `WRITE`; it reads everything back
   and writes only what's missing, data area included). Expected ending: "power-cycle the speaker to start the new
   TAS firmware". Power cycle.
4. Check with `oc install status`. Sound is back on the OpenCompanion firmware.

Sending to temporary memory then starting (step 1, script) is **validated**; the rest after an interrupted
install is **not yet validated**.

#### (b) Back to the Bose firmware from DFU mode

**Never** run the Bose firmware from temporary memory to "repair": it starts but plays no sound (observed). It
must go back into U6002, from **your** backup.

- **Recipe (a) made the speaker appear as `1209:0001`** (even without a power cycle): `oc install restore` (both
  chips, audio processor first; `--tas` for the USB controller only; `--from <folder>` to pick the backup). Type
  `WRITE`. The USB controller restarts on the Bose firmware (`05A7:1020`, seen as a speaker). **Not yet
  validated.**
- **Directly from DFU mode** (interrupted install): WinUSB installed ([4.1](#41-windows-winusb-driver)), then
  `oc tas restore-app --image <backup>/u6002_tas.bin --yes`; progress ends with `download complete`. Power cycle:
  the speaker should come back as a speaker. Remove WinUSB, then `oc tas mode app --yes` puts back the exact Bose
  header ("written and verified"). **Not yet validated.** If it comes back in DFU mode after the power cycle, the
  chip was in temporary-memory mode: use the previous case.
- **Development case** (header set to DFU by `oc tas mode dfu`): power cycle, run `tools\tas_dfu_run.ps1` with no
  argument (loads the test firmware), then `uv run tools/tas_romdump.py --restore-header --yes` (refuses to write
  unless the header is exactly the expected one, then reads it back), then power cycle. **Validated**: memory
  identical to the backup, sound OK.

#### (c) The speaker doesn't appear at all

Use the [startup escape](#46-startup-escape), then recipe (a) or (b) (temporary-memory mode).

### 4.4 If the USB controller runs OpenCompanion

No recipe needed: the OpenCompanion firmware can enter DFU mode without writing anything
(`uv run tools/oc_tas.py dfu`, **validated**, `05A7:1020` appears in ~1 s) or restart the chip
(`uv run tools/oc_tas.py reboot`, **validated**). These are development tools.

### 4.5 Pitfalls

- **Don't unplug the USB cable to "start" the firmware.** The chip pauses when the cable is removed and never
  resumes: it stays stuck until the next power cycle. Only a USB device restart by the computer (script,
  `pnputil /restart-device`) starts the firmware sent.
- **That restart must follow the send immediately.** If Windows puts the device to sleep in between, the chip
  gets stuck the same way: power cycle and start over.
- **The Bose firmware run from temporary memory is unusable**: green light, no sound, and it once showed up again
  as a DFU device. It's not a repair.
- At the end of the send, the chip announces a ~35-minute wait (a chip bug): `oc` caps it, don't interrupt.
- **Remove the WinUSB driver** ([4.1](#41-windows-winusb-driver)), or the Bose speaker won't be seen as a speaker.

### 4.6 Startup escape

OpenCompanion USB controller firmware installed, but the computer no longer sees the speaker:

1. Power cycle.
2. As soon as the pod's **red** light comes on, press mute **4 times** within **8 seconds**.
3. The USB controller switches to DFU mode (`05A7:1020`, temporary-memory mode) without writing anything. Apply
   recipe (a) or (b).

**Not yet validated.** Doesn't work without the pod, or if the firmware freezes before reading the button: then
see part 5.

## 5. Last resort: opening the speaker

**Rarely needed**: only if parts 2 to 4 can't get the computer talking to the speaker. You rewrite the affected
memory directly from your backups. For someone with the equipment and electronics experience.

You need:

- a screwdriver, and protection against static electricity;
- an I²C **EEPROM programmer** (the project used a Raspberry Pi Pico) and a **test clip** for the chip package:
  - U6002 = AT24C64A (USB controller, 8 KB, TSSOP-8);
  - U7001 = AT24C512 (audio processor, 64 KB, SOIC-8);
- the speaker **unplugged from the mains and USB** during the operation;
- your backups `u6002_tas.bin` and `u7001_dsp.bin` (check their fingerprint before writing).

When it applies:

- **Power cut during the sensitive step** of the audio processor (~17 ms during the first install, orange bar):
  the chip no longer starts at all. Memory: U7001.
- **Serious firmware defect** damaging the audio processor's startup chain (the firmwares refuse these writes,
  but a defect is possible). Memory: U7001.
- **USB controller firmware fully written but freezing at startup**, with no [escape](#46-startup-escape).
  Memory: U6002. Texas Instruments' procedure also suggests briefly shorting this memory's SCL and SDA pins at
  power-on to force DFU mode: **not yet validated** on this speaker.
- **Audio processor firmware that starts then freezes**, until requesting rescue mode from the app exists
  ([part 3](#3-audio-processor-rescue-mode)).

This guide isn't a support service: if in doubt, ask the project before opening the speaker.
