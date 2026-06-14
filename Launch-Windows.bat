@echo off
setlocal
cd /d "%~dp0"

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\launch-windows.ps1"
if errorlevel 1 (
  echo.
  echo Open Dungeon launcher failed. See the PowerShell output above.
  pause
)
