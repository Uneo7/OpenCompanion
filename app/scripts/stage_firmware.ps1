# Wraps the firmware build outputs of the development tree into the two .ocfw containers in firmware\release\,
# the committed folder the release workflow packages (the DSP needs TI's compiler, so CI cannot build the
# firmware). The identity checks of the installer run first (`oc firmware pack`). Run it after building and
# testing both firmwares, check with `scripts\package_firmware.ps1 --from ..\firmware\release`, then commit.
# Format: docs\developer\firmware-releases.md.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$env:CARGO_TARGET_DIR = 'target\package'
cargo run --quiet -p oc-cli -- firmware pack --out ..\firmware\release @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# The raw build outputs are no longer committed: the containers carry them.
$rel = '..\firmware\release'
foreach ($f in 'boot', 'opencompanion_dsp.out', 'loader.out', 'oc_tas.dfu', 'oc_tas_data.bin') {
    $p = Join-Path $rel $f
    if (Test-Path $p) { Remove-Item -Recurse -Force $p }
}
Write-Host 'firmware\release updated'
