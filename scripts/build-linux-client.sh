#!/bin/sh
# Build the CURRENT normal Sobol3D game from Windows-client SOURCE for Linux.
# Make the same six-root-entry tar.gz layout as Sobol's original Linux package.
set -eu
repo="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo 'Install Rust from https://rustup.rs' >&2; exit 1; }

# Same application targets Sobol builds for Windows, compiled natively for Linux.
# Specify binaries explicitly, because the fork also has a dedicated-server bin.
cargo build --locked --release -p omsi-app --bin openomsi
cargo build --locked --release -p omsi-launcher-core --bin openomsi-launcher

out="dist/linux-client"
rm -rf "$out"
mkdir -p "$out/Assets"
cp target/release/openomsi "$out/openomsi"
cp target/release/openomsi-launcher "$out/openomsi-launcher"
cp scripts/linux/openomsi.desktop "$out/openomsi.desktop"
cp assets/icons/app/openomsi-256.png "$out/openomsi.png"
test -s assets/steam_redist/libsteam_api.so
cp assets/steam_redist/libsteam_api.so "$out/libsteam_api.so"
for folder in Infobox TM-Summertime; do
  test -d "assets/$folder" || { echo "Missing Assets/$folder" >&2; exit 1; }
  cp -R "assets/$folder" "$out/Assets/$folder"
done
chmod +x "$out/openomsi" "$out/openomsi-launcher"
test ! -e "$out/openomsi-server"

version="$(tr -d '[:space:]' < VERSION)"
case "$version" in *[!0-9.]*|'') echo "Invalid VERSION: $version" >&2; exit 1 ;; esac
archive="dist/Sobols-openOMSI-$version-linux-x64.tar.gz"
tar -czf "$archive" -C "$out" \
  Assets libsteam_api.so openomsi openomsi.desktop openomsi.png openomsi-launcher
echo "Linux graphical client archive: $archive"
