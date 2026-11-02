# Unsigned Windows release packages into dist\ (macOS and Linux: scripts/package_app.sh). Nothing is uploaded
# and nothing is signed.
#   OpenCompanion-<ver>-windows-x64.zip   portable app: opencompanion.exe (icon and version resource embedded by
#                                         oc-app's build.rs), LICENSE, licenses\
#   oc-<ver>-windows-x64.zip              the `oc` command-line tool (docs/recovery.md)
# Each artefact gets a `<file>.sha256` (`<hex>  <file>`, LF).
# The build goes to target\package-app (CARGO_TARGET_DIR), never target\release (the developer's own build).
# Code signing (later): `signtool sign /fd sha256 /tr <timestamp url> /td sha256 /f <cert> opencompanion.exe` on
# the staged exe, before zipping.
# Usage: scripts\package_app.ps1 [-OutDir dist]
param([string]$OutDir = 'dist')

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $Root 'target\package-app' }
$Target = 'x86_64-pc-windows-msvc'
$Label = 'windows-x64'
$Version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$Stage = Join-Path $env:CARGO_TARGET_DIR 'stage\windows'
$Bin = Join-Path $env:CARGO_TARGET_DIR "$Target\release"
$RepoRoot = Split-Path -Parent $Root
New-Item -ItemType Directory -Force $OutDir | Out-Null
$Out = (Resolve-Path $OutDir).Path

Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem

function Invoke-Cargo {
    & cargo @args
    if ($LASTEXITCODE -ne 0) { throw "cargo $args failed ($LASTEXITCODE)" }
}

# Zip of a folder, the folder itself as the top entry; entry names with '/' (readable by every unzip tool).
function New-Zip($dir, $zipPath) {
    if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
    $base = Split-Path -Parent $dir
    $zip = [System.IO.Compression.ZipFile]::Open($zipPath, 'Create')
    try {
        Get-ChildItem -Recurse -File $dir | ForEach-Object {
            $entry = $_.FullName.Substring($base.Length + 1).Replace('\', '/')
            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $_.FullName, $entry, 'Optimal') | Out-Null
        }
    } finally {
        $zip.Dispose()
    }
}

function Write-Checksum($file) {
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash.ToLowerInvariant()
    $name = Split-Path -Leaf $file
    [System.IO.File]::WriteAllText("$file.sha256", "$hash  $name`n")
    Write-Host "  $file"
}

function New-Package($name, [string[]]$files, [switch]$Licenses) {
    $dir = Join-Path $Stage $name
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
    New-Item -ItemType Directory -Force $dir | Out-Null
    Copy-Item $files $dir
    Copy-Item (Join-Path $RepoRoot 'LICENSE') (Join-Path $dir 'LICENSE.txt')
    if ($Licenses) { Copy-Item -Recurse (Join-Path $RepoRoot 'licenses') (Join-Path $dir 'licenses') }
    $zip = Join-Path $Out "$name.zip"
    New-Zip $dir $zip
    Write-Checksum $zip
}

Write-Host "OpenCompanion $Version -> $Out"
# Two invocations: the app's `release-updates` feature (firmware from the GitHub release) stays out of `oc`.
Invoke-Cargo build --release --locked -p oc-app --features release-updates --target $Target
Invoke-Cargo build --release --locked -p oc-cli --target $Target

New-Package "OpenCompanion-$Version-$Label" @(Join-Path $Bin 'opencompanion.exe') -Licenses
New-Package "oc-$Version-$Label" @(Join-Path $Bin 'oc.exe')
