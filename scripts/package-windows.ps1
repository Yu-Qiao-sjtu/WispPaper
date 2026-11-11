# Build Wisp Paper for Windows and package it as a portable zip:
# dist/WispPaper-v<version>-windows-x64.zip, holding WispPaper.exe (the app) and
# wisp.exe (the command line) in a folder of the same name.
#
#   pwsh -File scripts/package-windows.ps1          # unsigned, for a look on this machine
#   pwsh -File scripts/package-windows.ps1 -Sign    # Authenticode-sign both, as the release does
#
# -Sign goes through scripts/sign-windows-esigner.ps1 (SSL.com eSigner): run that
# script with -Install once before, and have the ESIGNER_* variables set. A
# signature that fails stops the packaging, so no half-signed zip is written.
#
# Environment: CARGO_TARGET_DIR is where cargo builds (default: target).

param(
    [switch]$Sign
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

# The version is written once, in the root Cargo.toml.
$match = Select-String -LiteralPath 'Cargo.toml' -Pattern '^version = "(.*)"$' | Select-Object -First 1
if (-not $match) { throw 'No version in Cargo.toml' }
$version = $match.Matches[0].Groups[1].Value

& cargo build --release -p wisp_paper -p wisp_cli
if ($LASTEXITCODE -ne 0) { throw "cargo build exited $LASTEXITCODE" }

$targetDir = $env:CARGO_TARGET_DIR
if ([string]::IsNullOrWhiteSpace($targetDir)) { $targetDir = 'target' }
$release = Join-Path $targetDir 'release'

$name = "WispPaper-v$version-windows-x64"
$dist = Join-Path $root 'dist'
$folder = Join-Path $dist $name
$zip = Join-Path $dist "$name.zip"
if (Test-Path -LiteralPath $folder) { Remove-Item -LiteralPath $folder -Recurse -Force }
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
New-Item -ItemType Directory -Force -Path $folder | Out-Null

# The app is a GUI-subsystem program; `wisp` beside it is the console one.
$app = Join-Path $folder 'WispPaper.exe'
$cli = Join-Path $folder 'wisp.exe'
Copy-Item -LiteralPath (Join-Path $release 'wisp_paper.exe') -Destination $app
Copy-Item -LiteralPath (Join-Path $release 'wisp.exe') -Destination $cli
Copy-Item -LiteralPath 'LICENSE', 'THIRD_PARTY.md', 'README.md' -Destination $folder

if ($Sign) {
    $signer = Join-Path $PSScriptRoot 'sign-windows-esigner.ps1'
    foreach ($file in @($app, $cli)) {
        & pwsh -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $signer -Path $file
        if ($LASTEXITCODE -ne 0) { throw "Signing $([IO.Path]::GetFileName($file)) failed (exit $LASTEXITCODE)" }
    }
} else {
    Write-Host 'Not signed: pass -Sign for a release.'
}

# The folder itself goes into the zip, so unpacking gives one folder.
Compress-Archive -LiteralPath $folder -DestinationPath $zip -CompressionLevel Optimal
Write-Host "folder: $folder"
Write-Host ("zip: $zip ({0:N1} MB)" -f ((Get-Item -LiteralPath $zip).Length / 1MB))
