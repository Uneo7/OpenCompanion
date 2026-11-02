# Installing OpenCompanion

Guide for owners of the Bose® Companion® 5 speaker. No technical knowledge is needed.
If you come across a word you don't know, see the short glossary at the end of this guide.

> **Project status: first release.** This guide describes the installation as the app presents it. Some parts
> have not yet been validated on a real speaker; they are marked "not yet validated" or "coming soon".
> Downloading the app and opening it the first time: see the [README](../README.md#installation).

## 1. What is OpenCompanion?

Your speaker contains two small computers, each with its own **firmware** (the internal software,
stored in one of the speaker's memories):

- the **audio processor**, which handles the sound (equalization, splitting the sound between the satellites and
  the subwoofer, speaker protections);
- the **USB controller**, which talks to the computer and receives the sound over the USB cable.

OpenCompanion replaces these two Bose firmwares with free, independent firmwares, and provides an
app for the computer. It is not a Bose product.

### What changes for you

| | Original Bose firmware | With OpenCompanion |
|---|---|---|
| Sound received from the computer | 6 channels, 16 bits, 48 kHz | stereo, 24 bits, 44.1 kHz or 48 kHz (validated) |
| Control pod's volume knob and "mute" button | act on the speaker only | also adjust the computer's volume and mute, for this speaker only (validated on Windows; on macOS, tracking of the volume slider has not been checked yet) |
| Without a computer (AUX input only) | the knob adjusts the volume | unchanged: the knob adjusts the volume |
| Maximum volume | full power | **limited to −12 dB** for safety, until the speaker protections have been checked on the speaker |
| Automatic standby | after 2 minutes without sound | **coming soon** (not available yet) |
| Headphones plugged into the control pod | mutes the speakers | mutes the speakers and also mutes the speaker in the computer; headphone sound is **not yet validated** |
| Equalizer and 4 profiles stored in the speaker | no | the firmware can handle them; adjusting them from the app is **coming soon** |
| "Original Bose sound" profile | — | **approximate**: it is not identical to the Bose sound; a faithful version is in progress |
| High-quality 5.1 and 2.1 modes | — | **coming soon** (today, the center and rear channels of 5.1 content are not played) |

## 2. What you need

- The Companion 5 speaker with its control pod, plugged into the mains.
- The USB cable that connects the speaker to the computer. If possible, plug it directly into the computer, not
  into a hub (a USB power strip-style box).
- The OpenCompanion app, from the project's Releases page on GitHub. The builds are not code-signed yet: the first
  time you open the app, Windows or macOS shows a warning; the [README](../README.md#2-first-run-of-an-unsigned-build)
  explains how to get past it.
- An **Internet connection**: the app downloads the firmware from the project's GitHub releases.
- A laptop must stay **plugged into its charger** during the installation.

### Windows
Windows 10 (version 1703 or later) or Windows 11. No driver to install: Windows recognizes
the speaker on its own. This is the system OpenCompanion has been tested on.

### macOS
No driver to install. **Not yet validated**: installation and sound have not been tested on a Mac yet.

### Linux
No driver to install, but Linux does not allow an ordinary app to talk to the speaker.
You need to add a **udev rule** (a small file that gives this permission to the logged-in user).
**Not yet validated** on Linux.

1. Download `70-opencompanion.rules` from the release (same page as the app) and copy it into
   `/etc/udev/rules.d/`: `sudo cp 70-opencompanion.rules /etc/udev/rules.d/`. Its content:
   ```
   KERNEL=="hidraw*", ATTRS{idVendor}=="05a7", ATTRS{idProduct}=="1020", TAG+="uaccess"
   KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0001", TAG+="uaccess"
   KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0002", TAG+="uaccess"
   SUBSYSTEM=="usb", ATTR{idVendor}=="05a7", ATTR{idProduct}=="1020", TAG+="uaccess"
   SUBSYSTEM=="usb", ATTR{idVendor}=="ffff", ATTR{idProduct}=="fffe", TAG+="uaccess"
   ```
   The first line covers the speaker with the Bose firmware and with the OpenCompanion firmware 0.5.0 or later
   (both use the USB ID `05A7:1020`); the two `1209` lines are only needed for a speaker still running an
   OpenCompanion firmware 0.4.x (IDs `1209:0001` / `1209:0002`). The last two lines are only used by the
   [recovery guide](recovery.md#42-macos-and-linux) (the USB controller's rescue loading mode).
2. In a terminal: `sudo udevadm control --reload && sudo udevadm trigger`
3. Unplug the speaker's USB cable, then plug it back in.
4. Make the app executable: `chmod +x OpenCompanion-*.AppImage` (or Properties > Permissions > "Allow executing
   file as program"), then open it.

## 3. The installation, step by step

On first launch, if the speaker still has its Bose firmware, the app opens the
"Install OpenCompanion" wizard on its own. You can also open it from **Settings > Firmware**.

Before starting, the wizard shows the expected duration and **a single warning**:

> **Do not unplug the speaker from the computer or the mains during the update.**

Also turn the computer's volume and the knob all the way down. During the installation, the app prevents
the computer from going to sleep.

Then, when you click "Start the installation", a progress bar moves through these steps:

1. **Download.** The app downloads the latest OpenCompanion firmware from the project's GitHub releases and
   checks its SHA-256 fingerprints. Without Internet, a firmware it already downloaded is used.
2. **Backup of the original Bose® firmware.** The app reads the USB controller's memory twice,
   checks that both reads are identical and saves it on your computer. For the audio processor,
   it checks that the speaker's memory matches the original Bose firmware and stores a copy of it in the same
   place. Nothing in the speaker has been changed yet.
3. **Preparation.** The app loads a small temporary program into the audio processor. Nothing is
   written to the speaker's memories.
4. **Audio processor.** The app first installs a startup chain with a **rescue mode**, then
   the OpenCompanion firmware. Each piece written is read back and checked. If this step is interrupted,
   the speaker restarts in rescue mode and the installation can resume (see the
   [recovery guide](recovery.md)). **Validated on the speaker.**
   - For a fraction of a second (about 17 thousandths of a second), the bar turns orange and shows
     "Sensitive step: do not unplug now". This is the only moment when a power cut would make
     the speaker unrecoverable without opening it.
5. **USB controller.** The app writes the USB controller's OpenCompanion firmware, page by page, reads everything back,
   and only writes the mark that says "installation complete" at the very end. If this step is interrupted, the chip
   starts in a rescue loading mode built into the chip (see the [recovery guide](recovery.md)).
   **Not yet validated**: the USB controller firmware has been tested on the speaker, but only loaded into
   temporary memory, not yet written to its memory by the wizard.

At the end, the app shows "OpenCompanion is installed". It may ask you to unplug
the speaker from the mains and then plug it back in: this is what starts the new USB controller firmware.

If the installation is interrupted (cable unplugged, power cut…), the wizard shows "Interrupted"
and a "Resume" button. The installation picks up where it stopped: what has already been written is read back and
is not written again.

### Duration
The app announces **about 3 minutes**. This is an estimate: the actual duration of a complete installation
has not been measured yet.

### Where are your files?
Backups are stored per speaker, in a dated folder (for example `2026-11-01_143012`):

| System | Backups |
|---|---|
| Windows | `%APPDATA%\OpenCompanion\backups\` |
| macOS | `~/Library/Application Support/OpenCompanion/backups/` |
| Linux | `~/.local/share/opencompanion/backups/` |

The app's other files:

| | Windows | macOS | Linux |
|---|---|---|---|
| Exported profiles | `%APPDATA%\OpenCompanion\profiles\` | `~/Library/Application Support/OpenCompanion/profiles/` | `~/.local/share/opencompanion/profiles/` |
| App settings | `%APPDATA%\OpenCompanion\config.json` | `~/Library/Application Support/OpenCompanion/config.json` | `~/.config/opencompanion/config.json` |
| Logs (if there is a problem) | `%LOCALAPPDATA%\OpenCompanion\logs\` | `~/Library/Logs/OpenCompanion/` | `~/.local/state/opencompanion/` |

If you had a previous version, the app moves these files on its own at first launch,
without deleting anything.

**Do not delete this folder.** It contains the files `u6002_tas.bin` (USB controller) and `u7001_dsp.bin`
(audio processor), which are needed to go back to the Bose firmware one day. Keep a copy elsewhere (USB stick,
online storage).

## 4. Going back to the Bose firmware

- **Before the installation is finished**, nothing is lost as long as the audio processor's sensitive step has not
  been passed: the speaker restarts with its Bose firmware.
- **After the installation**: in the app, **Settings > Firmware**, Backup row, **Restore…**. The "Restore the
  Bose® firmware" wizard puts the original firmware back on the audio processor, then on the USB controller, from
  the backup made at installation, and reads everything back. The profiles stored in the speaker are erased. The
  button stays greyed out while no backup is found: keep your backups. **Not yet validated** on a real speaker.
- If the speaker no longer works, follow the [recovery guide](recovery.md).

## 5. Legal notices

> Texts taken from the app (Settings > Legal notices). **Draft: these texts still need to be
> reviewed by a lawyer.** They do not constitute legal advice.

### Trademarks

Bose® and Companion® are trademarks of Bose Corporation.

OpenCompanion is an independent project. It is not affiliated with, endorsed or sponsored by Bose Corporation,
and it is not a Bose product. The names Bose and Companion 5 are used only to indicate the hardware
OpenCompanion works with.

Other product names and trademarks mentioned belong to their respective owners.

### No warranty

OpenCompanion is free software distributed under the GNU General Public License (GPL), version 3 or (at your
option) any later version (file `LICENSE`). Only the English text of the license is authoritative.

The software is provided "as is", without warranty of any kind, express or implied, to the extent
permitted by applicable law.

These exclusions apply to the extent permitted by law. They do not deprive anyone of the rights that
applicable law grants them on a mandatory basis.

### Firmware risks

OpenCompanion replaces your speaker's original firmware with an unofficial firmware that was neither designed
nor tested by Bose®. You install it at your own risk.

- **Speaker unusable.** If the installation or an update is interrupted (cable unplugged, power
  cut, computer going to sleep) or if the firmware has a defect, the speaker may stop
  working. A recovery procedure exists, but it does not succeed in every case.
- **Unexpected sound.** A firmware defect or a wrong setting can produce abnormal, loud or distorted sound.
  The protection limiter stays active, but it cannot cover every defect. Keep the volume moderate
  during your first tries.
- **"Bose® original" profile.** This profile approximates the factory setting. It is not
  identical to the sound of the Bose® firmware.
- **Warranties.** A failure caused by OpenCompanion or by its installation may not be covered, neither
  by Bose® nor by your seller. Bose® provides no support for this firmware.
- **Going back to the Bose® firmware.** Going back to the original firmware from the app is **coming soon** (see
  part 4). It will depend on the backup made at installation, and the profiles stored in the speaker
  will then be erased.

## Short glossary

| Word | Meaning |
|---|---|
| Firmware | A device's internal software, stored in one of the device's memories. |
| Audio processor (DSP) | The speaker chip that computes the sound. |
| USB controller | The speaker chip that talks to the computer over the USB cable. |
| Memory (EEPROM) | A small memory that keeps the firmware when the speaker is off. |
| Rescue mode | A minimal startup mode of the audio processor, which lets you reinstall its firmware over USB. |
| Control pod | The small round box connected to the speaker, with the volume knob, the "mute" button and the headphone jack. |
| 24 bits, 44.1 / 48 kHz | Precision and rate of the sound sent by the computer; 44.1 kHz is the CD rate. |
| udev rule | A Linux configuration file that allows access to a USB device. |
