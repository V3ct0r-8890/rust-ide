#!/bin/sh
# Installs the tools rust-ide drives on Ubuntu 24.04+ and Fedora / RHEL 9/10 /
# Rocky / AlmaLinux. Uses sudo for system packages, rustup for Rust.
set -e

say() { printf '\n==> %s\n' "$*"; }

if command -v apt-get >/dev/null 2>&1; then
    say "Installing system packages with apt"
    sudo apt-get update
    sudo apt-get install -y build-essential curl python3 pipx gdb python3-debugpy \
        xdg-desktop-portal xdg-desktop-portal-gtk \
        libxkbcommon0 libxkbcommon-x11-0 libwayland-client0
    PY_LSP="pipx"
elif command -v dnf >/dev/null 2>&1; then
    say "Installing system packages with dnf"
    sudo dnf install -y gcc curl python3 python3-pip gdb \
        xdg-desktop-portal xdg-desktop-portal-gtk \
        libxkbcommon libxkbcommon-x11 libwayland-client
    PY_LSP="pip"
else
    echo "Unsupported system: need apt (Ubuntu/Debian) or dnf (Fedora/RHEL)." >&2
    echo "See https://github.com/V3ct0r-8890/rust-ide/blob/main/docs/INSTALL.md" >&2
    exit 1
fi

if ! command -v rustup >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    say "Installing Rust with rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
PATH="$HOME/.cargo/bin:$PATH"

say "Installing rust-analyzer"
rustup component add rust-analyzer

say "Installing basedpyright"
if [ "$PY_LSP" = "pipx" ]; then
    pipx install basedpyright || pipx upgrade basedpyright
    pipx ensurepath
else
    python3 -m pip install --user basedpyright debugpy
fi

# The debugger needs gdb 14 or newer (DAP support). RHEL 9 ships an older one.
gdb_major=$(gdb --version 2>/dev/null | head -1 | awk '{print $NF}' | cut -d. -f1)
if [ "${gdb_major:-0}" -lt 14 ] && command -v dnf >/dev/null 2>&1; then
    say "Installing a newer gdb (gcc-toolset-14-gdb)"
    sudo dnf install -y gcc-toolset-14-gdb || echo "Could not install gcc-toolset-14-gdb; Rust debugging needs gdb 14+."
fi

say "Done. Open a new terminal, then run rust-ide. Options > Dependencies shows what was found."
