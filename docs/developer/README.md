# OpenCompanion developer documentation

Contributor-facing index. User guides: [`../../README.md`](../../README.md), [`../README.md`](../README.md) (index),
[`../recovery.md`](../recovery.md).

## Status (2026-11-01)
Validated on hardware:
- both OpenCompanion firmwares run: the DSP firmware is installed in U7001 through the boot chain (boot0 /
  boot1 with rescue mode); the USB controller (TAS1020B) firmware so far runs **from RAM only** (ROM DFU);
- USB Audio Class 2.0, stereo, 24-bit, 48 and 44.1 kHz;
- pod knob and mute synced with the OS (Windows);
- DSP update by the normal path (`UPD` → rescue → updater → image);
- software DFU entry (`REBOOT_DFU`, HID command 0x7E).

In progress: 5.1 / 2.1 HQ modes, the faithful "Bose d'origine" profile, the app backend (oc-device ↔ oc-app).
Not validated: writing the TAS firmware to U6002 (`oc install tas`), restore to stock (no flow in `oc-install`
yet), macOS and Linux.

## Where things are
| Topic | Document |
|---|---|
| Project overview, user README | [`../../README.md`](../../README.md); user guides (install, unsigned builds, troubleshooting): [`../README.md`](../README.md) |
| Development loop, safety rules | [`dev-loop.md`](dev-loop.md) |
| Firmware releases on GitHub, production build | [`firmware-releases.md`](firmware-releases.md) |
| Packaging, CI and release workflows, committed firmware files, publishing | [`releasing.md`](releasing.md) |
| Rust workspace, `oc` CLI, WinUSB / udev notes | [`../../app/README.md`](../../app/README.md) |
| USB controller firmware (TAS1020B, SDCC): UAC2, data area, DFU workflow, test plan | [`../../firmware/usb/README.md`](../../firmware/usb/README.md) |
| Vendor HID protocol of our TAS firmware | [`../../../re/docs/protocol_usb_hid.md`](../../../re/docs/protocol_usb_hid.md) |
| DSP firmware (TMS320DA705, CGT 7.4.24): architecture, console, protection, test plan | [`../../firmware/dsp/README.md`](../../firmware/dsp/README.md) |
| DSP RAM loader (stock-firmware hijack) | [`../../firmware/dsp/README.md`](../../firmware/dsp/README.md) (RAM loader section) |
| DSP boot chain (boot0, boot1, updater) | [`../../firmware/dsp/boot/README.md`](../../firmware/dsp/boot/README.md) |
| U7001 EEPROM map, image header | [`../../firmware/dsp/boot/docs/LAYOUT.md`](../../firmware/dsp/boot/docs/LAYOUT.md) |
| Parametric base: block diagram, JSON, EEPROM v4, `oc base` (spec, not implemented) | [`base-format.md`](base-format.md) |
| Rescue / updater protocol | [`../../firmware/dsp/boot/docs/RESCUE.md`](../../firmware/dsp/boot/docs/RESCUE.md) |
| Migration, update, power-loss analysis | [`../../firmware/dsp/boot/docs/UPDATE.md`](../../firmware/dsp/boot/docs/UPDATE.md) |
| App UI strings (FR / EN) | [`../../app/crates/oc-app/i18n/`](../../app/crates/oc-app/i18n/) (`fr.json`, `en.json`), loader: `app/crates/oc-app/src/i18n/` |
| App photo, sources and processing | [`../../app/crates/oc-app/assets/photo/SOURCES.md`](../../app/crates/oc-app/assets/photo/SOURCES.md) |
| Licence texts | [`../../LICENSE`](../../LICENSE) (GPL-3.0-or-later), third-party: [`../../licenses/`](../../licenses/) |
| Reverse engineering of the stock firmware (French) | `../../../doc.md`, `../../../re/` |
