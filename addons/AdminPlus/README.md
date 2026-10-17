# Admin Plus – server administration (Windows)

English-language WPF tool for openOMSI Sobol3D 0.4.2 Dedicated Server.
It is bundled in the Windows server distribution in its own folder.

1. Set `admin_password = <strong password>` in server.cfg.
2. Start the dedicated server and open Start-AdminPlus.cmd on the same Windows PC.
3. Enter its web_port (default 27025) and your admin password; click Refresh.

Tabs: Overview, Players, Traffic, Time, Weather, Custom Weather, Messages, Services.
Custom Weather uses the server's authenticated `weather custom` command.
Use `metar_sync = 0` to enable manual weather changes. The server broadcasts
changes promptly; **unmodified Sobol3D 0.4.2 clients take up to 240 seconds
to finish blending the new weather**.

The panel requires Windows PowerShell 5.1 and Desktop Experience, and connects
only to `127.0.0.1`. Never expose the /admin endpoint via a reverse proxy.
It does not save the admin password. For the optional player roster,
`share_positions = 1` exposes player positions through the public API;
keep it at 0 and enter player IDs manually if location privacy matters.
