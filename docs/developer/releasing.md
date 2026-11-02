# Releasing

A release is one GitHub release of [`Uneo7/OpenCompanion`](https://github.com/Uneo7/OpenCompanion), tagged
`v<version>` (the `version` of `app/Cargo.toml`). It carries the app packages for the three systems, the `oc`
command-line tool, the firmware archive the app downloads, and checksums. Nothing is code-signed yet.

## Assets

| Asset | Built by | Notes |
|---|---|---|
| `OpenCompanion-<ver>-windows-x64.zip` | `app/scripts/package_app.ps1` | portable: `opencompanion.exe` (icon and version resource from `oc-app/build.rs`), `LICENSE.txt`, `licenses/` |
| `OpenCompanion-<ver>-macos-universal.{dmg,zip}` | `app/scripts/package_app.sh macos` (Mac) | `OpenCompanion.app`, arm64 + x86_64 (`lipo`), ad-hoc signature (`codesign --sign -`) |
| `OpenCompanion-<ver>-linux-{x86_64,aarch64}.AppImage` | `app/scripts/package_app.sh linux-x86_64 linux-aarch64` | zig-linked against glibc 2.31 (Debian 11 sysroot); needs libudev.so.1 and libasound.so.2 on the host |
| `70-opencompanion.rules` | `app/packaging/linux/` | udev rule: HID 05a7:1020 (stock and OpenCompanion 0.5.0+), 1209:0001/0002 (0.4.x), DFU 05a7:1020 / ffff:fffe |
| `oc-<ver>-<system>.{zip,tar.gz}` | same scripts | the CLI, for [`recovery.md`](../recovery.md) |
| `opencompanion-firmware-<dsp>-<tas>.zip`, `.zip.sha256`, `firmware-manifest.json` | `app/scripts/package_firmware.sh --from ../firmware/release` | format: [`firmware-releases.md`](firmware-releases.md) |
| `<asset>.sha256`, `SHA256SUMS.txt` | scripts / release workflow | `sha256sum -c` format |

Packages go to `app/dist/`; builds to `app/target/package-app/` (never `target/release`, the developer's own
build). The production app is built with `--features release-updates` (firmware from the GitHub release only).

## Firmware files in the repository

CI cannot build the DSP firmware (TI's C6000 compiler is not redistributable), so the release packages the
**committed** files of [`firmware/release/`](../../firmware/release/README.md): the seven files of the release
layout, copied from the build outputs by `app/scripts/stage_firmware.{sh,ps1}`. Every other build output
(`firmware/**/build/`) is ignored by git. `.gitattributes` keeps these files byte-exact (`binary`).

## Workflows

- `.github/workflows/ci.yml`: on branch pushes and pull requests, build + test on `windows-latest`, `macos-latest`,
  `ubuntu-latest`, plus macOS x86_64 and Linux aarch64 (zig) builds.
- `.github/workflows/release.yml`: on a `v*` tag push: checks that the tag equals `v<app version>`, runs the CI
  workflow, packages on native runners (Windows; macOS universal with bundle and signature checks; Linux
  x86_64 + aarch64 with a glibc-floor check and an AppImage extraction test), packages the firmware from
  `firmware/release/`, verifies every `.sha256`, writes `SHA256SUMS.txt`, and creates a **draft** release
  (notes: `.github/release-notes.md`). Publishing the draft stays manual.

## Local checks

| Host | What can be built |
|---|---|
| Windows | `scripts\package_app.ps1` (complete); `bash scripts/package_app.sh linux-x86_64 linux-aarch64` from Git Bash (Linux binaries, `oc` archives, AppDir; no AppImage: appimagetool needs Linux) |
| Linux | `scripts/package_app.sh` (complete, both architectures) |
| macOS | `scripts/package_app.sh` (complete) |

macOS packages need a Mac: the Apple SDK is not redistributable, so elsewhere the macOS targets are only
type-checked (`scripts/check_all_targets.sh`).

## Steps

1. Build both firmwares, validate them on a speaker, then `app/scripts/stage_firmware.sh` and
   `app/scripts/package_firmware.sh --from ../firmware/release` (prints the versions).
2. Set `version` in `app/Cargo.toml` (`[workspace.package]`), `cargo build` to refresh `Cargo.lock`.
3. Commit (including `firmware/release/`), push, wait for CI to pass.
4. `git tag v<version> && git push origin v<version>`: the release workflow builds the draft.
5. On GitHub, Releases: check the draft (assets, notes), then **Publish**. From then on, installed apps see the
   new firmware (the update check reads the latest published release).

## Code signing (later)

- Windows: `signtool sign /fd sha256 /tr <timestamp URL> /td sha256` on `opencompanion.exe` and `oc.exe` before
  zipping (`package_app.ps1`); SmartScreen reputation still builds up over downloads.
- macOS: replace the ad-hoc `codesign --sign -` (`scripts/lib/package_macos.sh`) with a Developer ID identity and
  `--options runtime`, then `xcrun notarytool submit --wait` and `xcrun stapler staple` on the app and the dmg.
- Linux: AppImages can carry a GPG signature (`appimagetool --sign`); `SHA256SUMS.txt` can be signed alongside.
