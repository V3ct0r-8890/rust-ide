#!/bin/sh
# Installs the tools rust-ide drives on Ubuntu 24.04+ and Fedora / RHEL 9/10 /
# Rocky / AlmaLinux. Uses sudo for system packages, rustup for Rust.
#
# Behind a firewall or proxy: export HTTPS_PROXY=http://proxy:3128 first (apt and dnf use
# their own proxy settings, see docs/INSTALL.md "Behind a firewall"). A step that cannot
# download is skipped and listed at the end with what to do by hand; the others still run.
set -u

FAILED=""
say() { printf '\n==> %s\n' "$*"; }

# step "name" command...   runs a command; on failure remembers it and carries on
step() {
    name=$1
    shift
    if ! "$@"; then
        echo "!! $name failed (network, proxy or firewall?)" >&2
        FAILED="$FAILED
  - $name"
        return 1
    fi
}

if command -v apt-get >/dev/null 2>&1; then
    say "Installing system packages with apt"
    step "apt update" sudo apt-get update
    step "system packages (apt)" sudo apt-get install -y build-essential curl python3 pipx gdb python3-debugpy \
        xdg-desktop-portal xdg-desktop-portal-gtk \
        libxkbcommon0 libxkbcommon-x11-0 libwayland-client0
    PY_LSP="pipx"
elif command -v dnf >/dev/null 2>&1; then
    say "Installing system packages with dnf"
    step "system packages (dnf)" sudo dnf install -y gcc curl python3 python3-pip gdb \
        xdg-desktop-portal xdg-desktop-portal-gtk \
        libxkbcommon libxkbcommon-x11 libwayland-client
    PY_LSP="pip"
else
    echo "Unsupported system: need apt (Ubuntu/Debian) or dnf (Fedora/RHEL)." >&2
    echo "See docs/INSTALL.md (or https://github.com/V3ct0r-8890/rust-ide/blob/main/docs/INSTALL.md)" >&2
    exit 1
fi

if ! command -v rustup >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    say "Installing Rust with rustup"
    step "Rust (rustup, needs sh.rustup.rs and static.rust-lang.org)" \
        sh -c "curl --proto '=https' --tlsv1.2 -sSf --connect-timeout 20 https://sh.rustup.rs | sh -s -- -y"
fi
PATH="$HOME/.cargo/bin:$PATH"

if command -v rustup >/dev/null 2>&1; then
    say "Installing rust-analyzer"
    step "rust-analyzer" rustup component add rust-analyzer
fi

say "Installing basedpyright"
if [ "$PY_LSP" = "pipx" ]; then
    step "basedpyright (pipx, needs pypi.org)" sh -c "pipx install basedpyright || pipx upgrade basedpyright"
    pipx ensurepath >/dev/null 2>&1 || true
else
    step "basedpyright and debugpy (pip, needs pypi.org)" python3 -m pip install --user basedpyright debugpy
fi

# The debugger needs gdb 14 or newer (DAP support). RHEL 9 ships an older one.
gdb_major=$(gdb --version 2>/dev/null | head -1 | awk '{print $NF}' | cut -d. -f1)
case "$gdb_major" in ''|*[!0-9]*) gdb_major=0 ;; esac
if [ "$gdb_major" -lt 14 ] && command -v dnf >/dev/null 2>&1; then
    say "Installing a newer gdb (gcc-toolset-14-gdb)"
    step "gdb 14+ (gcc-toolset-14-gdb)" sudo dnf install -y gcc-toolset-14-gdb
elif [ "$gdb_major" -lt 14 ]; then
    FAILED="$FAILED
  - gdb 14+ (found: ${gdb_major:-none}; Ubuntu 24.04 or newer has it)"
fi

if [ -n "$FAILED" ]; then
    printf '\n==> Finished, but these steps did not complete:%s\n' "$FAILED"
    cat <<'EOF'

If this machine is behind a firewall or has no internet:
  * Proxy:      export HTTPS_PROXY=http://user:pass@proxy:3128   then run this script again
  * Company CA: export CURL_CA_BUNDLE=/path/ca.pem  SSL_CERT_FILE=/path/ca.pem  PIP_CERT=/path/ca.pem
  * Mirrors:    RUSTUP_DIST_SERVER / RUSTUP_UPDATE_ROOT (Rust), pip config set global.index-url ...
  * By hand:    open rust-ide, press Alt+T D, select the row and press M for step-by-step
                manual / offline instructions (also in docs/INSTALL.md, "Behind a firewall").
EOF
    exit 2
fi
say "Done. Open a new terminal, then run rust-ide. Tools > Dependencies shows what was found."
