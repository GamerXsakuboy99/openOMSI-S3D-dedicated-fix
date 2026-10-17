#!/bin/sh
# Build the REAL graphical Linux game and its launcher; never package the host.
# Run from repository root: sh scripts/build-linux-client.sh
set -eu
repo="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || {
  echo "Rust is missing. Install it via https://rustup.rs." >&2
  exit 1
}
# Explicit bin targets keep the dedicated server out of the client release.
cargo build --locked --release -p omsi-app --bin openomsi
cargo build --locked --release -p omsi-launcher-core --bin openomsi-launcher

out="dist/linux-client"
mkdir -p "$out/Assets"
cp target/release/openomsi "$out/openomsi"
cp target/release/openomsi-launcher "$out/openomsi-launcher"
cp scripts/linux/start-client.sh "$out/start-client.sh"
cp scripts/linux/openomsi.desktop "$out/openomsi.desktop"
cp assets/icons/app/openomsi-256.png "$out/openomsi.png"
cp LICENSE "$out/LICENSE"
cp docs/LINUX-CLIENT.md "$out/README.md"
# Sobol3D's two additional shipped Asset models.
for folder in Infobox TM-Summertime; do
  if [ -d "assets/$folder" ]; then
    cp -R "assets/$folder" "$out/Assets/$folder"
  fi
done
# Bundled Steam runtime must travel with the game binary.
test -s assets/steam_redist/libsteam_api.so
cp assets/steam_redist/libsteam_api.so "$out/libsteam_api.so"
chmod +x "$out/openomsi" "$out/openomsi-launcher" "$out/start-client.sh"
test ! -e "$out/openomsi-server"
printf 'Linux client built separately in %s\n' "$out"
