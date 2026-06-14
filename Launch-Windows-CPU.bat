@echo off
setlocal
cd /d "%~dp0"

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\setup-windows.ps1" -CpuOnly
if errorlevel 1 (
  echo.
  echo Open Dungeon CPU launcher failed. See the PowerShell output above.
  pause
)
