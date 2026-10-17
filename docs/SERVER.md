# openOMSI Sobol3D Edition 0.4.2 — Dedicated Server

This fork is a **server-only distribution**. Its release artifacts never
contain the playable game or launcher. It still compiles much of the original
game library because timetable, traffic and passenger simulations share code.

**An original OMSI 2 installation is required.** The server uses its maps,
Vehicles, Sceneryobjects, Splines, and installed content. Install on a machine
you administer; player mods must exist on the server too.

## Run

Windows:

```bat
start.cmd "C:\Program Files (x86)\Steam\steamapps\common\OMSI 2"
```

Linux:

```sh
./start.sh "/srv/omsi2"
```

You can also launch the binary directly with:
```
openomsi-server --root "<OMSI2-folder>" --server "server.cfg"
```

If the configuration file is missing, the server generates its full commented
`server.cfg` template on first start. It can be edited in a text editor;
restart the server to apply configuration changes. The binary **requires**
`--server` and refuses to start a game or graphical launcher.

Build workflows: **Build Dedicated Server Windows** and
**Build Dedicated Server Linux**. They publish one downloadable artifact
containing the server executable and its start script. The Windows package
also includes Admin Plus and Discord Bridge under separate folders.

## Config examples

```ini
name = openOMSI Dedicated Server
motd = Welcome!
map = maps/Berlin-Spandau/global.cfg
date =
time = 08:00
weather =
traffic = 30
timetable = 1
passengers = 1
port = 27015
web_port = 27025
max_players = 16
tunnel = 0
radius = 0
admin_password =
time_speed = 1
real_time = 0
metar_sync = 0
metar_station =
vehicles =
share_positions = 0
duty_tracking = 0
voice_server_uid =
voice_channel =
voice_channel_password =
voice_range = 20
```

This is a sample, not a mandatory override of your existing configuration.
Leave `vehicles` empty to allow installed buses. The zero/one switches
also accept true/false, yes/no and on/off.

**Network:** `port` is the UDP session port. `web_port` is the TCP gateway,
status API and WebSocket port. `tunnel = 1` starts the optional Cloudflare
quick tunnel when `cloudflared` is installed. The generated default may be
different; set your preferred values deliberately.

**Time:** `real_time = 1` locks the clock to the server machine's local time
and disables manual clock-speed administration. `time_speed` is 1 to 30.

**Weather:** `metar_sync = 1` follows reports at `metar_station`, or the
nearest airport when empty. Disable it (`metar_sync = 0`) to change weather
manually using Admin Plus. The dedicated server validates the custom weather
wire and broadcasts it when an admin applies settings. Unmodified 0.4.2
client games gradually blend weather over about 240 seconds; this cannot
be overridden in a server-only package.

**Privacy:** `share_positions = 1` makes names and player positions available
to anyone who can reach the server's `/players` HTTP path. Leave it at 0 if
you do not want public positions. `duty_tracking = 1` separately enables
a **local, private** `duty-events.jsonl` file next to server.cfg; it has
player names, IDs, duty line, tour, vehicle names and event timestamps, not
coordinates. The option is OFF by default. Protect the file like other
private server logs.

## Server administration (Windows)

`AdminPlus/Start-AdminPlus.cmd` runs a separate Windows PowerShell WPF
control panel on the **same Windows machine** as the dedicated server.
The panel talks to `http://127.0.0.1:<web_port>/admin` and requires
`admin_password` in server.cfg. The password is not stored by the panel.

- Overview: server details, traffic and passenger counters.
- Players: kick, session ban, bus recovery, private messages, services.
- Traffic, Time and Messages: live admin commands.
- Weather and Custom Weather: OWT presets or manually set weather values.

**Never expose the `/admin` path through a tunnel or reverse proxy**,
including one forwarding from localhost. A valid admin password permits
players to be kicked and other server changes.

## Discord Bridge (Windows)

`DiscordBridge/Start-DiscordBridge.cmd` runs a separate status and
activity watcher beside the server. It does not require the admin password.

1. Run it once to create `DiscordBridge/config.json` from the included example.
2. Set `webhook_url` to your own channel's secret Discord webhook URL.
3. Set `server_web_port` to match your server.cfg.
4. To receive duty events, set `duty_tracking = 1` in server.cfg and restart,
   then enter the absolute location of `duty-events.jsonl` in
   `duty_events_log_path`.
5. Start the bridge again and use Test-Webhook.cmd if you need to test delivery.

Available messages include server online/offline, player count, weather,
duty started, duty ended, changed duties and duty disconnects. A stock 0.4.2
client publishes its selected `line/tour` but does **not** send confirmed
trip completion or intermediate stop milestones to the server. Thus no
"Trip Completed" event is claimed. It cannot reliably distinguish a
kick from a normal disconnect either.

The webhook URL is a secret. Do not commit `config.json`. The repository
ignores it, and the Windows ZIP only includes `config.example.json`.
If the whole machine loses Internet/power, a bridge on that machine cannot
send a "Server Offline" message.

## Linux

The Linux artifact is server-only. Admin Plus and Discord Bridge shipped
here are Windows-specific PowerShell utilities; they are not background
services for Linux. The server's local `/status` and JSONL activity log can
be integrated with an external notifier that you choose to run on the
server machine.

## Testing and limitations

Build workflows run `cargo check` and build the server binary. The Windows
workflow checks the management scripts with Windows PowerShell 5.1 and runs
offline smoke tests. A successful build is not the same as a complete
multiplayer gameplay test; always test a map, clients and weather before
public deployment.

Original openOMSI project:
https://github.com/sobolx30/openOMSI-Sobol3D-Edition
