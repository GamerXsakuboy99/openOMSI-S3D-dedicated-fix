@echo off
rem Dedicated Server build for Windows (MSVC x64). No launcher or playable client.
setlocal
cd /d "%~dp0\.."
set "TARGET=x86_64-pc-windows-msvc"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>nul
if errorlevel 1 (
  echo Install the Rust MSVC toolchain from https://rustup.rs
  exit /b 1
)
cargo build --locked --release --target %TARGET% -p omsi-app --bin openomsi-server
if errorlevel 1 exit /b 1
if not exist "dist\windows-server" mkdir "dist\windows-server"
copy /y "target\%TARGET%\release\openomsi-server.exe" "dist\windows-server\openomsi-server.exe" >nul
if errorlevel 1 exit /b 1
copy /y "scripts\server\start.cmd" "dist\windows-server\start.cmd" >nul
copy /y "docs\SERVER.md" "dist\windows-server\README.md" >nul
copy /y "assets\steam_redist\steam_api64.dll" "dist\windows-server\steam_api64.dll" >nul
echo Server build created under dist\windows-server.
exit /b 0
