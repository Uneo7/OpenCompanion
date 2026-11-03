# Troubleshooting

"**Power cycle**" below = unplug the speaker's power plug for 10 seconds, plug it back in, USB cable left
connected.

## Common issues

| Problem | Fix |
|---|---|
| The app doesn't see the speaker | USB cable straight into the computer (no hub), speaker on. Linux: install the [udev rule](install.md#linux-the-udev-rule) and replug the cable. |
| Windows or macOS refuses to open the app | See [unsigned builds](install.md#first-launch-unsigned-builds). |
| The AppImage doesn't start (FUSE error) | Run it with `--appimage-extract-and-run`, or install your distribution's FUSE package. |
| The install stopped ("Interrupted") | Power cycle, then **Resume**. What's already written is checked, not rewritten. |
| No sound after the install | Power cycle. Check the speaker is the selected output device. |
| The app offers "Update OpenCompanion" and the speaker is silent | The audio processor started in rescue mode: accept the update ([details](recovery.md#3-audio-processor-rescue-mode)). |
| "GitHub unreachable" in Settings > Firmware | The app needs Internet to download firmware; one already downloaded works offline. Behind a proxy, or after many checks in an hour, try again later. |
| The computer sees an unknown USB device, or nothing at all | Follow the [recovery guide](recovery.md). |

## Logs

Useful in a bug report:

| System | Logs |
|---|---|
| Windows | `%LOCALAPPDATA%\OpenCompanion\logs\` |
| macOS | `~/Library/Logs/OpenCompanion/` |
| Linux | `~/.local/state/opencompanion/` |

## Still stuck?

Open an issue on [GitHub](https://github.com/Uneo7/OpenCompanion/issues) with the log and what you see.
