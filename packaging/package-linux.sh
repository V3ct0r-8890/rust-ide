#!/bin/sh
# Builds the release binary and packs dist/rust-ide-<version>-linux-x86_64.tar.gz
# Usage (from the repository root):  sh packaging/package-linux.sh [--no-build]
set -e
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
target_dir=${CARGO_TARGET_DIR:-target}

if [ "$1" != "--no-build" ]; then
    cargo build --release --locked
fi

name="rust-ide-$version-linux-x86_64"
stage="dist/$name"
rm -rf "$stage"
mkdir -p "$stage/docs"
install -m 755 "$target_dir/release/rust-ide" "$target_dir/release/rust-ide-tui" "$stage/"
install -m 755 packaging/linux/install.sh packaging/linux/install-deps.sh "$stage/"
install -m 644 packaging/linux/rust-ide.desktop README.md "$stage/"
install -m 644 docs/INSTALL.md "$stage/docs/"

tar -C dist -czf "dist/$name.tar.gz" "$name"
echo "Created dist/$name.tar.gz"
