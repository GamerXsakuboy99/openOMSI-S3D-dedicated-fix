#!/bin/sh
# Start the graphical openOMSI Linux client from its extracted folder.
set -eu
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
if [ ! -x "$here/openomsi" ]; then
  printf 'openomsi is missing. Extract the complete Linux CLIENT archive.\n' >&2
  exit 1
fi
# Keep the executable and Steam library together, including for GUI launches.
export LD_LIBRARY_PATH="$here${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
cd "$here"
exec "$here/openomsi" "$@"
