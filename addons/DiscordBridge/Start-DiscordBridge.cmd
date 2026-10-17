@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0DiscordBridge.ps1"
if errorlevel 1 (
  echo Discord Bridge stopped with an error. Check config.json and the message above.
  pause
)
