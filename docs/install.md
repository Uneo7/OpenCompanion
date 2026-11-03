# Install the app

## What you need

- A **Bose® Companion® 5** (satellites, subwoofer, control pod), plugged into the mains.
- Its **USB cable**, plugged directly into the computer (not through a hub).
- An **Internet connection**: the app downloads the firmware from this project's GitHub releases.
- One of:
  - **Windows** 10 (1703 or later) or 11, x64;
  - **macOS**, Apple Silicon or Intel (universal app);
  - **Linux** x86_64 or ARM64, glibc 2.31 or later (Ubuntu 20.04, Debian 11, Fedora 32 and newer).

No driver to install. Linux needs one udev rule (below).

## Download

From the [Releases](https://github.com/Uneo7/OpenCompanion/releases) page:

| System | File |
|---|---|
| Windows | `OpenCompanion-<version>-windows-x64.zip` |
| macOS | `OpenCompanion-<version>-macos-universal.dmg` (or `.zip`) |
| Linux x86_64 | `OpenCompanion-<version>-linux-x86_64.AppImage` + `70-opencompanion.rules` |
| Linux ARM64 | `OpenCompanion-<version>-linux-aarch64.AppImage` + `70-opencompanion.rules` |

### Check the download (optional)

Every file has a `.sha256`; `SHA256SUMS.txt` lists them all.

- Linux: `sha256sum -c SHA256SUMS.txt --ignore-missing`
- macOS: `shasum -a 256 -c SHA256SUMS.txt --ignore-missing`
- Windows (PowerShell): `Get-FileHash <file>`, then compare with the `.sha256` file.

## Put it in place

- **Windows**: right-click the zip > **Extract All…**. The app is portable: run `opencompanion.exe` from
  wherever you put it.
- **macOS**: open the `.dmg`, drag **OpenCompanion** onto **Applications**.
- **Linux**: install the udev rule, then make the AppImage executable (see below).

### Linux: the udev rule

Linux doesn't let an ordinary app talk to the speaker. The rule gives that permission to the logged-in user.

```
sudo cp 70-opencompanion.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
chmod +x OpenCompanion-*.AppImage
```

Then unplug the speaker's USB cable and plug it back in.

What the rule contains:

```
KERNEL=="hidraw*", ATTRS{idVendor}=="05a7", ATTRS{idProduct}=="1020", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0001", TAG+="uaccess"
KERNEL=="hidraw*", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="0002", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="05a7", ATTR{idProduct}=="1020", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="ffff", ATTR{idProduct}=="fffe", TAG+="uaccess"
```

- Line 1: the speaker with the Bose firmware, or OpenCompanion firmware 0.5.0 or later (both `05A7:1020`).
- Lines 2-3: only for a speaker still on OpenCompanion firmware 0.4.x (`1209:0001` / `1209:0002`).
- Lines 4-5: only for [recovery](recovery.md#42-macos-and-linux) (the USB controller's DFU mode).

## First launch: unsigned builds

The packages aren't code-signed yet, so your system warns the first time. That's expected; the checksums above
prove the file is the one published here.

### Windows (SmartScreen)

"Windows protected your PC": click **More info**, then **Run anyway**. Asked once per version.

### macOS (Gatekeeper)

"OpenCompanion cannot be opened because Apple cannot check it for malicious software" (or "…developer cannot
be verified"):

- **macOS 14 Sonoma and earlier**: in Applications, right-click (or Control-click) OpenCompanion > **Open**,
  then **Open** again.
- **macOS 15 Sequoia and later**: open the app once (it's blocked), then **System Settings › Privacy &
  Security**, scroll to "OpenCompanion was blocked…", click **Open Anyway** and confirm with your password.
- **"The app is damaged and can't be opened"**: in Terminal,
  `xattr -dr com.apple.quarantine /Applications/OpenCompanion.app`, then open it again.

### Linux

- The AppImage must be executable (`chmod +x`, or Properties > Permissions > "Allow executing file as program").
- The udev rule must be installed, or the app won't see the speaker.
- FUSE error? Run it with `--appimage-extract-and-run`, or install your distribution's FUSE package.

## Next

[First setup](first-setup.md): install OpenCompanion on the speaker.
