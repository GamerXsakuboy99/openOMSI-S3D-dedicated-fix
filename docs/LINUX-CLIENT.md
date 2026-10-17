# Normal Sobol3D Edition Game — Linux x64

This is the normal, playable graphical openOMSI game. It is compiled natively
for Linux from the same maintained Rust game/launcher source targets as
Sobol's current Windows version. It is not a conversion of a Windows EXE,
a recycled old Linux binary, or a Dedicated Server build.

## Download from GitHub Actions

1. Open the [Build Linux Game Client workflow](https://github.com/GamerXsakuboy99/openOMSI-Sobol3D-Edition-dedicatedserver/actions/workflows/linux-client.yml).
2. Select a **successful** run and download its artifact named
   Sobols-openOMSI-linux-x64-tar-gz.
3. GitHub Actions wraps each artifact in a ZIP: unzip that to get
   Sobols-openOMSI-0.4.2-linux-x64.tar.gz.
4. Extract the .tar.gz itself to a writable game folder.

Like the old Sobol Linux archive shown in the screenshot, this tar.gz contains
exactly six top-level entries:

    Assets/
    libsteam_api.so
    openomsi
    openomsi.desktop
    openomsi.png
    openomsi-launcher

The server binary and the Admin Plus/Discord Bridge addons are not included.
It is a separate download from the Dedicated Server Releases.

## Run

An actual Linux graphical desktop and GPU graphics drivers are required.
The original OMSI 2 installation is also required and is NOT bundled.

From a terminal in the extracted game folder run:

    ./openomsi

The game's built-in launcher will appear. In Setup, choose the original
OMSI 2 folder that contains Omsi.exe, maps, Vehicles, etc. For explicit path:

    ./openomsi --root "/home/user/Games/OMSI 2" --menu

The separate openomsi-launcher helper should remain beside openomsi.
If executable permissions were lost during extraction, use:

    chmod +x openomsi openomsi-launcher

## Building locally

Install Rust from https://rustup.rs, then install native dependencies:

    sudo apt-get install build-essential pkg-config libasound2-dev libudev-dev libgtk-3-dev libxkbcommon-dev libwayland-dev libssl-dev
    sh scripts/build-linux-client.sh

This builds the normal game/launcher, NOT the dedicated-server binary.
The result is in the dist/ directory as a Sobols-openOMSI-*-linux-x64.tar.gz
archive.

## Upstream updates and validation

The fork uses Sobol's maintained game source, but GitHub forks do not
automatically sync new upstream commits. Sync upstream changes into this
fork when you want newer Windows-game functionality included in the Linux
client.

GitHub Actions checks and builds both Rust binaries, extracts the tar.gz
and tests that the Linux game can start with --help. CI does not prove
actual graphical gameplay, maps, mods or multiplayer work on every Linux PC.
