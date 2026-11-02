# Recovery guide

Use this only if the speaker no longer works normally after an OpenCompanion installation or update
that went wrong (non-nominal case). For a normal installation, see
[installation.md](installation.md).

> **Project status: first release.** Steps marked "not yet validated" have not yet been tried on a real
> speaker.

## What to know before you start

- The speaker contains two chips, each with its **firmware** (internal software) stored in its own
  **memory**: the **audio processor** (which computes the sound, memory U7001) and the **USB controller** (which
  talks to the computer, memory U6002).
- "**Recoverable**" means: the computer can talk to the speaker again over the USB cable, without
  opening the speaker. Almost all incidents are recoverable. The exceptions are described in part 5.
- Keep your **backups** at hand (folder `OpenCompanion/backups/<date>/`, files `u6002_tas.bin` and
  `u7001_dsp.bin`, see [installation.md](installation.md#where-are-your-files)).
- Turn the computer's volume and the knob all the way down before each attempt.
- "**Power cycle**" = unplug the speaker's power plug for 10 seconds, then plug it back in, while
  leaving the USB cable connected.

## 1. Which solution should you choose?

Try them **in order**. The rescue loading mode (part 4) is a **last-resort software**
solution: use it only if parts 2 and 3 do not apply.

| What you see | What it means | What to do |
|---|---|---|
| The wizard shows "Interrupted · Connection lost" | The USB link was cut during writing. What was written is checked, nothing is lost. | Part 2 |
| The app sees the speaker but offers "Update OpenCompanion", or the speaker is recognized but plays no sound | The audio processor started in **rescue mode**: its firmware is missing or damaged. | Part 2, then part 3 |
| The app shows "Interrupted · Recovery guide" (recovery needed); the computer sees an **unknown USB device** `05A7:1020` or `FFFF:FFFE`; no sound; the control pod's light stays off | The USB controller started in its **rescue loading mode** (the "DFU" built into the chip): writing its firmware was interrupted. | Part 4 |
| After a power cycle, the computer no longer sees **any** USB device | The USB controller firmware starts but does not connect, or a memory is damaged. | Part 4.6, then part 5 |

**Reading the USB ID** (two 4-character numbers, the manufacturer then the product):
- Windows: Device Manager > the device > Properties > Details > "Hardware Ids"
  (`USB\VID_05A7&PID_1020` = `05A7:1020`). Careful: the normal Bose firmware also uses `05A7:1020`, but it
  then shows up as a speaker, not as an unknown device.
- macOS: Apple menu > System Information > USB. Linux: `lsusb` command.
- With the `oc` tool: `oc install status` shows "TAS ROM in DFU mode (05a7:1020)" in this case.

## 2. Resume in the app

This is the solution in the vast majority of cases.

1. Do a power cycle.
2. Reopen the wizard ("Resume" button, or Settings > Firmware). It finds the speaker again and resumes
   the installation where it stopped. What has already been written is read back and is not written again.
3. While it resumes, do not unplug anything and keep the computer on.

## 3. Audio processor rescue mode

You have nothing to do to turn it on: if the audio processor firmware is missing, incomplete or damaged,
the speaker **automatically starts in rescue mode** every time it is powered on. This minimal mode plays no
sound, but lets the app reinstall the firmware over USB.

1. Do a power cycle.
2. Open the app: it detects that the audio processor is not ready and offers "Update
   OpenCompanion". Accept. It reinstalls the firmware from rescue mode.

Rescue mode and reinstalling the audio processor firmware are **validated on the speaker**, including
when the USB controller is running the OpenCompanion firmware.

Known limitation: if the audio processor firmware **starts and then freezes**, rescue mode does not turn on
by itself. Requesting rescue mode from the app is **coming soon**.

## 4. USB controller rescue loading mode (DFU)

**This mode is burned into the USB controller chip**: it cannot be erased. The chip starts in it by itself
when writing its firmware was interrupted. It lets you send a firmware again over the USB cable.

> **Today, this recovery is done with the `oc` command-line tool** (commands typed into a
> terminal), not in the app. Download `oc-<version>-<system>.zip` (Windows) or `.tar.gz` (macOS, Linux) from the
> project's Releases page, extract it, and run the commands from the extracted folder (`.\oc.exe` on Windows,
> `./oc` on macOS and Linux). If you are not comfortable with this, ask the project for help before trying.

Two ways it can work, which look the same from the outside:
- **after an interrupted installation** (the most common case): what you send is **written directly
  to memory** U6002 and stays after a power cut (based on analysis of the chip; **not yet
  validated** on the speaker);
- **after the startup escape** (part 4.6) or during development: what you send goes to **temporary
  memory** and is erased at the next power cut (**validated**).

The recipes below work in both cases.

### 4.1 Windows: WinUSB driver (administrator rights required)

Windows has no driver for this mode. You need to install **WinUSB** (a generic driver provided by Microsoft).
Installing the driver and restarting the USB device require an **administrator account**: open
PowerShell with right-click > "Run as administrator".

**Scripted method (ID `05A7:1020` only).** The script `tools\tas_dfu_run.ps1` installs the driver,
sends the firmware, restarts the USB device and then **removes the driver** by itself. See recipe 4.3 (a). This
script belongs to the development tree; it is not part of the release downloads (ask the project for it).

**Manual method with Zadig** (required for `FFFF:FFFE`). Download Zadig **only from its official
site: https://zadig.akeo.ie**; OpenCompanion does not host or distribute it.
1. Launch Zadig (it asks for administrator rights).
2. Menu **Options > List All Devices**.
3. Choose the device whose "USB ID" field is **05A7 1020** or **FFFF FFFE**. Double-check
   the ID: never install WinUSB on any other device.
4. To the right of the arrow, choose **WinUSB**, then **Replace Driver** (or **Install Driver**).
5. Check: `oc tas dfu list` must show `05a7:1020 DFU interface 0` (or `ffff:fffe ...`).

**Remove this driver after the recovery.** The Bose firmware uses the same ID `05A7:1020`: if
WinUSB stays installed, the speaker would no longer show up as a speaker. In an administrator PowerShell:
`pnputil /enum-drivers`, find the package whose provider is "libwdi" (name `oemNN.inf`), then
`pnputil /delete-driver oemNN.inf /uninstall /force`. Or, in Device Manager: right-click on
the device > **Uninstall device**, tick "Attempt to remove the driver for this device", then do a power cycle.
(The script does this by itself; the manual method is **not yet validated**.)

### 4.2 macOS and Linux

No driver to install (**not yet validated**). On Linux, you need to allow access: the
`70-opencompanion.rules` file published with each release (see [installation.md](installation.md#linux)) already
contains these lines; with an older copy of `/etc/udev/rules.d/70-opencompanion.rules`, replace its content with:
```
KERNEL=="hidraw*", ATTRS{idVendor}=="05a7", ATTRS{idProduct}=="1020", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0001", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0002", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="05a7", ATTR{idProduct}=="1020", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="ffff", ATTR{idProduct}=="fffe", TAG+="uaccess"
```
The first two lines are for normal operation (HID), the last two for DFU mode. Then
`sudo udevadm control --reload && sudo udevadm trigger`.

### 4.3 Recipes

Files needed: the firmware `oc_tas.dfu` and its data area `oc_tas_data.bin` (same version: both are in the
`opencompanion-firmware-<version>.zip` of the release, and in the app's download folder
`OpenCompanion/firmware/<release>/`, next to `backups/`), and your backup `u6002_tas.bin`.
Always start with a power cycle, then `oc install status` (TAS line: "TAS ROM in DFU mode").

#### (a) Reinstall OpenCompanion from DFU mode

1. Send the firmware **and restart the USB device immediately**:
   - Windows, `05A7:1020` (administrator PowerShell, `bose` folder):
     ```
     powershell -ExecutionPolicy Bypass -File tools\tas_dfu_run.ps1 opencompanion\firmware\usb\build\oc_tas\oc_tas.dfu
     ```
     You should see: `binding WinUSB to the DFU device...`, `05a7:1020 DFU interface 0`, the progress
     `.../...`, `download complete`, `restarting the USB device (bus reset)...`, `removing WinUSB package
     oemNN.inf`, then a table of devices. Log: `tools\tas_dfu_run.log`.
   - Windows, `FFFF:FFFE` (driver installed by Zadig, administrator PowerShell), **on a single line** so that
     the restart follows right away:
     ```
     $id = (Get-PnpDevice -PresentOnly | ? InstanceId -match 'VID_FFFF&PID_FFFE').InstanceId; oc tas dfu download oc_tas.dfu --to-ram --id ffff:fffe --yes; if ($LASTEXITCODE -eq 0) { pnputil /restart-device "$id" }
     ```
     (**not yet validated**). Then remove the driver (4.1).
   - macOS / Linux: `oc tas dfu download oc_tas.dfu --to-ram --yes` (the tool sends the restart itself;
     **not yet validated**).

   If `oc` shows "warning: USB reset request failed … replug the cable", **do not unplug the cable**:
   on Windows this is normal, the script (or `pnputil`) does this restart (see 4.5).
2. After a few seconds, the speaker should appear as `05A7:1020` "Companion 5 (OpenCompanion)" (OpenCompanion
   0.5.0+; 0.4.x appeared as `1209:0001` "Companion 5 dev") and
   `oc install status` should show OpenCompanion.
   - **Nothing appears**: do a power cycle. If the speaker then appears (OpenCompanion, see above), the firmware was written to
     memory: continue to step 3. If the speaker comes back in DFU mode, or if nothing appears, the firmware's data
     area is missing (without it the firmware does not connect to the computer): go to recipe (b).
3. Finish the installation: the app's wizard ("Resume"), or `oc install tas` (type `WRITE`
   to confirm; it reads everything back and only writes what is missing, data area included). Expected ending:
   "power-cycle the speaker to start the new TAS firmware". Do a power cycle.
4. Check: `oc install status`. Sound comes back with the OpenCompanion firmware.

Sending to temporary memory and then starting (step 1, script) are **validated**; the rest after an
interrupted installation is **not yet validated**.

#### (b) Go back to the Bose firmware from DFU mode

The Bose firmware must **never** be run from temporary memory to "repair" things: it starts but plays
no sound (observed). It has to be put back into memory U6002, from **your** backup.

- **If recipe (a) made the speaker appear as `1209:0001`** (even without a power cycle): `oc install
  restore` (both chips, audio processor first; `--tas` for the USB controller only; `--from <folder>`
  to choose the backup). Type `WRITE`. The USB controller restarts by itself on the Bose firmware
  (`05A7:1020`, recognized as a speaker). **Not yet validated.**
- **Otherwise, directly from DFU mode** (interrupted installation): WinUSB driver installed (4.1), then
  `oc tas restore-app --image <backup>/u6002_tas.bin --yes`; the progress ends with `download
  complete`. Do a power cycle: the speaker should come back as a speaker. Remove the WinUSB driver (4.1),
  then `oc tas mode app --yes` puts back the exact Bose header ("written and verified"). **Not yet validated.**
  If the speaker comes back in DFU mode after the power cycle, the chip was in temporary memory mode: use
  the previous case.
- **Development case** (the header was put into DFU mode by `oc tas mode dfu`): power cycle, script
  `tools\tas_dfu_run.ps1` with no argument (it loads the test firmware), then `uv run tools/tas_romdump.py
  --restore-header --yes` (it refuses to write if the header is not exactly the expected one, then reads it back),
  then power cycle. **Validated**: memory identical to the backup, sound OK.

#### (c) The speaker no longer appears at all: startup escape

See 4.6, then apply recipe (a) or (b) (temporary memory mode).

### 4.4 If the USB controller is running OpenCompanion

No need to go through these recipes: the OpenCompanion firmware can itself enter DFU mode without writing
anything (`uv run tools/oc_tas.py dfu`, **validated**, `05A7:1020` appears in ~1 s) or restart the chip
(`uv run tools/oc_tas.py reboot`, **validated**). These commands are development tools.

### 4.5 Pitfalls to avoid

- **Do not unplug the USB cable to "start" the firmware.** The chip pauses when the cable is
  removed and never comes out of that pause: it stays stuck until the next power cycle. Only a
  restart of the USB device by the computer (script, `pnputil /restart-device`) starts the firmware that was sent.
- **This restart must follow the send immediately.** If Windows puts the device to sleep in between, the
  chip gets stuck the same way. If this happens: power cycle, then start the recipe again.
- **The Bose firmware run from temporary memory is unusable**: it starts (green light) but plays no
  sound, and once showed up again as a DFU device. Do not mistake it for a repair.
- At the end of the send, the chip announces a wait of about 35 minutes (a bug in the chip): `oc` limits
  this wait itself, do not interrupt it.
- Do not forget to **remove the WinUSB driver** (4.1), otherwise the Bose speaker will no longer be recognized as a
  speaker.

### 4.6 Startup escape (OpenCompanion firmware installed but invisible)

If the USB controller's OpenCompanion firmware is installed but the computer no longer sees the speaker:
1. Do a power cycle.
2. As soon as the control pod's **red** light comes on, press the "mute" button **4 times** within
   **8 seconds**.
3. The USB controller switches to DFU mode (`05A7:1020`, temporary memory mode), without writing anything. Apply
   recipe (a) or (b).

**Not yet validated** on the speaker. Does not work without the control pod connected, or if the firmware
freezes before reading the button: in that case, part 5.

## 5. Last resort: opening the speaker

**Rarely needed**: only if none of parts 2 to 4 lets the computer talk to
the speaker. You then have to rewrite the affected memory directly from your backups. This is reserved for
someone with the right equipment and experience in electronics.

What you need:
- a screwdriver to open the enclosure, and protection against static electricity;
- an I²C **EEPROM programmer** (a small device that reads and writes this type of memory; the project used a
  Raspberry Pi Pico) and a **test clip** that fits the chip's package: U6002 = AT24C64A (USB controller,
  8 KB, TSSOP-8 package), U7001 = AT24C512 (audio processor, 64 KB, SOIC-8 package). Speaker **unplugged from
  the mains and from USB** during the operation;
- your backups `u6002_tas.bin` and `u7001_dsp.bin` (check their fingerprint before writing).

The cases concerned:
- **Power cut during the audio processor's sensitive step** (about 17 thousandths of a second during the
  first installation, orange bar): the chip no longer starts at all. Memory: U7001.
- **Serious defect in the OpenCompanion firmware** that would damage the audio processor's startup chain (the
  firmwares refuse these writes, but a defect remains possible). Memory: U7001.
- **USB controller firmware fully written but freezing at startup**, with no escape possible
  (4.6). Memory: U6002. The Texas Instruments procedure also suggests briefly shorting the
  SCL and SDA pins of this memory at power-on to force DFU mode: **not yet validated** on
  this speaker.
- **Audio processor firmware that starts and then freezes**, as long as requesting rescue mode from
  the app does not exist (part 3).

This guide is not a support service: if in doubt, ask the project for help before opening the speaker.
