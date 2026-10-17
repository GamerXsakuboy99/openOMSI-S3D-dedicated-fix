# openOMSI Sobol3D Edition – Dedicated Server Fork

**This fork builds a dedicated multiplayer server, not a playable game client.**

Based on **Sobol3D Edition 0.4.2**. The headless server simulates OMSI 2 maps,
vehicles, timetables, traffic and passengers. You still need an original OMSI 2
installation (including any map/vehicle mods that players use).

## Download

Go to **Actions** and run either **Build Dedicated Server Windows** or
**Build Dedicated Server Linux**. Download the matching workflow artifact,
extract it and follow its README.

There is one executable: `openomsi-server.exe` (Windows) or
`openomsi-server` (Linux). **No playable game or launcher is packaged.**
The source tree retains the simulation and client library modules that are
required to compile the shared dedicated-host implementation.

### Run on Windows

```bat
start.cmd "C:\Program Files (x86)\Steam\steamapps\common\OMSI 2"
```

### Run on Linux

```sh
./start.sh "/srv/omsi2"
```

On first start the server writes `server.cfg` next to the script.
Edit it, then restart. See [server documentation](docs/SERVER.md).

## Server administration and Discord monitoring

Enable the optional private driving activity journal with \`duty_tracking = 1\` in
\`server.cfg\` (disabled by default). It records duty starts, switches, the duty
being cleared and disconnections; it does **not** falsely report successful trip
completions that a stock 0.4.2 client never confirms to the host.

The server supports a loopback-only, password-protected `POST /admin`
endpoint and the public `/status` endpoint. Optional management tools
can run separately on the same server machine. Do not expose `/admin`
through Cloudflare or a reverse proxy.

This fork is independent of the original **openOMSI Sobol3D Edition** and is
not an official upstream server distribution. Original project:
https://github.com/sobolx30/openOMSI-Sobol3D-Edition

License: [MIT](LICENSE).
