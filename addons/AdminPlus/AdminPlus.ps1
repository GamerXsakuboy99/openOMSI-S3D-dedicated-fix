param([switch]$ValidateXaml)
# Admin Plus standalone panel for openOMSI 0.4.1 dedicated server.
# Runs on the SAME Windows machine as the server, Windows PowerShell 5.1 + WPF.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase
[Net.ServicePointManager]::Expect100Continue = $false
[xml]$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        Title="openOMSI Admin Plus" Width="900" Height="750" MinWidth="700" MinHeight="550"
        WindowStartupLocation="CenterScreen" Background="#111B29" Foreground="#E5EDF9"
        FontFamily="Segoe UI" FontSize="13">
 <DockPanel Margin="16">
  <StackPanel DockPanel.Dock="Top">
   <TextBlock Text="openOMSI  /  ADMIN PLUS" FontSize="24" FontWeight="Bold" Foreground="#79C8FF"/>
   <TextBlock Text="Standalone server administration panel | 127.0.0.1" Margin="0,3,0,10"/>
   <WrapPanel>
    <TextBlock Text="Web port:" VerticalAlignment="Center"/>
    <TextBox Name="Port" Text="27025" Width="78" Padding="6" Margin="6"/>
    <TextBlock Text="Admin password:" VerticalAlignment="Center"/>
    <PasswordBox Name="Password" Width="200" Padding="6" Margin="6"/>
    <Button Name="Refresh" Content="Refresh" Padding="12,7" Margin="6"/>
   </WrapPanel>
   <TextBlock Name="Connection" Text="Not connected to the server yet." Margin="0,6,0,12"/>
  </StackPanel>
  <Border DockPanel.Dock="Bottom" Margin="0,12,0,0" Background="#203045" Padding="8">
   <StackPanel>
    <TextBlock Text="Activity log" FontWeight="Bold"/>
    <TextBox Name="Log" Height="92" IsReadOnly="True" AcceptsReturn="True" TextWrapping="Wrap"
       VerticalScrollBarVisibility="Auto" Background="#122033" Foreground="#DCEAFF"/>
   </StackPanel>
  </Border>
  <TabControl Name="Tabs" Background="#203045" BorderBrush="#334962" Foreground="#111827"/>
 </DockPanel>
</Window>
'@
$reader = New-Object System.Xml.XmlNodeReader($xaml)
$win = [System.Windows.Markup.XamlReader]::Load($reader)
function Get-Control([string]$name) { return $win.FindName($name) }
function Write-Log([string]$text) {
    (Get-Control 'Log').AppendText(('{0:HH:mm:ss}  {1}' -f (Get-Date),$text) + [Environment]::NewLine)
    (Get-Control 'Log').ScrollToEnd()
}
function Base-Url {
    $port = 0
    if (-not [int]::TryParse((Get-Control 'Port').Text.Trim(),[ref]$port) -or $port -lt 1 -or $port -gt 65535) {
        throw 'Web port must be between 1 and 65535.'
    }
    return "http://127.0.0.1:$port"
}
function One-Line([string]$value) {
    return ($value -replace '[\r\n\x00-\x1f]+',' ').Trim()
}
function Send-Command([string]$command) {
    try {
        $url = Base-Url
        $password = (Get-Control 'Password').Password
        if ([string]::IsNullOrWhiteSpace($password)) { throw 'Enter admin_password from server.cfg.' }
        $command = One-Line $command
        if (-not $command -or $command.Length -gt 200) { throw 'Command must be 1 to 200 characters long.' }
        $body = [Text.Encoding]::UTF8.GetBytes($command)
        $result = Invoke-WebRequest -Uri "$url/admin" -Method Post -Body $body -TimeoutSec 5 -UseBasicParsing -ContentType 'text/plain; charset=utf-8' -Headers @{'X-Admin-Password'=$password}
        Write-Log "Accepted (HTTP $($result.StatusCode)): $command"
    } catch {
        Write-Log ("Error: "+$_.Exception.Message)
        [void][System.Windows.MessageBox]::Show($_.Exception.Message,'Admin Plus','OK','Warning')
    }
}
function Read-Status {
    try {
        $url = Base-Url
        $s = Invoke-RestMethod -Uri "$url/status" -TimeoutSec 4
        (Get-Control 'Connection').Text = "Connected: $($s.name) ($($s.players)/$($s.max_players) players)"
        $script:Stats.Text = "Name: $($s.name)" + [Environment]::NewLine +
           "Map: $($s.map)" + [Environment]::NewLine +
           "Players: $($s.players)/$($s.max_players)" + [Environment]::NewLine +
           "Time: $($s.time)" + [Environment]::NewLine +
           "Weather: $($s.weather)" + [Environment]::NewLine + "Version: $($s.version)"
        if ($null -ne $s.world) {
            $w = $s.world
            $script:WorldStats.Text = "AI cars: $($w.cars) | Scheduled buses: $($w.buses)" +
              [Environment]::NewLine + "Passengers: walking $($w.walking), waiting $($w.waiting), aboard $($w.aboard)" +
              [Environment]::NewLine + "Traffic target: $($w.traffic)"
        } else { $script:WorldStats.Text = 'Simulation counters are unavailable.' }
        $script:PlayerList.Items.Clear()
        try {
            $people = Invoke-RestMethod -Uri "$url/players" -TimeoutSec 3
            foreach ($p in @($people)) {
                if ($null -ne $p -and $null -ne $p.id) {
                    $item = New-Object System.Windows.Controls.ListBoxItem
                    $item.Tag = [string]$p.id
                    $item.Content = "#$($p.id)  $($p.name)   |   $($p.line)   |   $($p.bus)"
                    [void]$script:PlayerList.Items.Add($item)
                }
            }
        } catch {
            # 404 is expected unless server.cfg sets share_positions = 1.
        }
    } catch {
        (Get-Control 'Connection').Text = 'Cannot connect to the server. Start the server and check web_port.'
        $script:Stats.Text = 'Unable to connect.'
        $script:WorldStats.Text = ''
        Write-Log ('Refresh failed: '+$_.Exception.Message)
    }
}
function New-Tab([string]$name) {
    $tab = New-Object System.Windows.Controls.TabItem
    $tab.Header = $name
    $scroll = New-Object System.Windows.Controls.ScrollViewer
    $scroll.VerticalScrollBarVisibility = 'Auto'
    $pane = New-Object System.Windows.Controls.StackPanel
    $pane.Margin = [System.Windows.Thickness]::new(16)
    $scroll.Content = $pane
    $tab.Content = $scroll
    [void](Get-Control 'Tabs').Items.Add($tab)
    return $pane
}
function Add-Text($pane,[string]$value,[int]$size=14) {
    $t = New-Object System.Windows.Controls.TextBlock
    $t.Text = $value
    $t.TextWrapping = 'Wrap'
    $t.FontSize = $size
    $t.Foreground = [System.Windows.Media.Brushes]::WhiteSmoke
    $t.Margin = [System.Windows.Thickness]::new(5,9,5,9)
    [void]$pane.Children.Add($t)
    return $t
}
function Add-Input($pane,[string]$label,[string]$default='') {
    [void](Add-Text $pane $label 13)
    $tb = New-Object System.Windows.Controls.TextBox
    $tb.Text = $default
    $tb.Margin = [System.Windows.Thickness]::new(5,2,5,8)
    $tb.Padding = [System.Windows.Thickness]::new(8)
    $tb.Background = [System.Windows.Media.Brushes]::White
    $tb.Foreground = [System.Windows.Media.Brushes]::Black
    [void]$pane.Children.Add($tb)
    return $tb
}
function Add-Choice($pane,[string]$label,[string[]]$values,[int]$selected=0) {
    [void](Add-Text $pane $label 13)
    $combo = New-Object System.Windows.Controls.ComboBox
    $combo.Margin = [System.Windows.Thickness]::new(5,2,5,8)
    $combo.Padding = [System.Windows.Thickness]::new(8,5,8,5)
    $combo.Width = 320
    $combo.HorizontalAlignment = 'Left'
    foreach ($value in $values) { [void]$combo.Items.Add($value) }
    $combo.SelectedIndex = $selected
    [void]$pane.Children.Add($combo)
    return $combo
}
function Add-Check($pane,[string]$label,[bool]$enabled=$false) {
    $check = New-Object System.Windows.Controls.CheckBox
    $check.Content = $label
    $check.IsChecked = $enabled
    $check.Margin = [System.Windows.Thickness]::new(8,6,8,6)
    $check.Foreground = [System.Windows.Media.Brushes]::WhiteSmoke
    [void]$pane.Children.Add($check)
    return $check
}
# Accept commas or dots in numeric inputs; multiplayer weather protocol requires dots.
function Weather-Number([string]$key,[double]$minimum,[double]$maximum) {
    $raw = $script:WeatherFields[$key].Text.Trim().Replace(',','.')
    $num = 0.0
    $culture = [Globalization.CultureInfo]::InvariantCulture
    $style = [Globalization.NumberStyles]::Float
    if (-not [double]::TryParse($raw,$style,$culture,[ref]$num) -or
        [double]::IsNaN($num) -or [double]::IsInfinity($num) -or
        $num -lt $minimum -or $num -gt $maximum) {
        throw "Weather value $key must be between $minimum and $maximum."
    }
    return $num
}
function Weather-Fmt([double]$number,[string]$fmt='0.##') {
    return $number.ToString($fmt,[Globalization.CultureInfo]::InvariantCulture)
}
function Custom-Weather-Command {
    $vis = Weather-Number 'vis' 50 50000
    $br = (Weather-Number 'br' 0 150) / 100
    $wd = Weather-Number 'wd' 0 360
    $ws = Weather-Number 'ws' 0 50
    $t = Weather-Number 't' -40 50
    $rh = Weather-Number 'rh' 0 100
    $p = Weather-Number 'p' 900 1100
    $cb = Weather-Number 'cb' 50 5000
    $pi = (Weather-Number 'pi' 0 100) * 255 / 100
    $wet = (Weather-Number 'wet' 0 100) / 100
    $cloud = $script:CloudChoice.SelectedIndex
    $precip = $script:PrecipChoice.SelectedIndex
    if ($cloud -lt 0 -or $precip -lt 0) { throw 'Select cloud cover and precipitation type.' }
    $snow = if ($script:SnowCover.IsChecked -eq $true) { 1 } else { 0 }
    $snowRoad = if ($script:SnowRoad.IsChecked -eq $true) { 1 } else { 0 }
    $parts = @(
        "vis=$(Weather-Fmt $vis '0')"
        "br=$(Weather-Fmt $br '0.00')"
        "wd=$(Weather-Fmt $wd '0')"
        "ws=$(Weather-Fmt $ws '0.0')"
        "t=$(Weather-Fmt $t '0.0')"
        "rh=$(Weather-Fmt $rh '0')"
        "p=$(Weather-Fmt $p '0')"
        "c=$cloud"
        "cb=$(Weather-Fmt $cb '0')"
        "pt=$precip"
        "pi=$(Weather-Fmt $pi '0')"
        "wet=$(Weather-Fmt $wet '0.00')"
        "snow=$snow"
        "snowroad=$snowRoad"
    )
    return 'weather custom custom:' + ($parts -join ';')
}

function Add-Button($pane,[string]$label,[string]$command) {
    $b = New-Object System.Windows.Controls.Button
    $b.Content = $label
    $b.Tag = $command
    $b.Margin = [System.Windows.Thickness]::new(5,3,5,3)
    $b.Padding = [System.Windows.Thickness]::new(13,8,13,8)
    $b.Background = [System.Windows.Media.BrushConverter]::new().ConvertFromString('#3475B9')
    $b.Foreground = [System.Windows.Media.Brushes]::White
    $b.HorizontalAlignment = 'Left'
    $b.MinWidth = 175
    $b.Add_Click({
        param($sender,$eventArgs)
        try {
            $cmd = [string]$sender.Tag
            if ($cmd.Contains('@ID@')) {
                $id = 0
                if (-not [int]::TryParse($script:IdInput.Text.Trim(),[ref]$id) -or $id -le 0) {
                    throw 'Enter a player ID or select a player from the list.'
                }
                $cmd = $cmd.Replace('@ID@',[string]$id)
            }
            switch ($cmd) {
                'say-input' { $cmd = 'say ' + (One-Line $script:MessageInput.Text) }
                'private-input' {
                    $id = 0
                    if (-not [int]::TryParse($script:IdInput.Text.Trim(),[ref]$id) -or $id -le 0) {
                        throw 'Enter a player ID or select a player from the list.'
                    }
                    $cmd = 'tell ' + $id + ' ' + (One-Line $script:PrivateInput.Text)
                }
                'weather-input' {
                    $file = One-Line $script:WeatherInput.Text
                    if ($file -notmatch '^(?i:Weather)/[^/\\]+\.owt$' -or $file.Contains('..')) {
                        throw 'Weather path must be Weather/filename.owt.'
                    }
                    $cmd = "weather set $file"
                }
                'weather-custom' {
                    $cmd = Custom-Weather-Command
                }
                'clock-input' {
                    $v = $script:ClockInput.Text.Trim()
                    if ($v -notmatch '^([01]\d|2[0-3]):([0-5]\d)$') { throw 'Time must use the HH:MM format.' }
                    $cmd = 'clock ' + (([int]$Matches[1]*3600)+([int]$Matches[2]*60))
                }
                'notice-input' {
                    $msg = One-Line $script:NoticeInput.Text
                    $n = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
                    $cmd = "notify all ap$n 8 info $msg"
                }
            }
            if ($cmd -match '^(kick|ban|duty-off)\s') {
                if ([System.Windows.MessageBox]::Show("Confirm this action: $cmd?",'Admin Plus','YesNo','Warning') -ne 'Yes') { return }
            }
            Send-Command $cmd
        } catch {
            [void][System.Windows.MessageBox]::Show($_.Exception.Message,'Admin Plus','OK','Warning')
        }
    })
    [void]$pane.Children.Add($b)
}
$overview = New-Tab 'Overview'
$script:Stats = Add-Text $overview 'Waiting for server information.' 16
$script:WorldStats = Add-Text $overview '' 15
[void](Add-Text $overview 'HTTP 202 means the command was accepted, not necessarily completed.' 12)
$players = New-Tab 'Players'
[void](Add-Text $players 'The player list requires share_positions = 1. You can also enter a player ID manually.' 12)
$script:PlayerList = New-Object System.Windows.Controls.ListBox
$script:PlayerList.Height = 115
$script:PlayerList.Margin = [System.Windows.Thickness]::new(5)
[void]$players.Children.Add($script:PlayerList)
$script:IdInput = Add-Input $players 'Player ID' ''
$script:PlayerList.Add_SelectionChanged({
    if ($null -ne $script:PlayerList.SelectedItem) {
        $script:IdInput.Text = [string]$script:PlayerList.SelectedItem.Tag
    }
})
Add-Button $players 'Kick player (confirm)' 'kick @ID@'
Add-Button $players 'Ban for this session (confirm)' 'ban @ID@'
Add-Button $players 'Repair and refuel bus' 'service repair @ID@'
Add-Button $players 'Recover overturned bus' 'unstick @ID@'
Add-Button $players 'End player duty (confirm)' 'duty-off @ID@'
$script:PrivateInput = Add-Input $players 'Private message' ''
Add-Button $players 'Send private message' 'private-input'
$traffic = New-Tab 'Traffic'
[void](Add-Text $traffic 'Adjust random AI traffic. Scheduled buses are not affected.' 14)
foreach ($density in @(0,10,20,30,50,75,100)) {
    Add-Button $traffic "AI traffic: $density" "traffic $density"
}
Add-Button $traffic 'Clear AI traffic congestion' 'traffic clear'
$clock = New-Tab 'Time'
[void](Add-Text $clock 'Real-time synchronization prevents manual clock changes.' 13)
foreach ($pair in @(@('06:00','21600'),@('08:00','28800'),@('12:00','43200'),@('18:00','64800'),@('23:00','82800'))) {
    Add-Button $clock $pair[0] ('clock '+$pair[1])
}
$script:ClockInput = Add-Input $clock 'Custom time, HH:MM' '09:00'
Add-Button $clock 'Set time' 'clock-input'
Add-Button $clock '+1 hour' 'time 3600'
Add-Button $clock '-1 hour' 'time -3600'
foreach ($speed in @(1,2,4,8,15,30)) { Add-Button $clock "Time speed x$speed" "speed $speed" }
$weather = New-Tab 'Weather'
Add-Button $weather 'Next weather preset' 'weather next'
$script:WeatherInput = Add-Input $weather 'OWT file in your OMSI 2 Weather folder' 'Weather/#CAVOK.owt'
Add-Button $weather 'Apply weather preset' 'weather-input'
$custom = New-Tab 'Custom Weather'
[void](Add-Text $custom 'Apply new custom weather to the running server. Stock 0.4.2 clients blend changes gradually.' 15)
[void](Add-Text $custom 'Requires an updated openOMSI server supporting weather custom. Disable METAR synchronization.' 12)
$script:WeatherFields = @{}
$script:WeatherFields['t'] = Add-Input $custom 'Temperature (°C), -40 to +50' '15'
$script:WeatherFields['rh'] = Add-Input $custom 'Relative humidity (%), 0 to 100' '51'
$script:WeatherFields['p'] = Add-Input $custom 'Air pressure (hPa), 900 to 1100' '1013'
$script:WeatherFields['vis'] = Add-Input $custom 'Visibility (m), 50 to 50000' '50000'
$script:WeatherFields['br'] = Add-Input $custom 'Brightness (%), 0 to 150' '100'
$script:WeatherFields['wd'] = Add-Input $custom 'Wind direction (°), 0 to 360' '0'
$script:WeatherFields['ws'] = Add-Input $custom 'Wind speed (m/s), 0 to 50' '0'
$script:CloudChoice = Add-Choice $custom 'Cloud cover' @('Clear skies','Cumulus 1','Cumulus 2','Cumulus 3','Overcast') 0
$script:WeatherFields['cb'] = Add-Input $custom 'Cloud base (m), 50 to 5000' '500'
$script:PrecipChoice = Add-Choice $custom 'Precipitation type' @('None','Rain','Snow') 0
$script:WeatherFields['pi'] = Add-Input $custom 'Precipitation intensity (%), 0 to 100' '0'
$script:WeatherFields['wet'] = Add-Input $custom 'Road wetness (%), 0 to 100' '0'
$script:SnowCover = Add-Check $custom 'Snow on the ground' $false
$script:SnowRoad = Add-Check $custom 'Snow on roads' $false
Add-Button $custom 'Apply custom weather to server' 'weather-custom'
[void](Add-Text $custom 'Click Apply after each edit. Server broadcasts immediately; stock 0.4.2 clients blend to the new weather over about 240 seconds. No extra OWT files needed.' 12)

$messages = New-Tab 'Messages'
$script:MessageInput = Add-Input $messages 'Broadcast chat message' 'Welcome to the server!'
Add-Button $messages 'Send chat message' 'say-input'
$script:NoticeInput = Add-Input $messages 'On-screen notice for all players (8 seconds)' 'Please follow the server rules.'
Add-Button $messages 'Show notification' 'notice-input'
$service = New-Tab 'Services'
Add-Button $service 'Repair all buses' 'service repair all'
Add-Button $service 'Refuel all buses' 'service refuel all'
Add-Button $service 'Wash all buses' 'service wash all'

(Get-Control 'Refresh').Add_Click({ Read-Status })
if ($ValidateXaml) {
    $test = Custom-Weather-Command
    if (-not $test.StartsWith('weather custom custom:vis=50000;') -or
        -not $test.Contains(';pt=0;pi=0;wet=0.00;snow=0;snowroad=0') -or
        $test.Length -gt 200) {
        throw "Admin Plus custom weather serialization failed: $test"
    }
    Write-Host 'Admin Plus WPF UI and custom weather serialization OK.'
    exit 0
}
$timer = New-Object System.Windows.Threading.DispatcherTimer
$timer.Interval = [TimeSpan]::FromSeconds(15)
$timer.Add_Tick({ Read-Status })
$timer.Start()
Write-Log 'Admin Plus is running. Password is not saved.'
Read-Status
[void]$win.ShowDialog()
$timer.Stop()
