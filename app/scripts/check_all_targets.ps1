# Needs: rustup, zig >= 0.13 on PATH, cargo-zigbuild (`cargo install cargo-zigbuild`), curl.exe and tar.exe
# (both shipped with Windows 10+).
# Usage: scripts\check_all_targets.ps1 [-Release] [-Targets t1,t2]
param(
    [switch]$Release,
    [string[]]$Targets = @('x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu', 'x86_64-apple-darwin', 'aarch64-apple-darwin')
)

$ErrorActionPreference = 'Continue'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$Sysroots = Join-Path $TargetDir 'cross-sysroot'
$Debian = 'https://deb.debian.org/debian/pool/main'
$UdevVer = '247.3-7+deb11u5'
$AlsaVer = '1.2.4-1.1'
$CargoArgs = @()
if ($Release) { $CargoArgs = @('--release') }
$Results = New-Object System.Collections.Generic.List[object]

function Record($name, $status) { $Results.Add([pscustomobject]@{ Target = $name; Status = $status }) }

function Require($cmd, $hint) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { Write-Error "missing: $cmd ($hint)"; exit 2 }
}
Require cargo 'https://rustup.rs'
Require rustup 'https://rustup.rs'

function Ensure-Target($t) {
    if (-not ((rustup target list --installed) -contains $t)) { rustup target add $t | Out-Null }
}

function Fetch-Sysroot($arch, $multiarch) {
    $dst = Join-Path $Sysroots $arch
    $lib = Join-Path $dst "usr\lib\$multiarch"
    if ((Test-Path "$lib\libudev.so") -and (Test-Path "$lib\libasound.so")) { return $true }
    $debs = Join-Path $Sysroots 'debs'
    New-Item -ItemType Directory -Force $dst, $debs | Out-Null
    $pkgs = @("s/systemd/libudev1_$UdevVer", "s/systemd/libudev-dev_$UdevVer",
              "a/alsa-lib/libasound2_$AlsaVer", "a/alsa-lib/libasound2-dev_$AlsaVer")
    foreach ($p in $pkgs) {
        $deb = Join-Path $debs ((Split-Path -Leaf $p) + "_$arch.deb")
        if (-not (Test-Path $deb)) {
            curl.exe -sfL -o $deb "$Debian/${p}_$arch.deb"
            if ($LASTEXITCODE -ne 0) { Write-Host "download failed: $p"; return $false }
        }
        $x = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
        New-Item -ItemType Directory $x | Out-Null
        tar.exe -xf $deb -C $x                              # bsdtar reads the .deb (ar) container
        $data = Get-ChildItem $x -Filter 'data.tar.*' | Select-Object -First 1
        tar.exe -xf $data.FullName -C $dst 2>$null          # dev symlinks may fail; copied below
        Remove-Item -Recurse -Force $x
    }
    # Real files are libudev.so.1.N.N / libasound.so.2.N.N (their SONAME stays .so.1 / .so.2).
    Copy-Item -Force (Get-Item "$lib\libudev.so.1.*").FullName "$lib\libudev.so"
    Copy-Item -Force (Get-Item "$lib\libasound.so.2.*").FullName "$lib\libasound.so"
    return $true
}

# pkg-config stand-in: the -sys build scripts only ask for link flags (headers come from CFLAGS_<target>).
function Write-PkgConfigShim {
    $shim = Join-Path $Sysroots 'pkg-config.cmd'
    @'
@echo off
setlocal
echo %* | findstr /c:"--modversion" >nul && (echo 1.0& exit /b 0)
set L=
echo %* | findstr /c:"libudev" >nul && set L=udev
echo %* | findstr /c:" alsa" >nul && set L=asound
if "%L%"=="" (echo pkg-config shim: unknown package in %* 1>&2& exit /b 1)
echo -L%OC_XLIBDIR% -l%L%
'@ | Set-Content -Encoding ascii $shim
    return $shim
}

Write-Host '=== host: build + test'
cargo build --workspace --all-targets @CargoArgs
$ok = $LASTEXITCODE -eq 0
if ($ok) { cargo test --workspace @CargoArgs; $ok = $LASTEXITCODE -eq 0 }
Record 'host' $(if ($ok) { 'build+test OK' } else { 'FAILED' })

foreach ($t in $Targets) {
    Write-Host "=== $t"
    Ensure-Target $t
    if ($t -like '*-linux-gnu') {
        Require zig 'https://ziglang.org/download/'
        cargo zigbuild --help *> $null
        if ($LASTEXITCODE -ne 0) { Write-Error 'missing: cargo-zigbuild (cargo install cargo-zigbuild)'; exit 2 }
        if ($t -like 'x86_64-*') { $arch = 'amd64'; $ma = 'x86_64-linux-gnu' } else { $arch = 'arm64'; $ma = 'aarch64-linux-gnu' }
        if (-not (Fetch-Sysroot $arch $ma)) { Record $t 'FAILED (sysroot)'; continue }
        $sr = (Join-Path $Sysroots $arch) -replace '\\', '/'
        $cflagsVar = 'CFLAGS_' + ($t -replace '-', '_')
        $env:PKG_CONFIG = Write-PkgConfigShim
        $env:PKG_CONFIG_ALLOW_CROSS = '1'
        $env:OC_XLIBDIR = "$sr/usr/lib/$ma"
        Set-Item "env:$cflagsVar" "-I$sr/usr/include"
        cargo zigbuild --workspace --all-targets --target $t @CargoArgs
        $ok = $LASTEXITCODE -eq 0
        Remove-Item "env:$cflagsVar", env:PKG_CONFIG, env:PKG_CONFIG_ALLOW_CROSS, env:OC_XLIBDIR
        Record $t $(if ($ok) { 'build OK (linked)' } else { 'FAILED' })
    }
    elseif ($t -like '*-apple-darwin') {
        if ($env:SDKROOT -and (Get-Command zig -ErrorAction SilentlyContinue)) {
            cargo zigbuild --workspace --all-targets --target $t @CargoArgs
            Record $t $(if ($LASTEXITCODE -eq 0) { 'build OK (zig + SDKROOT)' } else { 'FAILED' })
        }
        else {
            cargo check --workspace --all-targets --target $t @CargoArgs --config "target.$t.hidapi.rustc-cfg=['hidapi']"
            Record $t $(if ($LASTEXITCODE -eq 0) { 'check OK (no SDK: hidapi C and link not verified)' } else { 'FAILED' })
        }
    }
    else {
        cargo build --workspace --all-targets --target $t @CargoArgs
        Record $t $(if ($LASTEXITCODE -eq 0) { 'build OK' } else { 'FAILED' })
    }
}

Write-Host ''
Write-Host '=== summary'
$Results | Format-Table -AutoSize | Out-String | Write-Host
if ($Results | Where-Object { $_.Status -like 'FAILED*' }) { exit 1 }
exit 0
