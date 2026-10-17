param([switch]$SelfTest, [switch]$CheckConfig, [switch]$SendTest)

# Standalone openOMSI Discord Bridge. Windows PowerShell 5.1; no game changes.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$script:ConfigFile = Join-Path $PSScriptRoot 'config.json'
$script:ExampleFile = Join-Path $PSScriptRoot 'config.example.json'
$script:Config = $null

function Log([string]$message) {
    Write-Host ('[{0:yyyy-MM-dd HH:mm:ss}] {1}' -f (Get-Date), $message)
}
function Clean([string]$value, [int]$limit = 150) {
    if ($null -eq $value) { return '' }
    $value = ($value -replace '[\x00-\x1f]', ' ').Trim()
    if ($value.Length -gt $limit) { return $value.Substring(0,$limit-3) + '...' }
    return $value
}
function Valid-Webhook([string]$value) {
    $uri = $null
    if (-not [Uri]::TryCreate($value,[UriKind]::Absolute,[ref]$uri)) { return $false }
    if ($uri.Scheme -ne 'https' -or $uri.Host -notin @('discord.com','discordapp.com') -or
        $uri.Port -ne 443 -or $uri.UserInfo -or $uri.Query -or $uri.Fragment) { return $false }
    return $uri.AbsolutePath -match '^/api/webhooks/[0-9]+/[A-Za-z0-9._-]+$'
}
function Read-Config {
    if (-not (Test-Path -LiteralPath $script:ConfigFile -PathType Leaf)) {
        Copy-Item -LiteralPath $script:ExampleFile -Destination $script:ConfigFile
        throw 'Created config.json. Enter your webhook URL, then restart the bridge.'
    }
    $c = Get-Content -LiteralPath $script:ConfigFile -Encoding UTF8 -Raw | ConvertFrom-Json
    if (-not (Valid-Webhook ([string]$c.webhook_url))) {
        throw 'Set webhook_url to a valid Discord HTTPS webhook URL in config.json.'
    }
    $port = 0
    if (-not [int]::TryParse([string]$c.server_web_port,[ref]$port) -or $port -lt 1 -or $port -gt 65535) {
        throw 'server_web_port must be between 1 and 65535.'
    }
    $interval = [int]$c.poll_interval_seconds
    $failures = [int]$c.offline_after_failures
    $summary = [int]$c.summary_interval_minutes
    if ($interval -lt 5 -or $interval -gt 300) { throw 'poll_interval_seconds must be 5-300.' }
    if ($failures -lt 2 -or $failures -gt 20) { throw 'offline_after_failures must be 2-20.' }
    if ($summary -lt 0 -or $summary -gt 1440) { throw 'summary_interval_minutes must be 0-1440.' }
    foreach ($key in @('notify_online_offline','notify_initial_online','notify_player_join',
        'notify_player_leave','notify_player_count','notify_weather_change',
        'notify_duty_started','notify_duty_ended','notify_duty_switched',
        'notify_duty_disconnected','notify_line_changed','notify_bus_changed')) {
        if ($null -eq $c.$key -or $c.$key -isnot [bool]) { throw "$key must be true or false." }
    }
    $script:Config = $c
}
function Send-Discord([string]$title,[string]$description,[int]$color) {
    $embed = @{
        title = (Clean $title 200)
        description = (Clean $description 1800)
        color = $color
        timestamp = (Get-Date).ToUniversalTime().ToString('o')
        footer = @{ text = 'openOMSI Discord Bridge' }
    }
    $payload = @{
        username = (Clean ([string]$script:Config.discord_username) 70)
        allowed_mentions = @{ parse = @() }
        embeds = @($embed)
    }
    $json = $payload | ConvertTo-Json -Depth 8 -Compress
    try {
        $null = Invoke-WebRequest -Uri ([string]$script:Config.webhook_url) -Method Post -Body ([Text.Encoding]::UTF8.GetBytes($json)) -ContentType 'application/json; charset=utf-8' -TimeoutSec 10 -UseBasicParsing
        Log "Discord notification sent: $title"
        return $true
    } catch {
        # Do NOT display exception text: it can include the secret webhook URL.
        $status = 'network error'
        if ($null -ne $_.Exception.Response) {
            try { $status = [string][int]$_.Exception.Response.StatusCode } catch {}
        }
        Log "Discord delivery failed ($status). Secret URL not logged."
        return $false
    }
}
function Get-Status {
    $base = 'http://127.0.0.1:' + [string]$script:Config.server_web_port
    $s = Invoke-RestMethod -Uri "$base/status" -TimeoutSec 5
    if ($null -eq $s -or $null -eq $s.name -or $null -eq $s.players) {
        throw 'Incomplete server status response.'
    }
    return $s
}
function Get-Players {
    $base = 'http://127.0.0.1:' + [string]$script:Config.server_web_port
    try {
        $all = Invoke-RestMethod -Uri "$base/players" -TimeoutSec 5
        return ,@($all | Where-Object { $null -ne $_ -and $null -ne $_.id })
    } catch {
        # 404 is normal when share_positions=0. Never interpret failure as all players leaving.
        return $null
    }
}
function Compare-Players($oldList,$newList) {
    $before = @{}
    $after = @{}
    foreach ($p in @($oldList)) {
        if ($null -ne $p -and $null -ne $p.id) { $before[[string]$p.id] = $p }
    }
    foreach ($p in @($newList)) {
        if ($null -ne $p -and $null -ne $p.id) { $after[[string]$p.id] = $p }
    }
    $joined = @($after.Keys | Where-Object { -not $before.ContainsKey($_) } | Sort-Object | ForEach-Object { $after[$_] })
    $left = @($before.Keys | Where-Object { -not $after.ContainsKey($_) } | Sort-Object | ForEach-Object { $before[$_] })
    return [pscustomobject]@{ Joined = $joined; Left = $left }
}
function Player-Names($players) {
    $names = @($players | Select-Object -First 20 | ForEach-Object {
        ('#{0} {1}' -f [string]$_.id,(Clean ([string]$_.name) 65))
    })
    $remaining = @($players).Count - $names.Count
    if ($remaining -gt 0) { $names += "+$remaining more" }
    return $names -join [Environment]::NewLine
}
function Status-Summary($s) {
    $lines = @(
        "Players: $($s.players)/$($s.max_players)"
        ('Map: ' + (Clean ([string]$s.map) 130))
        ('Weather: ' + (Clean ([string]$s.weather) 130))
        "Time: $($s.time)"
    )
    if ($null -ne $s.world) {
        $lines += "AI cars: $($s.world.cars); timetable buses: $($s.world.buses)"
        $lines += "Passengers: $($s.world.waiting) waiting; $($s.world.aboard) aboard"
    }
    return $lines -join [Environment]::NewLine
}

# Server writes one JSON event per line beside server.cfg; no HTTP player
# positions, admin password or other additional network port is involved.
function Read-DutyEvents {
    if (-not $script:DutyLogFile -or -not (Test-Path -LiteralPath $script:DutyLogFile -PathType Leaf)) { return @() }
    $stream = [System.IO.FileStream]::new(
        $script:DutyLogFile, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try {
        if ($stream.Length -lt $script:DutyCursor) {
            # The file was truncated or rotated: start from the beginning of the new file.
            $script:DutyCursor = 0L
            $script:DutyPartial = ''
        }
        if ($stream.Length -eq $script:DutyCursor) { return @() }
        if ($stream.Length - $script:DutyCursor -gt 262144) {
            # Prevent unbounded buffering after a long pause; keep the newest 256 KB.
            $script:DutyCursor = $stream.Length - 262144
            $script:DutyPartial = ''
            Log 'Duty event backlog exceeded 256 KB; older entries were skipped.'
        }
        $null = $stream.Seek($script:DutyCursor,[IO.SeekOrigin]::Begin)
        $reader = [System.IO.StreamReader]::new($stream,[Text.Encoding]::UTF8,$false,4096,$true)
        try {
            $text = $reader.ReadToEnd()
            $script:DutyCursor = $stream.Position
        } finally { $reader.Dispose() }
        $parts = ($script:DutyPartial + $text).Split([char]10)
        $script:DutyPartial = $parts[$parts.Length - 1]
        $complete = @()
        for ($i = 0; $i -lt $parts.Length - 1; $i++) {
            $line = $parts[$i].Trim()
            if ($line) {
                try {
                    $record = $line | ConvertFrom-Json -ErrorAction Stop
                    if ($null -ne $record.kind -and $null -ne $record.player_id) {
                        $complete += $record
                    }
                } catch { Log 'An invalid duty event line was skipped.' }
            }
        }
        return $complete
    } finally { $stream.Dispose() }
}

function Send-DutyEvent($record) {
    $titles = @{
        duty_started = 'Duty started'
        duty_ended = 'Duty ended (completion unknown)'
        duty_switched = 'Duty switched'
        duty_disconnected = 'Driver disconnected during duty'
        line_changed = 'Line changed'
        bus_changed = 'Bus changed'
    }
    $flags = @{
        duty_started = 'notify_duty_started'
        duty_ended = 'notify_duty_ended'
        duty_switched = 'notify_duty_switched'
        duty_disconnected = 'notify_duty_disconnected'
        line_changed = 'notify_line_changed'
        bus_changed = 'notify_bus_changed'
    }
    $kind = [string]$record.kind
    if (-not $titles.ContainsKey($kind) -or -not $script:Config.($flags[$kind])) { return }
    $color = if ($kind -in @('duty_ended','duty_switched','duty_disconnected')) { 15105570 } else { 3066993 }
    $name = Clean ([string]$record.player) 90
    $line = Clean ([string]$record.line) 70
    $tour = Clean ([string]$record.tour) 70
    $route = Clean ([string]$record.route) 70
    $bus = Clean ([string]$record.bus) 120
    $details = @(
        "Driver: $name (#$($record.player_id))"
        "Duty line: $line"
        "Tour: $tour"
        "Route display: $route"
        "Bus: $bus"
    ) -join [Environment]::NewLine
    $null = Send-Discord $titles[$kind] $details $color
}

function Run-SelfTest {
    $one = [pscustomobject]@{ id = 1; name = 'One' }
    $two = [pscustomobject]@{ id = 2; name = 'Two' }
    $three = [pscustomobject]@{ id = 3; name = 'Three' }
    $diff = Compare-Players @($one,$two) @($two,$three)
    if (@($diff.Joined).Count -ne 1 -or @($diff.Left).Count -ne 1 -or
        [string]$diff.Joined[0].id -ne '3' -or [string]$diff.Left[0].id -ne '1') {
        throw 'Player join/leave comparison failed.'
    }
    if (-not (Valid-Webhook 'https://discord.com/api/webhooks/123/fake_token') -or
        (Valid-Webhook 'http://discord.com/api/webhooks/123/secret') -or
        (Valid-Webhook 'https://example.com/api/webhooks/123/secret')) {
        throw 'Webhook URL validation failed.'
    }
    $testFile = [IO.Path]::GetTempFileName()
    try {
        [IO.File]::WriteAllText($testFile, '{"kind":"duty_started","player_id":12,"player":"Test","line":"37","tour":"01","trip":"Route","trip_number":1,"trip_total":2}' + [Environment]::NewLine)
        $script:DutyLogFile = $testFile
        $script:DutyCursor = 0L
        $script:DutyPartial = ''
        $events = @(Read-DutyEvents)
        if ($events.Count -ne 1 -or [string]$events[0].kind -ne 'duty_started') {
            throw 'Duty event log reader failed.'
        }
        if (@(Read-DutyEvents).Count -ne 0) { throw 'Duty event cursor replayed the same line.' }
    } finally { Remove-Item -LiteralPath $testFile -ErrorAction SilentlyContinue }
    Log 'Self-test passed: player tracking, duty journal and webhook validation (no network).'
}
if ($SelfTest) { Run-SelfTest; exit 0 }

try { Read-Config } catch {
    Log $_.Exception.Message
    exit 1
}
if ($CheckConfig) {
    Log 'Configuration valid. No network requests were made.'
    exit 0
}
if ($SendTest) {
    if (Send-Discord 'Discord Bridge test' 'Your openOMSI webhook is working.' 3447003) {
        exit 0
    }
    exit 1
}

$online = $false
$everOnline = $false
$failCount = 0
$prevStatus = $null
$prevPlayers = $null
$lastSummary = Get-Date
$wait = [int]$script:Config.poll_interval_seconds
$failureLimit = [int]$script:Config.offline_after_failures

# Never replay all earlier trip records when Discord Bridge starts.
$script:DutyLogFile = [string]$script:Config.duty_events_log_path
$script:DutyCursor = 0L
$script:DutyPartial = ''
if ($script:DutyLogFile) {
    $script:DutyLogFile = [Environment]::ExpandEnvironmentVariables($script:DutyLogFile)
    if (-not [IO.Path]::IsPathRooted($script:DutyLogFile)) {
        $script:DutyLogFile = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot $script:DutyLogFile))
    }
    if (Test-Path -LiteralPath $script:DutyLogFile -PathType Leaf) {
        $script:DutyCursor = (Get-Item -LiteralPath $script:DutyLogFile).Length
    }
    if (Test-Path -LiteralPath $script:DutyLogFile -PathType Leaf) {
        Log "Duty event log found: $($script:DutyLogFile)"
    } else {
        Log 'Duty event log not found yet. Set duty_tracking = 1 in server.cfg and restart the server.'
        Log 'The server creates duty-events.jsonl beside server.cfg after the first duty event.'
        Log "Waiting for the file: $($script:DutyLogFile)"
    }
} else {
    Log 'Duty alerts off: duty_events_log_path is empty. Server status and weather alerts still work.'
}

Log "Discord Bridge started. Reading local server status every $wait seconds."
Log 'Players are named only when share_positions=1. No coordinates are sent to Discord.'
Log 'Press Ctrl+C to stop.'

while ($true) {
    try {
        $status = Get-Status
        $wasOnline = $online
        $online = $true
        $failCount = 0
        $name = Clean ([string]$status.name) 100
        if (-not $wasOnline) {
            $initial = -not $everOnline
            $everOnline = $true
            if ($script:Config.notify_online_offline -and
                (-not $initial -or $script:Config.notify_initial_online)) {
                $null = Send-Discord 'Server online' "$name is online." 3066993
            }
            Log "Connected: $name"
        } elseif ($script:Config.notify_weather_change -and
                  [string]$status.weather -ne [string]$prevStatus.weather) {
            $weather = Clean ([string]$status.weather) 130
            $null = Send-Discord 'Weather changed' "$name weather: $weather" 3447003
        }

        $track = $script:Config.notify_player_join -or $script:Config.notify_player_leave
        $players = $null
        if ($track) { $players = Get-Players }
        $hasRoster = $null -ne $players
        $hadRoster = $null -ne $prevPlayers
        if ($wasOnline -and $hasRoster -and $hadRoster) {
            $diff = Compare-Players $prevPlayers $players
            if ($script:Config.notify_player_join -and $diff.Joined.Count -gt 0) {
                $text = (Player-Names $diff.Joined) + [Environment]::NewLine + "Online: $($status.players)/$($status.max_players)"
                $null = Send-Discord 'Players joined' $text 5763719
            }
            if ($script:Config.notify_player_leave -and $diff.Left.Count -gt 0) {
                $text = (Player-Names $diff.Left) + [Environment]::NewLine + "Online: $($status.players)/$($status.max_players)"
                $null = Send-Discord 'Players left' $text 15548997
            }
        } elseif ($wasOnline -and $script:Config.notify_player_count -and
                  [int]$status.players -ne [int]$prevStatus.players) {
            $null = Send-Discord 'Player count changed' "Online: $($status.players)/$($status.max_players)" 10181046
        }

        # Roster failure resets its baseline, so a later recovery does not create fake joins.
        if ($hasRoster) { $prevPlayers = @($players) } else { $prevPlayers = $null }
        $prevStatus = $status
        if ([int]$script:Config.summary_interval_minutes -gt 0 -and
            ((Get-Date) - $lastSummary).TotalMinutes -ge [int]$script:Config.summary_interval_minutes) {
            $null = Send-Discord 'Server status' (Status-Summary $status) 10181046
            $lastSummary = Get-Date
        }
    } catch {
        $failCount++
        if ($online -and $failCount -ge $failureLimit) {
            $online = $false
            $prevStatus = $null
            $prevPlayers = $null
            Log 'Server is offline (consecutive status checks failed).'
            if ($script:Config.notify_online_offline) {
                $null = Send-Discord 'Server offline' 'The openOMSI server stopped responding.' 15158332
            }
        } elseif (-not $online -and $failCount -eq 1) {
            Log 'Waiting for the local server...'
        }
    }
    if ($script:DutyLogFile) {
        try {
            foreach ($entry in @(Read-DutyEvents)) { Send-DutyEvent $entry }
        } catch { Log ('Duty journal read failed: ' + $_.Exception.Message) }
    }
    Start-Sleep -Seconds $wait
}
