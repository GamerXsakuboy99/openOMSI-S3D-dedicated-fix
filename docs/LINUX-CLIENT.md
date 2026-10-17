# openOMSI Sobol3D Edition — Linux Game Client (x86-64)

This is the **playable graphical client**, built from this Sobol3D Edition
fork. It is **not** the Dedicated Server, even though both share source
libraries. It includes `openomsi` (the actual game) and
`openomsi-launcher` (launcher helper).

## Download

1. Open [Build Linux Game Client](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/actions/workflows/linux-client.yml).
2. Select the latest **successful** workflow run, or choose **Run workflow**.
3. Under **Artifacts**, download `openOMSI-Sobol3D-0.4.2-linux-client-x64`.
4. Unzip the complete archive to one folder; do not run the binary inside ZIP.

This is a **separate Actions artifact** and is intentionally NOT attached to
Dedicated Server Releases. Only successful builds provide artifacts.

## Start the game

You need a **graphical Linux desktop** with working GPU drivers. The source
game uses Vulkan/OpenGL graphics via wgpu. OMSI 2's original proprietary game
content is **not bundled**: provide a complete original OMSI 2 installation
or an appropriately copied folder containing `Omsi.exe`, `maps`,
`Vehicles`, and the remaining required assets.

Inside the extracted Linux client folder:

```sh
./start-client.sh
```

The game normally opens its launcher; select the original OMSI 2 content
folder in the launcher Setup screen. If you know the OMSI 2 path already:

```sh
./start-client.sh --root "/home/user/Games/OMSI 2" --menu
```

To use the separate launcher helper, run `./openomsi-launcher`. Both files
must remain beside each other. The folder also includes the Steam runtime
library, an application icon and extra `Assets/` models.

## Build locally

On Ubuntu 22.04, install Rust via <https://rustup.rs> and:

```sh
sudo apt-get install build-essential pkg-config libasound2-dev libudev-dev \
    libgtk-3-dev libxkbcommon-dev libwayland-dev libssl-dev
sh scripts/build-linux-client.sh
```

Look for the finished client in `dist/linux-client/`. This script builds
ONLY the client and its launcher, never the server executable.

### Scope of automated tests

CI checks Cargo, builds the release executables, validates the package and
starts the Linux client with `--help` in headless mode. This is **not a full
graphical gameplay test**; a Linux desktop and your own OMSI 2 content are
needed to test maps, controls and multiplayer.

Upstream: <https://github.com/sobolx30/openOMSI-Sobol3D-Edition>.
