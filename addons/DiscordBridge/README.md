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
