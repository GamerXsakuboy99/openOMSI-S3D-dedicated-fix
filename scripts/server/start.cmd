@echo off
rem Run from an extracted dedicated-server ZIP: start.cmd "C:\path\to\OMSI 2" [server.cfg]
setlocal
set "HERE=%~dp0"
if "%~1"=="" (
  echo Usage: start.cmd "C:\path\to\OMSI 2" [server.cfg]
  echo The folder must contain your original OMSI 2 content.
  pause
  exit /b 1
)
set "CFG=%~2"
if "%CFG%"=="" set "CFG=%HERE%server.cfg"
"%HERE%openomsi-server.exe" --root "%~1" --server "%CFG%"
if errorlevel 1 (
  echo.
  echo Dedicated server stopped with an error. Check the console messages.
  pause
  exit /b 1
)
