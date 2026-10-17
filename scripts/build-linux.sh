#!/bin/sh
# Dedicated Server only. No playable client or GUI launcher is produced.
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo 'Install Rust from https://rustup.rs' >&2; exit 1; }
cargo build --locked --release --bin openomsi-server -p omsi-app
out=dist/linux-server
mkdir -p "$out"
cp target/release/openomsi-server "$out/"
cp scripts/server/start.sh "$out/start.sh"
cp docs/SERVER.md "$out/README.md"
cp LICENSE "$out/LICENSE"
mkdir -p "$out/Assets"
for folder in Infobox TM-Summertime; do
    if [ -d "assets/$folder" ]; then
        cp -R "assets/$folder" "$out/Assets/$folder"
    fi
done
if [ -f assets/steam_redist/libsteam_api.so ]; then cp assets/steam_redist/libsteam_api.so "$out/"; fi
chmod +x "$out/openomsi-server" "$out/start.sh"
printf 'Dedicated server built in %s\n' "$out"
