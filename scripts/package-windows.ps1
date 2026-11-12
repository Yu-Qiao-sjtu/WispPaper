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
    [switch]$Sign,
    # The fork's explicitly unsigned community package, with an in-package notice.
    [switch]$ForkRelease
)

$ErrorActionPreference = 'Stop'
if ($Sign -and $ForkRelease) { throw 'Choose signed packaging or an explicitly unsigned fork package.' }

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
# Every recursive cleanup target must stay inside this checkout's dist directory.
$distAbsolute = [IO.Path]::GetFullPath($dist).TrimEnd([IO.Path]::DirectorySeparatorChar)
foreach ($destination in @($folder, $zip)) {
    $resolved = [IO.Path]::GetFullPath($destination)
    if (-not $resolved.StartsWith($distAbsolute + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Packaging output escapes dist: $resolved"
    }
}
if (Test-Path -LiteralPath $folder) { Remove-Item -LiteralPath $folder -Recurse -Force }
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
New-Item -ItemType Directory -Force -Path $folder | Out-Null

# The app is a GUI-subsystem program; `wisp` beside it is the console one.
$app = Join-Path $folder 'WispPaper.exe'
$cli = Join-Path $folder 'wisp.exe'
Copy-Item -LiteralPath (Join-Path $release 'wisp_paper.exe') -Destination $app
Copy-Item -LiteralPath (Join-Path $release 'wisp.exe') -Destination $cli
Copy-Item -LiteralPath 'LICENSE', 'THIRD_PARTY.md', 'README.md' -Destination $folder
if ($ForkRelease) {
    $guide = Join-Path $folder 'docs'
    New-Item -ItemType Directory -Path $guide | Out-Null
    Copy-Item -LiteralPath 'docs/AI-WRITING.md', 'docs/DATA.md' -Destination $guide
    $notice = @"
Wisp Paper Fork v$version — Windows x64

This is the Yu-Qiao-sjtu community fork, not an upstream signed build.
WispPaper.exe and wisp.exe do not have an Authenticode signature.
Download and verify SHA256SUMS from:
https://github.com/Yu-Qiao-sjtu/WispPaper/releases/tag/v$version

Extract the whole folder and run WispPaper.exe. The CLI is wisp.exe.
Back up your literature library before first use. Settings and libraries
use the existing WispPaper locations; WISP_PAPER_HOME can isolate a copy.

Usage: docs/AI-WRITING.md
Source and issues: https://github.com/Yu-Qiao-sjtu/WispPaper
Upstream author/project: https://github.com/xuzhougeng/WispPaper
"@
    [IO.File]::WriteAllText((Join-Path $folder 'FORK-NOTICE.txt'), $notice, [Text.UTF8Encoding]::new($false))
}

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
