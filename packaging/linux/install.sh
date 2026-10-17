#!/bin/sh
# Installs rust-ide for the current user: the binary to ~/.local/bin and a
# menu entry to ~/.local/share/applications. No root needed.
set -e
here=$(cd "$(dirname "$0")" && pwd)
bin="$HOME/.local/bin"
apps="$HOME/.local/share/applications"

mkdir -p "$bin" "$apps"
install -m 755 "$here/rust-ide" "$bin/rust-ide"
install -m 755 "$here/rust-ide-tui" "$bin/rust-ide-tui"
sed "s|^Exec=.*|Exec=$bin/rust-ide %f|" "$here/rust-ide.desktop" > "$apps/rust-ide.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" || true

echo "Installed $bin/rust-ide (GUI), $bin/rust-ide-tui (terminal) and a menu entry."
case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "Note: $bin is not on your PATH yet; log out and in, or add it to ~/.profile." ;;
esac
echo "Missing tools? Run ./install-deps.sh"
