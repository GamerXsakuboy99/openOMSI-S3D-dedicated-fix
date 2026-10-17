#!/bin/sh
# Linux dedicated-server Discord Bridge; requires Python 3, no pip packages.
set -eu
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
exec python3 "$here/DiscordBridge.py" "$@"
