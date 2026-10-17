# Discord Bridge – dedicated server event notifier (Windows)

This Windows PowerShell 5.1 process is bundled with the Windows Dedicated
Server but runs **separately**. It reads local server status and duty journals
and posts configurable Discord webhook embeds. No admin password is needed.

## Install

1. Run Start-DiscordBridge.cmd once to create config.json from config.example.json.
2. Create a Discord channel webhook (channel settings > Integrations > Webhooks)
   and put its secret HTTPS URL in config.json as webhook_url.
3. Set server_web_port to server.cfg's web_port (default 27025).
4. Optionally enable duty_tracking = 1 in server.cfg, restart the server,
   and set duty_events_log_path to the absolute location of duty-events.jsonl
   beside server.cfg. The journal is created on the first duty event.
5. Run Start-DiscordBridge.cmd to watch the server. Test-Webhook.cmd sends
   one harmless test to Discord.

## What does `duty_events_log_path` do?

`duty_events_log_path` is **the path to a FILE, not a directory or a
Discord URL**. Discord Bridge reads this private file to detect the start,
change and end of a player's selected driving duty, as well as a disconnect
while on duty. It sends the enabled `notify_duty_*` alerts to your Discord
webhook. Other webhook notifications (server online/offline, player counts,
weather) do **not** depend on this setting.

**To enable duty alerts:**

1. Open the dedicated server's **`server.cfg`**, set `duty_tracking = 1`,
   save it and **restart the server**. The default is `0` (disabled).
2. When a player first starts a duty, the server creates
   **`duty-events.jsonl` in the SAME folder as `server.cfg`**. The file
   is created by the game server, **not** by Discord Bridge.
3. Open **`DiscordBridge/config.json`** and set `duty_events_log_path`
   to that file. With the recommended directory layout below, a relative
   path works on Windows **and** Linux:

   ```json
   "duty_events_log_path": "../duty-events.jsonl"
   ```

4. Restart Discord Bridge. It reads **new events only**, so it will not post
   the entire history from an existing log.

Default extracted release layout:

```text
DedicatedServer/
├── openomsi-server.exe   (or openomsi-server on Linux)
├── server.cfg
├── duty-events.jsonl     <-- the server creates this when a duty event occurs
└── DiscordBridge/
    └── config.json       <-- put ../duty-events.jsonl here
```

If `server.cfg` is elsewhere, use the **actual** full file path instead,
for example `"C:\\OMSI-Server\\duty-events.jsonl"` on Windows (JSON needs
**double backslashes**) or `"/srv/openomsi/duty-events.jsonl"` on Linux.
The path is resolved relative to the DiscordBridge folder when it is not
absolute.

**Leave `duty_events_log_path` empty (`""`) if you don't want duty
messages.** Status, weather and player-count messages still work.

**Privacy and accuracy:** The journal contains player names, IDs, selected
lines/tours, vehicle names and timestamps, **not coordinates**. Keep it private
and don't upload it to GitHub. `share_positions = 1` is **not required** for
this journal. Stock 0.4.2 clients do not provide confirmed trip-completion
events: `duty_ended` means the selected duty ended, **not** that every trip
was successfully driven.

## Alert types

Server online/offline, player count, weather changes, optional named player
joins/leaves, periodic status reports, duty started, duty ended, duty switched,
driver disconnected while on duty, optional line changes and bus changes.
Set individual notify_* flags in config.json; restart the bridge after edits.

Duty events work with share_positions = 0. The local journal contains names,
IDs, lines, tours and bus names **but no coordinates**. It is private, ignored
by Git, and is not served by an HTTP endpoint.

**Important:** The unmodified 0.4.2 client tells the server the selected tour,
not verified trip completion. A cleared duty is reported as "completion unknown",
not "Trip Completed". Kicks and bans also cannot be reliably distinguished from
ordinary disconnections by this external watcher.

The bridge runs on the same Windows machine as the server. A whole-machine
power/network outage cannot be reported by a tool running on that same machine.
It checks status every 10 seconds by default and does not retry failed webhook
messages indefinitely. Run DiscordBridge.ps1 -SelfTest for offline checks.

Keep config.json secret: its webhook URL grants posting access to the channel.
The script restricts webhooks to Discord HTTPS hosts and disables @mentions.

## Linux (Python 3 standard library)

The Linux server artifact also includes `DiscordBridge.py` and
`Start-DiscordBridge.sh`. This is a separate, optional Linux process,
requiring `python3` but **no pip-installed modules**. Use the same
`config.json` described above, kept only on the server machine.

```sh
./Start-DiscordBridge.sh --self-test
./Start-DiscordBridge.sh --check-config
./Start-DiscordBridge.sh
```

The tool polls `http://127.0.0.1:<web_port>/status` and tails the same
private JSONL duty journal as the Windows PowerShell version. No player
coordinates are posted to Discord.
