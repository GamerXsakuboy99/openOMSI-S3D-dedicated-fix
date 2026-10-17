@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0AdminPlus.ps1"
if errorlevel 1 (
 echo.
 echo Admin Plus encountered an error. See the message above.
 pause
)
