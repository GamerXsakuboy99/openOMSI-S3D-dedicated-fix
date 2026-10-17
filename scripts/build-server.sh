#!/bin/sh
# Build the server only; forward the output folder argument if supplied.
set -eu
repo="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo"
sh scripts/build-linux.sh
if [ "$#" -gt 0 ]; then
  mkdir -p "$1"
  cp -R dist/linux-server/. "$1/"
fi
