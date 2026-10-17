# openOMSI Sobol3D Edition — Dedicated Server

**Dedicated server only • based on Sobol3D Edition 0.4.2 • Windows x64 & Linux x64**

This fork builds the headless openOMSI multiplayer server, **not** the playable
game client or launcher. It can host existing OMSI 2 maps, timetables, AI
traffic and passengers. An **original OMSI 2 installation** and any content
used by the server are still required.

## Download the Dedicated Server

### [Download the latest Dedicated Server release](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/releases)

**Use the [Releases page](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/releases) for downloadable
server ZIPs**, not the temporary build artifacts in GitHub Actions. Releases
are created automatically only after the Windows **and** Linux build workflows
both pass on the *same source commit*, and after the package contents are
checked.

| Platform | File on Releases | Includes |
| --- | --- | --- |
| Windows x64 | `openOMSI-DedicatedServer-0.4.2-windows-x64.zip` | `openomsi-server.exe`, `start.cmd`, required bundled assets/runtime DLLs, **Admin Plus**, **Discord Bridge** |
| Linux x64 | `openOMSI-DedicatedServer-0.4.2-linux-x64.zip` | `openomsi-server`, `start.sh`, bundled assets/runtime files, **Discord Bridge (Python 3)** |

Every published release also contains `SHA256SUMS.txt`, which lists checksums
for both ZIP files. Each release links back to the exact Git commit used to
build it. Older versions remain available on the Releases page.

**If no release is listed yet**, a release is still awaiting successful
verification or publication. You can inspect the [build status](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/actions).
Do not assume an unverified or failed Actions run provides a working release.

## Installation

Extract the downloaded ZIP into a **new folder**. Do not run the executable
directly from inside the ZIP.

**Windows**, in the extracted server folder:

```bat
start.cmd "C:\Program Files (x86)\Steam\steamapps\common\OMSI 2"
```

**Linux**, in the extracted server folder:

```sh
./start.sh "/srv/omsi2"
```

Replace the OMSI 2 path with your actual installation directory. The first
start creates a commented `server.cfg` when it doesn't exist. Edit the
configuration and restart the server. Preserve your own `server.cfg` and
Discord `config.json` when updating from an older release.

See **[Dedicated Server setup and configuration](docs/SERVER.md)** for all
available settings, ports, METAR, weather, and administration.

## Administration and Discord

- **Admin Plus (Windows):** an English-language, standalone WPF GUI in the
  `AdminPlus/` folder. It controls the local server using its existing
  password-protected `/admin` endpoint. Custom weather changes are sent live;
  unmodified 0.4.2 game clients may still fade between weather states.
- **Discord Bridge:** a separate webhook notifier in `DiscordBridge/` for
  Windows (PowerShell) or Linux (Python 3). Server online/offline, player count,
  weather, and optionally selected driving-duty changes.
- **Duty Tracking:** opt in with `duty_tracking = 1` in `server.cfg`. It
  writes a private `duty-events.jsonl` beside the config, without exposing
  player locations. The 0.4.2 clients do **not** provide server-verifiable
  trip-completion events, so the bridge does not invent those notifications.

Keep `share_positions = 0` unless you intentionally want to expose player
positions through the HTTP API. Never forward `/admin` to the internet, and
never publish your Discord webhook URL.

## Build and release checks

[Windows build](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/actions/workflows/windows.yml) ·
[Linux build](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/actions/workflows/linux.yml) ·
[Management tools tests](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/actions/workflows/management-tools.yml)

The automated release pipeline checks that both Windows and Linux artifacts
were built successfully from the same commit; ensures the archives include
their startup scripts and expected components, and do not contain the game
client; and produces checksum-verified release downloads.

**Build success is not a full multiplayer gameplay certification.** Test with
your OMSI 2 installation, a map and players before public deployment.

This is an independent fork, **not an official Sobol3D Edition release**.
Original project: [sobolx30/openOMSI-Sobol3D-Edition](https://github.com/sobolx30/openOMSI-Sobol3D-Edition).

License: [MIT](LICENSE).
