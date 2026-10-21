#!/bin/sh
# Builds the release binaries and packs dist/rust-ide-<version>-macos-<arch>.tar.gz
# containing "Rust IDE.app" (the window version) and the terminal version.
# Usage (from the repository root):
#   sh packaging/package-macos.sh [rust-target] [--no-build]
# rust-target: aarch64-apple-darwin (default on Apple Silicon) or x86_64-apple-darwin.
set -e
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

target=""
build=1
for arg in "$@"; do
    case "$arg" in
        --no-build) build=0 ;;
        *) target="$arg" ;;
    esac
done
if [ -z "$target" ]; then
    case "$(uname -m)" in
        arm64) target=aarch64-apple-darwin ;;
        *) target=x86_64-apple-darwin ;;
    esac
fi
case "$target" in
    aarch64-*) arch=arm64 ;;
    *) arch=x86_64 ;;
esac

if [ "$build" = 1 ]; then
    rustup target add "$target" >/dev/null 2>&1 || true
    cargo build --release --locked --target "$target"
fi
bin="${CARGO_TARGET_DIR:-target}/$target/release"

name="rust-ide-$version-macos-$arch"
stage="dist/$name"
rm -rf "$stage"
mkdir -p "$stage/docs"

app="$stage/Rust IDE.app/Contents"
mkdir -p "$app/MacOS"
install -m 755 "$bin/rust-ide" "$app/MacOS/rust-ide"
cat > "$app/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Rust IDE</string>
    <key>CFBundleDisplayName</key><string>Rust IDE</string>
    <key>CFBundleIdentifier</key><string>io.github.v3ct0r-8890.rust-ide</string>
    <key>CFBundleExecutable</key><string>rust-ide</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

install -m 755 "$bin/rust-ide-tui" "$stage/"
install -m 755 packaging/macos/install-deps.sh "$stage/"
install -m 644 README.md "$stage/"
install -m 644 docs/INSTALL.md "$stage/docs/"

# Ad-hoc signature: Apple Silicon refuses to run unsigned code. (Not notarized:
# the first start needs right-click > Open, see docs/INSTALL.md.)
if command -v codesign >/dev/null 2>&1; then
    codesign --force --sign - "$stage/rust-ide-tui"
    codesign --force --deep --sign - "$stage/Rust IDE.app"
fi

tar -C dist -czf "dist/$name.tar.gz" "$name"
echo "Created dist/$name.tar.gz"
