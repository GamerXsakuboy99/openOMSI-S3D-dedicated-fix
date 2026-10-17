#!/bin/sh
# Run: ./start.sh /path/to/OMSI2 [server.cfg]
set -eu
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
root="${1:?Usage: start.sh /path/to/OMSI2 [server.cfg]}"
cfg="${2:-$here/server.cfg}"
exec "$here/openomsi-server" --root "$root" --server "$cfg"
