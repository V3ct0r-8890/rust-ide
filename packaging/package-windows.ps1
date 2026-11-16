# Builds the release exe and packs dist\rust-ide-<version>-windows-x86_64.zip
# Usage (from the repository root):  powershell -File packaging\package-windows.ps1 [-NoBuild]
param([switch]$NoBuild)
$ErrorActionPreference = "Stop"

$root = Split-Path $PSScriptRoot
Set-Location $root
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$target = if ($env:CARGO_BUILD_TARGET) { "target\$env:CARGO_BUILD_TARGET\release" } else { "target\release" }

if (-not $NoBuild) {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$name = "rust-ide-$version-windows-x86_64"
$stage = "dist\$name"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$stage\docs" | Out-Null
Copy-Item "$target\rust-ide.exe" $stage
Copy-Item "$target\rust-ide-tui.exe" $stage
Copy-Item README.md $stage
Copy-Item docs\INSTALL.md "$stage\docs"
Copy-Item packaging\windows\install-deps.ps1 $stage

$zip = "dist\$name.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Host "Created $zip"

# Loose copies with the release number in the file names, easy to find and run.
Copy-Item "$target\rust-ide.exe" "dist\rust-ide-$version.exe"
Copy-Item "$target\rust-ide-tui.exe" "dist\rust-ide-tui-$version.exe"
Write-Host "Created dist\rust-ide-$version.exe and dist\rust-ide-tui-$version.exe"

