@echo off
setlocal
title openOMSI Dedicated Server
set "HERE=%~dp0"
cd /d "%HERE%"

if not exist "%HERE%Start-Server.ps1" (
  echo ERROR: Start-Server.ps1 is missing.
  echo Extract the COMPLETE Windows Dedicated Server ZIP to a folder.
  pause
  exit /b 1
)

if not "%~1"=="" goto WITH_PATH
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -STA -File "%HERE%Start-Server.ps1"
goto DONE

:WITH_PATH
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -STA -File "%HERE%Start-Server.ps1" -RootPath "%~1" -ConfigPath "%~2"

:DONE
set "RESULT=%ERRORLEVEL%"
if not "%RESULT%"=="0" (
  echo.
  echo Dedicated Server could not start. See the error above.
  echo If the OMSI 2 folder has changed, run Change-OMSI-Folder.cmd.
  pause
)
exit /b %RESULT%
