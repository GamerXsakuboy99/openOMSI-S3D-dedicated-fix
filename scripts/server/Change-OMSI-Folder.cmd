@echo off
setlocal
title Change OMSI 2 installation
set "HERE=%~dp0"
cd /d "%HERE%"
if not exist "%HERE%Start-Server.ps1" (
  echo Start-Server.ps1 is missing. Extract the COMPLETE server ZIP.
  pause
  exit /b 1
)
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -STA -File "%HERE%Start-Server.ps1" -ChooseNewFolder
if errorlevel 1 pause
