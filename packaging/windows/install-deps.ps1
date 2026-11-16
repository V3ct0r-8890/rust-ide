# Installs the tools rust-ide drives on Windows 10/11.
# Run in PowerShell:  powershell -ExecutionPolicy Bypass -File install-deps.ps1
# Everything comes from winget, rustup and pip; Visual Studio is not needed.
#
# Behind a firewall or proxy:  $env:HTTPS_PROXY = "http://user:pass@proxy:3128"  first
# (winget also follows the Windows proxy settings). A step that cannot download is skipped and
# listed at the end with what to do by hand; the others still run.

$Failed = New-Object System.Collections.Generic.List[string]

function Have($name) { [bool](Get-Command $name -ErrorAction SilentlyContinue) }
function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                [Environment]::GetEnvironmentVariable("Path", "User") + ";" +
                "$env:USERPROFILE\.cargo\bin"
}
# Step "name" { commands }  - runs commands; a failure is remembered and the script carries on
function Step($name, [scriptblock]$body) {
    Write-Host "==> $name" -ForegroundColor Cyan
    try {
        & $body
        if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) { throw "exit code $LASTEXITCODE" }
    } catch {
        Write-Host "!! $name failed ($_). Network, proxy or firewall?" -ForegroundColor Yellow
        $Failed.Add($name)
        $global:LASTEXITCODE = 0
    }
}
function Winget($id) {
    winget install --id $id -e --accept-source-agreements --accept-package-agreements
}

if (-not (Have winget)) {
    Write-Host "winget is missing. Install 'App Installer' from the Microsoft Store, then run this again," -ForegroundColor Red
    Write-Host "or install the tools by hand: see docs\INSTALL.md, 'Behind a firewall'." -ForegroundColor Red
    exit 1
}

# Rust (GNU toolchain) and the MinGW-w64 binutils some crates need.
if (-not (Have rustup)) { Step "Rust (rustup)" { Winget "Rustlang.Rustup" }; Refresh-Path }
if (Have rustup) {
    Step "Rust GNU toolchain (static.rust-lang.org)" { rustup toolchain install stable-x86_64-pc-windows-gnu }
    # Only choose a default when none is set, so an existing setup is kept.
    rustup show active-toolchain *> $null
    if ($LASTEXITCODE -ne 0) { rustup default stable-x86_64-pc-windows-gnu }
    $LASTEXITCODE = 0
}
if (-not (Have dlltool)) { Step "MinGW-w64 and gdb (WinLibs)" { Winget "BrechtSanders.WinLibs.POSIX.MSVCRT" }; Refresh-Path }

# Python
if (-not (Have python)) { Step "Python 3.13" { Winget "Python.Python.3.13" }; Refresh-Path }

# Language servers for live errors and completion
if (Have rustup) { Step "rust-analyzer" { rustup component add rust-analyzer } }
if (Have python) { Step "basedpyright and debugpy (pypi.org)" { python -m pip install --user basedpyright debugpy } }

Write-Host ""
if ($Failed.Count -gt 0) {
    Write-Host "Finished, but these steps did not complete:" -ForegroundColor Yellow
    $Failed | ForEach-Object { Write-Host "  - $_" -ForegroundColor Yellow }
    Write-Host ""
    Write-Host "If this computer is behind a firewall or has no internet:"
    Write-Host '  * Proxy:      $env:HTTPS_PROXY = "http://user:pass@proxy:3128"   then run this script again'
    Write-Host '  * Company CA: $env:SSL_CERT_FILE = "C:\path\ca.pem"; $env:PIP_CERT = "C:\path\ca.pem"'
    Write-Host '  * Mirrors:    $env:RUSTUP_DIST_SERVER / RUSTUP_UPDATE_ROOT (Rust), pip config set global.index-url ...'
    Write-Host "  * By hand:    start rust-ide.exe, press Alt+T D, select the row and press M for step-by-step"
    Write-Host "                manual / offline instructions (also docs\INSTALL.md, 'Behind a firewall')."
    exit 2
}
Write-Host "Done. Open a new terminal (so PATH is refreshed) and start rust-ide.exe." -ForegroundColor Green
Write-Host "In the IDE, Tools > Dependencies shows what was found."
