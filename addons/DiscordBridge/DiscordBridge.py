#!/usr/bin/env python3
"""Standalone openOMSI Discord Bridge for Linux (Python 3 standard library only).
Reads localhost HTTP status and an optional private JSONL journal.
It never exposes the game admin endpoint or player coordinates to Discord.
"""
import argparse
import json
import os
import re
import shutil
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
TITLES = {
    "duty_started": "Duty started",
    "duty_ended": "Duty ended (completion unknown)",
    "duty_switched": "Duty switched",
    "duty_disconnected": "Driver disconnected during duty",
    "line_changed": "Line changed",
    "bus_changed": "Bus changed",
}
FLAGS = {key: "notify_" + key for key in TITLES}
DEFAULT_NAME = "openOMSI Server"


def clean(value, limit=130):
    return " ".join(str(value or "").split())[:limit]


def valid_webhook(value):
    try:
        url = urllib.parse.urlsplit(value)
        return (
            url.scheme == "https"
            and url.hostname in ("discord.com", "discordapp.com")
            and url.port in (None, 443)
            and not url.username
            and not url.password
            and not url.query
            and not url.fragment
            and re.fullmatch(r"/api/webhooks/[0-9]+/[A-Za-z0-9._-]+", url.path) is not None
        )
    except (ValueError, TypeError):
        return False


def read_config():
    path = HERE / "config.json"
    if not path.is_file():
        shutil.copyfile(HERE / "config.example.json", path)
        raise ValueError("Created config.json; enter your Discord webhook URL and restart.")
    cfg = json.loads(path.read_text(encoding="utf-8-sig"))
    if not valid_webhook(cfg.get("webhook_url", "")):
        raise ValueError("Invalid Discord webhook_url in config.json.")
    port = cfg.get("server_web_port")
    if type(port) is not int or not 1 <= port <= 65535:
        raise ValueError("server_web_port must be 1-65535.")
    for key, minimum, maximum in (
        ("poll_interval_seconds", 5, 300),
        ("offline_after_failures", 2, 20),
        ("summary_interval_minutes", 0, 1440),
    ):
        value = cfg.get(key)
        if type(value) is not int or not minimum <= value <= maximum:
            raise ValueError(f"{key} must be between {minimum} and {maximum}.")
    flags = ["notify_online_offline", "notify_initial_online", "notify_player_join",
             "notify_player_leave", "notify_player_count", "notify_weather_change", *FLAGS.values()]
    for flag in flags:
        if type(cfg.get(flag)) is not bool:
            raise ValueError(f"{flag} must be true or false.")
    return cfg


def log(message):
    print(time.strftime("[%Y-%m-%d %H:%M:%S]"), message, flush=True)


def get_json(port, endpoint):
    url = f"http://127.0.0.1:{port}/{endpoint}"
    with urllib.request.urlopen(url, timeout=5) as response:
        return json.loads(response.read(65536).decode("utf-8"))


def roster_diff(previous, current):
    old = {str(p["id"]): p for p in previous if isinstance(p, dict) and "id" in p}
    new = {str(p["id"]): p for p in current if isinstance(p, dict) and "id" in p}
    return ([new[k] for k in sorted(new.keys() - old.keys())],
            [old[k] for k in sorted(old.keys() - new.keys())])


class Journal:
    """Tail only new JSONL records. Never mistake an unavailable file for an empty roster."""

    def __init__(self, filename):
        self.path = Path(os.path.expandvars(filename)).expanduser() if filename else None
        if self.path and not self.path.is_absolute():
            self.path = HERE / self.path
        self.offset = self.path.stat().st_size if self.path and self.path.is_file() else 0
        self.partial = b""

    def poll(self):
        if not self.path or not self.path.is_file():
            return []
        size = self.path.stat().st_size
        if size < self.offset:
            self.offset = 0
            self.partial = b""
        if size == self.offset:
            return []
        if size - self.offset > 262144:
            self.offset = size - 262144
            self.partial = b""
            log("Duty event backlog exceeded 256 KB; older events skipped.")
        with self.path.open("rb") as file:
            file.seek(self.offset)
            data = file.read(262144)
            self.offset = file.tell()
        complete = (self.partial + data).split(b"\n")
        self.partial = complete.pop()
        results = []
        for raw in complete:
            if not raw.strip() or len(raw) > 8192:
                continue
            try:
                item = json.loads(raw)
                if isinstance(item, dict) and "kind" in item and "player_id" in item:
                    results.append(item)
            except (UnicodeError, ValueError):
                log("Invalid duty journal record ignored.")
        return results


class Bridge:
    def __init__(self, cfg):
        self.c = cfg
        self.journal = Journal(cfg.get("duty_events_log_path", ""))
        self.connected = False
        self.seen = False
        self.failures = 0
        self.prev_status = None
        self.prev_roster = None
        self.next_summary = time.monotonic() + cfg["summary_interval_minutes"] * 60

    def notify(self, title, description, color=3447003):
        payload = {
            "username": clean(self.c.get("discord_username") or DEFAULT_NAME, 70),
            "allowed_mentions": {"parse": []},
            "embeds": [{
                "title": clean(title, 200),
                "description": clean(description, 1800),
                "color": color,
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "footer": {"text": "openOMSI Discord Bridge"},
            }],
        }
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        request = urllib.request.Request(
            self.c["webhook_url"], data=data,
            headers={"Content-Type": "application/json; charset=utf-8"}, method="POST"
        )
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                response.read(512)
            log("Discord notification sent: " + title)
        except (urllib.error.URLError, TimeoutError, OSError) as error:
            # Deliberately omit exception message; it may contain the secret URL.
            status = getattr(error, "code", "network error")
            log(f"Discord delivery failed ({status}); webhook secret is not logged.")

    def process_duty(self, record):
        kind = record.get("kind")
        if kind not in TITLES or not self.c.get(FLAGS[kind], False):
            return
        fields = [
            "Driver: " + clean(record.get("player")) + " (#" + clean(record.get("player_id")) + ")",
            "Duty line: " + clean(record.get("line")),
            "Tour: " + clean(record.get("tour")),
            "Route display: " + clean(record.get("route")),
            "Bus: " + clean(record.get("bus")),
        ]
        color = 15105570 if kind in ("duty_ended", "duty_switched", "duty_disconnected") else 3066993
        self.notify(TITLES[kind], "\n".join(fields), color)

    def cycle(self):
        try:
            status = get_json(self.c["server_web_port"], "status")
            if not isinstance(status, dict) or "name" not in status or "players" not in status:
                raise ValueError("Incomplete status")
            was = self.connected
            self.connected = True
            self.failures = 0
            name = clean(status["name"])
            if not was:
                initial = not self.seen
                self.seen = True
                if self.c["notify_online_offline"] and (not initial or self.c["notify_initial_online"]):
                    self.notify("Server online", name + " is reachable.", 3066993)
                log("Connected to " + name)
            elif self.c["notify_weather_change"] and status.get("weather") != self.prev_status.get("weather"):
                self.notify("Weather changed", name + ": " + clean(status.get("weather")))

            before = self.prev_roster
            roster = None
            if self.c["notify_player_join"] or self.c["notify_player_leave"]:
                try:
                    result = get_json(self.c["server_web_port"], "players")
                    roster = result if isinstance(result, list) else None
                except (urllib.error.URLError, ValueError, TimeoutError, OSError):
                    pass
            if was and before is not None and roster is not None:
                joined, left = roster_diff(before, roster)
                if joined and self.c["notify_player_join"]:
                    details = "\n".join("#" + str(p["id"]) + " " + clean(p.get("name")) for p in joined[:20])
                    self.notify("Players joined", details + "\nOnline: " + str(status["players"]), 5763719)
                if left and self.c["notify_player_leave"]:
                    details = "\n".join("#" + str(p["id"]) + " " + clean(p.get("name")) for p in left[:20])
                    self.notify("Players left", details + "\nOnline: " + str(status["players"]), 15105570)
            elif was and self.c["notify_player_count"] and status["players"] != self.prev_status.get("players"):
                self.notify("Player count changed", f"Online: {status['players']}/{status.get('max_players', '?')}")
            self.prev_roster = roster
            self.prev_status = status

            if self.c["summary_interval_minutes"] and time.monotonic() >= self.next_summary:
                details = (f"Players: {status['players']}/{status.get('max_players', '?')}"
                           f"\nMap: {clean(status.get('map'))}"
                           f"\nWeather: {clean(status.get('weather'))}"
                           f"\nTime: {clean(status.get('time'))}")
                self.notify("Server status", details)
                self.next_summary = time.monotonic() + self.c["summary_interval_minutes"] * 60
        except (urllib.error.URLError, TimeoutError, ValueError, OSError):
            self.failures += 1
            if self.connected and self.failures >= self.c["offline_after_failures"]:
                self.connected = False
                self.prev_status = None
                self.prev_roster = None
                log("Server stopped responding.")
                if self.c["notify_online_offline"]:
                    self.notify("Server offline", "The openOMSI server stopped responding.", 15158332)
        for record in self.journal.poll():
            self.process_duty(record)

    def run(self):
        log("Discord Bridge started (Linux, Python 3). Ctrl+C stops it.")
        if self.journal.path:
            log("Private duty tracking journal configured.")
        while True:
            self.cycle()
            time.sleep(self.c["poll_interval_seconds"])


def self_test():
    assert valid_webhook("https://discord.com/api/webhooks/123/fake_token")
    assert not valid_webhook("http://discord.com/api/webhooks/123/token")
    assert not valid_webhook("https://evil.example/api/webhooks/123/token")
    a, b = roster_diff([{"id": 1, "name": "A"}], [{"id": 2, "name": "B"}])
    assert [x["id"] for x in a] == [2] and [x["id"] for x in b] == [1]
    with tempfile.TemporaryDirectory() as folder:
        path = Path(folder) / "journal.jsonl"
        reader = Journal(str(path))
        with path.open("ab") as handle:
            handle.write(b'{"kind":"duty_started","player_id":1}\n')
        events = reader.poll()
        assert len(events) == 1 and events[0]["kind"] == "duty_started"
        assert reader.poll() == []
    log("Self-test passed (no network calls).")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check-config", action="store_true")
    parser.add_argument("--test-webhook", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    try:
        cfg = read_config()
    except (ValueError, OSError, json.JSONDecodeError) as error:
        log(str(error))
        sys.exit(1)
    if args.check_config:
        log("Config valid. No network calls.")
        return
    bridge = Bridge(cfg)
    if args.test_webhook:
        bridge.notify("Discord Bridge test", "Your webhook is working.", 3447003)
        return
    try:
        bridge.run()
    except KeyboardInterrupt:
        log("Discord Bridge stopped.")


if __name__ == "__main__":
    main()
