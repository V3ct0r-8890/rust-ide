#!/bin/sh
# Installs the tools rust-ide drives on macOS 11+ (Apple Silicon or Intel).
# Uses Homebrew for packages and rustup for Rust. Nothing needs sudo.
set -e

say() { printf '\n==> %s\n' "$*"; }

if ! xcode-select -p >/dev/null 2>&1; then
    say "Installing Apple's command line tools (a dialog opens; run this script again when it finishes)"
    xcode-select --install || true
    exit 0
fi

if ! command -v brew >/dev/null 2>&1; then
    for candidate in /opt/homebrew/bin/brew /usr/local/bin/brew; do
        [ -x "$candidate" ] && eval "$("$candidate" shellenv)" && break
    done
fi
if ! command -v brew >/dev/null 2>&1; then
    echo "Homebrew is not installed. Get it from https://brew.sh, then run this script again." >&2
    exit 1
fi

say "Installing packages with Homebrew"
brew install python basedpyright || true

if ! command -v rustup >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    say "Installing Rust with rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
PATH="$HOME/.cargo/bin:$PATH"

say "Installing rust-analyzer"
rustup component add rust-analyzer

# Homebrew's Python is "externally managed" (PEP 668), so pip --user is blocked for debugpy.
# Use Tools > Dependencies > Managed Python in the IDE, then install debugpy there.

say "Done. C# needs the .NET SDK: brew install --cask dotnet-sdk"
say "Open a new terminal, then start Rust IDE. Tools > Dependencies shows what was found."
