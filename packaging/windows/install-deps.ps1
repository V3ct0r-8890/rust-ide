# Installs the tools rust-ide drives on Windows 10/11.
# Run in PowerShell:  powershell -ExecutionPolicy Bypass -File install-deps.ps1
# Everything comes from winget, rustup and pip; Visual Studio is not needed.

$ErrorActionPreference = "Stop"

function Have($name) { [bool](Get-Command $name -ErrorAction SilentlyContinue) }
function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                [Environment]::GetEnvironmentVariable("Path", "User") + ";" +
                "$env:USERPROFILE\.cargo\bin"
}
function Winget($id) {
    Write-Host "==> winget install $id" -ForegroundColor Cyan
    winget install --id $id -e --accept-source-agreements --accept-package-agreements
}

if (-not (Have winget)) {
    Write-Host "winget is missing. Install 'App Installer' from the Microsoft Store, then run this again." -ForegroundColor Red
    exit 1
}

# Rust (GNU toolchain) and the MinGW-w64 binutils some crates need.
if (-not (Have rustup)) { Winget "Rustlang.Rustup"; Refresh-Path }
rustup toolchain install stable-x86_64-pc-windows-gnu
# Only choose a default when none is set, so an existing setup is kept.
rustup show active-toolchain *> $null
if ($LASTEXITCODE -ne 0) { rustup default stable-x86_64-pc-windows-gnu }
if (-not (Have dlltool)) { Winget "BrechtSanders.WinLibs.POSIX.MSVCRT"; Refresh-Path }

# Python
if (-not (Have python)) { Winget "Python.Python.3.13"; Refresh-Path }

# Language servers for live errors and completion
Write-Host "==> rustup component add rust-analyzer" -ForegroundColor Cyan
rustup component add rust-analyzer
Write-Host "==> pip install --user basedpyright debugpy" -ForegroundColor Cyan
python -m pip install --user basedpyright debugpy

Write-Host ""
Write-Host "Done. Open a new terminal (so PATH is refreshed) and start rust-ide.exe." -ForegroundColor Green
Write-Host "In the IDE, Tools > Dependencies shows what was found."
