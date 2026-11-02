# OpenCompanion

Free firmware and settings app for the **Bose® Companion® 5** computer speaker system.

OpenCompanion replaces the two internal firmwares of the speaker (the audio processor and the USB controller) with
open ones, and gives you a desktop app for Windows, macOS and Linux to install them, keep them up to date and
change the settings. The speaker keeps working on its own as a standard USB audio device: the app is only needed to
install, update or change settings.

OpenCompanion is an independent project by the OpenCompanion contributors. It is not affiliated with, endorsed or
sponsored by Bose Corporation, and it is not a Bose product.

> **Status: first release.** The installation of the audio processor firmware and the sound itself are validated on
> the speaker under Windows. Some parts are not yet validated on a real speaker (writing the USB controller firmware
> to its memory, going back to the Bose firmware, macOS and Linux); the [installation guide](docs/installation.md)
> marks them "not yet validated". Read it before you start.

## Features

- **Guided installation** with an **automatic backup** of the original Bose firmware, taken and checked before
  anything is written. Every write is read back; an interrupted installation can be resumed.
- **Better USB audio**: stereo, 24 bits, 44.1 kHz or 48 kHz (the stock firmware: 16 bits, 48 kHz).
- **Volume knob and mute button in sync with the computer**: the control pod also moves the computer's volume and
  mute for this speaker, and the other way round (validated on Windows).
- **Firmware updates** from the app: it checks this project's GitHub releases, downloads the firmware and verifies
  its SHA-256 fingerprints before installing.
- **Safety first**: maximum volume limited to −12 dB until the speaker protections are fully checked; a rescue
  mode in the audio processor lets the app reinstall its firmware after a failed update.
- **Going back to the Bose firmware** from your backup, from the app.
- Single self-contained app: nothing else to install (no driver on Windows and macOS, one udev rule on Linux).

Coming later: equalizer and profiles adjusted from the app, high-quality 5.1 / 2.1 modes, automatic standby, a
faithful "Bose original" sound profile. The full comparison is in the
[installation guide](docs/installation.md#what-changes-for-you).

## Supported hardware and systems

| | |
|---|---|
| Speaker | **Bose Companion 5** only (satellites, subwoofer and control pod), connected to the computer with its USB cable |
| Windows | Windows 10 (1703 or later) or 11, x64. Tested system. |
| macOS | Apple Silicon and Intel (universal app). Not yet validated. |
| Linux | x86_64 and ARM64, any distribution with glibc 2.31 or later (Ubuntu 20.04, Debian 11, Fedora 32 and newer). Not yet validated. |

An Internet connection is needed for the installation: the app downloads the firmware from this project's GitHub
releases.

## Installation

### 1. Download

Get the files for your system from the [Releases](../../releases) page:

| System | File |
|---|---|
| Windows | `OpenCompanion-<version>-windows-x64.zip` |
| macOS | `OpenCompanion-<version>-macos-universal.dmg` (or `.zip`) |
| Linux x86_64 | `OpenCompanion-<version>-linux-x86_64.AppImage` and `70-opencompanion.rules` |
| Linux ARM64 | `OpenCompanion-<version>-linux-aarch64.AppImage` and `70-opencompanion.rules` |

Every file has a `.sha256` next to it, and `SHA256SUMS.txt` lists them all. To check a download:
`sha256sum -c SHA256SUMS.txt --ignore-missing` (Linux), `shasum -a 256 -c SHA256SUMS.txt --ignore-missing`
(macOS), `Get-FileHash <file>` (Windows PowerShell; compare with the `.sha256` file).

Then put the app in place:

- **Windows**: right-click the zip > **Extract All…**, then open the extracted folder. The app is portable:
  `opencompanion.exe` runs from wherever you put it.
- **macOS**: open the `.dmg` and drag **OpenCompanion** onto **Applications**.
- **Linux**: install the udev rule once (it lets the app talk to the speaker without root), then make the AppImage
  executable:
  ```
  sudo cp 70-opencompanion.rules /etc/udev/rules.d/
  sudo udevadm control --reload && sudo udevadm trigger
  chmod +x OpenCompanion-*.AppImage
  ```
  Unplug the speaker's USB cable and plug it back in.

### 2. First run of an unsigned build

The OpenCompanion packages are **not code-signed** yet, so the operating system warns the first time you open
them. This is expected; the checksums above let you check that the file is the one published here.

- **Windows (SmartScreen)**: "Windows protected your PC" appears. Click **More info**, then **Run anyway**. It is
  asked only once per version.
- **macOS (Gatekeeper)**: "OpenCompanion cannot be opened because Apple cannot check it for malicious software"
  (or "…developer cannot be verified").
  - macOS 14 Sonoma and earlier: in Applications, **right-click** (or Control-click) OpenCompanion > **Open**, then
    **Open** in the dialog.
  - macOS 15 Sequoia and later: open the app once (it is blocked), then go to **System Settings › Privacy &
    Security**, scroll down to "OpenCompanion was blocked…", click **Open Anyway** and confirm with your password.
  - If macOS says the app "is damaged and can't be opened", remove the download flag in Terminal:
    `xattr -dr com.apple.quarantine /Applications/OpenCompanion.app`, then open it again.
- **Linux**: the AppImage must be executable (`chmod +x`, or Properties > Permissions > "Allow executing file as
  program"), and the **udev rule** must be installed (step 1), otherwise the app does not see the speaker.

### 3. Guided installation in the app

1. Plug the speaker into the mains and into the computer with its USB cable (directly, not through a hub). Keep a
   laptop on its charger.
2. Open OpenCompanion. If the speaker still has its Bose firmware, the **Install OpenCompanion** wizard opens by
   itself (otherwise: Settings > Firmware).
3. Turn the computer's volume and the knob all the way down, then click **Start the installation**. The app
   downloads and checks the firmware, **backs up the original Bose firmware** on your computer (read twice and
   compared), then installs the audio processor and the USB controller firmwares, reading back everything it
   writes. It keeps the computer awake meanwhile. About 3 minutes.
4. **Do not unplug the speaker** (USB or mains) until the app says "OpenCompanion is installed". It may then ask
   you to unplug the speaker from the mains and plug it back in.

Keep the backup folder (`OpenCompanion/backups/`, see
[where are your files](docs/installation.md#where-are-your-files)): it is what lets you go back to the Bose
firmware. A copy on a USB stick or online storage is a good idea.

Step-by-step details, file locations and a glossary: **[docs/installation.md](docs/installation.md)**.

## Going back to the Bose firmware

In the app: **Settings > Firmware**, Backup row, **Restore…**. The wizard puts the original Bose firmware back on
both chips from the backup made at installation; the profiles stored in the speaker are erased. Not yet validated
on a real speaker. Details: [installation guide, part 4](docs/installation.md#4-going-back-to-the-bose-firmware).

## Troubleshooting

| Problem | What to do |
|---|---|
| The app does not see the speaker | Check the USB cable (directly on the computer, no hub) and that the speaker is on. Linux: install the udev rule (above) and replug the cable. |
| The installation stopped ("Interrupted") | Unplug the speaker from the mains for 10 seconds, plug it back in, then click **Resume**: what is already written is checked and not written again. |
| No sound after the installation | Unplug the speaker from the mains for 10 seconds and plug it back in. Check that the speaker is the selected output device. |
| The app offers "Update OpenCompanion" and the speaker is silent | The audio processor started in rescue mode: accept the update ([recovery guide, part 3](docs/recovery.md#3-audio-processor-rescue-mode)). |
| "GitHub unreachable" in Settings > Firmware | The app needs Internet to download firmware; an already downloaded firmware keeps working offline. Behind a proxy or after many checks in an hour, try again later. |
| Windows or macOS refuses to open the app | See [First run of an unsigned build](#2-first-run-of-an-unsigned-build). |
| The AppImage does not start (FUSE error) | Run it with `--appimage-extract-and-run`, or install your distribution's FUSE package. |
| The computer sees an unknown USB device, or nothing at all | Follow the **[recovery guide](docs/recovery.md)**. |

Logs (useful in a bug report): `%LOCALAPPDATA%\OpenCompanion\logs\` (Windows), `~/Library/Logs/OpenCompanion/`
(macOS), `~/.local/state/opencompanion/` (Linux).

The recovery guide uses the `oc` command-line tool for the rare cases the app cannot handle; it is published with
each release (`oc-<version>-<system>.zip` / `.tar.gz`).

## Licence

OpenCompanion is free software: you can redistribute it and/or modify it under the terms of the GNU General
Public License, version 3 or (at your option) any later version. See [LICENSE](LICENSE). It comes with no warranty;
installing an unofficial firmware is at your own risk (see the
[legal notices](docs/installation.md#5-legal-notices)). Third-party licences (fonts, icons, egui) are in
[licenses/](licenses/).

Copyright © the OpenCompanion contributors.

Bose® and Companion® are trademarks of Bose Corporation, used only to identify the hardware OpenCompanion works
with.

## For developers

| Folder | Contents | Language / tools |
|---|---|---|
| `app/` | Settings app (egui) and the `oc` command-line tool | Rust |
| `firmware/usb/` | TAS1020B USB controller firmware | C, SDCC |
| `firmware/dsp/` | TMS320DA705 DSP firmware | C, TI C6000 CGT (`cl6x`) |
| `firmware/release/` | Built firmware files of the next release (committed) | |

Developer documentation: [docs/developer/README.md](docs/developer/README.md). Building and packaging:
[app/README.md](app/README.md), releases: [docs/developer/releasing.md](docs/developer/releasing.md).
