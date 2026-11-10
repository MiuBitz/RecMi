@echo off
cd /d "%~dp0"
cargo build --release
if errorlevel 1 exit /b 1
echo Built target\release\recmi.exe
