# Sobol3D Edition — Dedicated Server passenger spawning fix

**Temporary, minimal fork of [Sobol3D Edition 0.4.2](https://github.com/sobolx30/openOMSI-Sobol3D-Edition).**

This branch (`openOMSI-S3D-dedicated-fix`) exists **only to preserve a
passenger spawning fix for the Dedicated Server** while the corresponding
problem remains unresolved in Sobol3D Edition's official version.
It is not intended to compete with or replace Sobol3D Edition.

## The only functional change

**[`crates/omsi-app/src/humans.rs`](crates/omsi-app/src/humans.rs)**

When a Dedicated Server knows stop/line targets but has no nearby
departure-time window (`soon`), the original destination filter rejects
every destination. Waiting passengers can therefore fail to spawn.

This fix limits destinations to imminent departures **only when a
departure window is available**. Without one, the normal eligible
stop destinations are considered instead. Existing filtering remains
in place when a real departure window is present.

Original fix: [`bf3f6d1`](https://github.com/GamerXsakuboy99/openOMSI-S3D-dedicatedserver/commit/bf3f6d178df9314418d831daa8baf409a03c216c).

**Everything else is taken from upstream 0.4.2.** No extra admin API,
custom weather commands, duty-event logging, Discord Bridge, Admin Plus,
separate dedicated-only binary or independent game-client builder is
part of this branch.

## Building and using the server

The project uses the **original Sobol3D Edition build and startup
scripts**; no new GitHub Actions or release-publishing workflows are
provided here.

On Linux, install Rust and the native dependencies described in the
upstream [build instructions](docs/BUILDING.md), then run:

```sh
sh scripts/build-server.sh
./dist/server/start.sh "/path/to/OMSI 2"
```

On Windows, the upstream build uses `scripts/build-windows.cmd` and
produces `openomsi.exe` in `dist/windows/`. The same executable can
host a headless server when started with:

```text
openomsi.exe --root "C:\\path\\to\\OMSI 2" --server "server.cfg"
```

A **complete licensed OMSI 2 installation** and its maps/vehicles are
required. The server's configuration and gameplay are documented in
[upstream server documentation](docs/SERVER.md).

The change in this branch is source-level only. **It does not prove
that passenger spawning works on every map or that graphical clients
have been tested with this patch.** Verify with a real dedicated server,
original OMSI 2 content and players before production use.

## Other branches and add-ons

- [**Addons**](https://github.com/GamerXsakuboy99/openOMSI-S3D-dedicatedserver/tree/Addons):
  independent Admin Plus and Discord Bridge tools, maintained separately.
- [**Redux-Dedicated**](https://github.com/GamerXsakuboy99/openOMSI-S3D-dedicatedserver/tree/Redux-Dedicated):
  the previous expanded dedicated-server build and its extra functionality.
  It is kept intact for reference and rollback.

The old extended builds or Releases associated with this repository
may contain features **not present in this minimal branch**. Do not
mistake those older binaries for a build of only the passenger fix.

Once the official Dedicated Server fix has been verified upstream, this
temporary branch may no longer be needed.

## Licence

The original openOMSI MIT licence and the Sobol3D Edition changes'
existing protections continue to apply. Original copyrightable work
on the passenger fix is separately identified in [LICENSE](LICENSE).
