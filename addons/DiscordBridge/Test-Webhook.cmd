@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0DiscordBridge.ps1" -SendTest
if errorlevel 1 echo Discord webhook test failed. Verify config.json.
pause
