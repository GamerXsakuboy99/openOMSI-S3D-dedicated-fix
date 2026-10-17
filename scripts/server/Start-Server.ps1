# Dedicated Server startup wizard for Windows PowerShell 5.1.
# Launch via start.cmd. Never manually type --root or move OMSI 2 files.
[CmdletBinding()]
param(
    [string]$RootPath = '',
    [string]$ConfigPath = '',
    [switch]$ChooseNewFolder,
    [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'
$script:Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$script:SavedRoot = Join-Path $script:Here 'omsi-root.json'
$script:Requirements = @(
    'Omsi.exe',
    'maps\Grundorf\global.cfg',
    'maps\Berlin-Spandau\global.cfg',
    'Vehicles\MAN_SD200',
    'Vehicles\MAN_SD202',
    'Vehicles\MAN_NL_NG',
    'Sceneryobjects',
    'Splines',
    'Texture',
    'Fonts',
    'Humans',
    'Weather',
    'Inputs',
    'envir.cfg'
)

function Normalize-OMSIPath([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return '' }
    $v = [Environment]::ExpandEnvironmentVariables($Value.Trim().Trim('"'))
    try { return [IO.Path]::GetFullPath($v) }
    catch { return '' }
}

function Missing-OMSIContent([string]$Folder) {
    if (-not $Folder -or -not (Test-Path -LiteralPath $Folder -PathType Container)) {
        return @('OMSI 2 folder not found')
    }
    $missing = @()
    foreach ($needed in $script:Requirements) {
        if (-not (Test-Path -LiteralPath (Join-Path $Folder $needed))) {
            $missing += $needed
        }
    }
    return $missing
}

function Is-OMSI2Folder([string]$Folder) {
    return @(Missing-OMSIContent $Folder).Count -eq 0
}

function Read-SavedOMSIPath {
    if (-not (Test-Path -LiteralPath $script:SavedRoot -PathType Leaf)) { return '' }
    try {
        $data = Get-Content -LiteralPath $script:SavedRoot -Raw -Encoding UTF8 | ConvertFrom-Json
        return Normalize-OMSIPath ([string]$data.root)
    } catch {
        Write-Host 'The saved OMSI folder setting could not be read. Choose the folder again.' -ForegroundColor Yellow
        return ''
    }
}

function Save-OMSIPath([string]$Folder) {
    try {
        # Save JSON as UTF-8 so paths with spaces or international letters work.
        @{ root = $Folder } | ConvertTo-Json -Compress |
            Set-Content -LiteralPath $script:SavedRoot -Encoding UTF8
        Write-Host 'OMSI 2 folder saved for future starts.' -ForegroundColor Green
    } catch {
        Write-Host 'Could not save the folder. You can still run the server now.' -ForegroundColor Yellow
    }
}

function Find-OMSI2 {
    $candidates = New-Object 'System.Collections.Generic.List[string]'
    if ($env:OMSI_ROOT) { $candidates.Add($env:OMSI_ROOT) }
    foreach ($programs in @(
        [Environment]::GetEnvironmentVariable('ProgramFiles(x86)'),
        $env:ProgramFiles
    )) {
        if ($programs) {
            $candidates.Add((Join-Path $programs 'Steam\steamapps\common\OMSI 2'))
        }
    }
    foreach ($registryKey in @('HKCU:\Software\Valve\Steam','HKLM:\SOFTWARE\WOW6432Node\Valve\Steam')) {
        try {
            $steam = Get-ItemProperty -Path $registryKey -ErrorAction Stop
            foreach ($property in @('SteamPath','InstallPath')) {
                if ($steam.$property) {
                    $candidates.Add((Join-Path ([string]$steam.$property) 'steamapps\common\OMSI 2'))
                }
            }
        } catch { }
    }
    # Common secondary Steam libraries, including D: or E: game drives.
    foreach ($disk in @(Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue)) {
        foreach ($relative in @(
            'SteamLibrary\steamapps\common\OMSI 2',
            'Steam\steamapps\common\OMSI 2',
            'Games\SteamLibrary\steamapps\common\OMSI 2'
        )) {
            $candidates.Add((Join-Path $disk.Root $relative))
        }
    }
    foreach ($candidate in $candidates) {
        $folder = Normalize-OMSIPath $candidate
        if (Is-OMSI2Folder $folder) { return $folder }
    }
    return ''
}

function Pick-OMSI2Folder([string]$InitialPath) {
    try {
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
        try {
            $dialog.Description = 'Select the ORIGINAL OMSI 2 folder (containing Omsi.exe, maps and Vehicles).'
            $dialog.ShowNewFolderButton = $false
            if ($InitialPath -and (Test-Path -LiteralPath $InitialPath -PathType Container)) {
                $dialog.SelectedPath = $InitialPath
            }
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                return [string]$dialog.SelectedPath
            }
            return ''
        } finally { $dialog.Dispose() }
    } catch {
        Write-Host 'A folder selection window is unavailable on this Windows installation.' -ForegroundColor Yellow
        Write-Host 'Paste the full OMSI 2 installation folder path below instead.'
        return [string](Read-Host 'OMSI 2 folder path')
    }
}

function Run-SelfTest {
    $temp = Join-Path ([IO.Path]::GetTempPath()) ('openOMSI Server Folder Test ' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $temp -Force | Out-Null
    try {
        if (Is-OMSI2Folder $temp) { throw 'Empty folder incorrectly accepted.' }
        foreach ($entry in $script:Requirements) {
            $target = Join-Path $temp $entry
            if ($entry -match '\.(exe|cfg)$') {
                New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
                [IO.File]::WriteAllText($target, 'test')
            } else {
                New-Item -ItemType Directory -Path $target -Force | Out-Null
            }
        }
        if (-not (Is-OMSI2Folder $temp)) { throw 'Valid OMSI folder incorrectly rejected.' }
        if ((Normalize-OMSIPath ('"' + $temp + '"')) -ne $temp) { throw 'Quoted folder path parsing failed.' }
        Remove-Item -LiteralPath (Join-Path $temp 'Omsi.exe') -Force
        if (Is-OMSI2Folder $temp) { throw 'Missing Omsi.exe incorrectly accepted.' }
        Write-Host 'Start-Server folder checks PASSED (including paths with spaces).'
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($SelfTest) { Run-SelfTest; exit 0 }

Write-Host ''
Write-Host '==========================================' -ForegroundColor Cyan
Write-Host '  openOMSI Dedicated Server - Windows' -ForegroundColor Cyan
Write-Host '==========================================' -ForegroundColor Cyan
Write-Host ''
Write-Host 'You do not need to type --root or copy OMSI 2 files.' -ForegroundColor Yellow
Write-Host 'On first start, simply select your original OMSI 2 game folder.'
Write-Host ''

$exe = Join-Path $script:Here 'openomsi-server.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    Write-Host 'ERROR: openomsi-server.exe is missing.' -ForegroundColor Red
    Write-Host 'Extract the entire Windows Dedicated Server release ZIP, not only start.cmd.'
    exit 1
}

$selected = ''
if ($ChooseNewFolder) {
    Write-Host 'Changing OMSI 2 folder...'
} elseif ($RootPath) {
    $selected = Normalize-OMSIPath $RootPath
} else {
    $selected = Read-SavedOMSIPath
    if (-not (Is-OMSI2Folder $selected)) {
        $selected = Find-OMSI2
        if ($selected) { Write-Host "Found OMSI 2 automatically: $selected" -ForegroundColor Green }
    }
}

while (-not (Is-OMSI2Folder $selected)) {
    if ($selected) {
        Write-Host "This is NOT a complete original OMSI 2 folder: $selected" -ForegroundColor Red
        Write-Host 'Missing:'
        @(Missing-OMSIContent $selected) | ForEach-Object { Write-Host "  - $_" }
    }
    Write-Host ''
    Write-Host 'Choose the folder where Omsi.exe is located.' -ForegroundColor Yellow
    Write-Host 'Example: C:\Program Files (x86)\Steam\steamapps\common\OMSI 2'
    $selected = Normalize-OMSIPath (Pick-OMSI2Folder $selected)
    if (-not $selected) {
        Write-Host 'No OMSI 2 folder selected. Server not started.'
        exit 1
    }
}

Save-OMSIPath $selected

if ($ChooseNewFolder) {
    Write-Host ''
    Write-Host "OMSI 2 folder updated: $selected" -ForegroundColor Green
    Write-Host 'Now double-click start.cmd to launch the server.'
    exit 0
}

if ([string]::IsNullOrWhiteSpace($ConfigPath)) {
    $ConfigPath = Join-Path $script:Here 'server.cfg'
} elseif (-not [IO.Path]::IsPathRooted($ConfigPath)) {
    $ConfigPath = Join-Path $script:Here $ConfigPath
}

Write-Host ''
Write-Host 'Original OMSI 2:' -ForegroundColor Cyan
Write-Host "  $selected"
Write-Host 'Server configuration:' -ForegroundColor Cyan
Write-Host "  $ConfigPath"
if (-not (Test-Path -LiteralPath $ConfigPath -PathType Leaf)) {
    Write-Host 'First start: server.cfg will be created with default settings.' -ForegroundColor Yellow
}
Write-Host ''
Write-Host 'Starting dedicated server... Press Ctrl+C to stop it.' -ForegroundColor Green
Write-Host '------------------------------------------'
& $exe --root $selected --server $ConfigPath
$result = $LASTEXITCODE
if ($null -eq $result) { $result = 1 }
if ($result -eq 0) {
    Write-Host 'Dedicated Server has stopped normally.'
} else {
    Write-Host "Dedicated Server exited with error code $result." -ForegroundColor Red
    Write-Host 'Check the error above. The OMSI 2 path is shown at the start of this window.'
}
exit $result
