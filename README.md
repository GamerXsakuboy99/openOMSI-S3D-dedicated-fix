# openOMSI Sobol3D Edition — Dedicated Server

**Dedicated server releases • based on Sobol3D Edition 0.4.2 • Windows x64 & Linux x64**

The **Dedicated Server releases** contain the headless multiplayer server,
**not** the playable client or launcher. A separately built Linux game client
is available from its own workflow (details below). It can host existing OMSI 2 maps, timetables, AI
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

**Windows:** Double-click **`start.cmd`** in the extracted ZIP folder.
You do not need to write command-line arguments or manually use `--root`.

On the first start the script checks common Steam installations. If it cannot
find OMSI 2 automatically, a folder selection window opens: select the
**original OMSI 2 game folder containing `Omsi.exe`** (not the `maps` folder
and not the openOMSI server folder). It verifies that the original maps and
other required files exist, saves your choice to `omsi-root.json`, and uses
the same folder automatically next time. To change the selected folder later,
double-click **`Change-OMSI-Folder.cmd`**.

On Windows Server Core or without a desktop, the script asks you to **paste
the OMSI 2 folder path** instead. Advanced users may still run
`start.cmd "D:\SteamLibrary\steamapps\common\OMSI 2"` to choose a folder
directly.

`--root` is supplied to `openomsi-server.exe` **internally by the script**,
not typed as a Windows command.

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

## Linux playable game client (separate download)

This fork also builds **the normal Sobol3D graphical game for Linux x64**,
from the same current Rust game source that Sobol uses to build the Windows
client. It is not converted from a Windows EXE or repackaged from an old
Linux release.

**[Download/build Linux Game Client](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/actions/workflows/linux-client.yml)**

From a successful workflow run download the **Sobols-openOMSI-linux-x64-tar-gz**
artifact. GitHub wraps the download in ZIP; inside you will find
**Sobols-openOMSI-0.4.2-linux-x64.tar.gz**. This has the six top-level items
shown in Sobol's older Linux archive: Assets/, libsteam_api.so, openomsi,
openomsi.desktop, openomsi.png, and openomsi-launcher.

Extract the tar.gz and launch the game with **./openomsi** on Linux.
Set the original OMSI 2 installation folder in the graphical launcher.

See the [Linux client guide](docs/LINUX-CLIENT.md). Client artifacts are
**separate** from the server-only Releases downloads.

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

## Licence, permitted use and attribution

**This repository is NOT licensed entirely under MIT.** The applicable
licence depends on who owns each part of the code:

- **Original openOMSI code** by usonskyyyy retains the **MIT licence**.
  The original MIT-licensed portions remain reusable under MIT on their own,
  including in other projects when the MIT terms are met.
- **Sobol3D Edition modifications** by sobolx30 retain the upstream
  **All Rights Reserved** terms. Reusing or redistributing those protected
  changes requires compliance with Sobol's licence and any necessary
  rightsholder permission.
- **Original changes first made for this Dedicated Server fork** and
  copyright-owned by GamerXsakuboy99 are **All Rights Reserved** with
  permission limited to this Sobol3D Edition-based fork and the official
  Sobol3D Edition project. They are not offered for copying, porting or
  integrating into independent projects such as **NeoOMSI** or the
  **original openOMSI** without prior written permission from their
  copyright holder.

The fork-specific restrictions cover only **copyrightable implementation
work owned by the contributor** (for example, original Dedicated Server
tooling, Admin Plus, Discord Bridge and duty-tracking implementation), not
the freely available MIT parts, third-party materials or general ideas.
Naming other projects does not imply they have infringed anyone's rights.

**Important for redistributors:** The fork's permission for its own original
contributions does not override Sobol3D Edition's upstream restrictions.
Obtain any permission required by the upstream rightsholder before
redistributing modified packages containing their protected material.

Read the complete **[LICENSE](LICENSE)**, including all three parts and the
third-party notices, before reusing or distributing code from this repository.

