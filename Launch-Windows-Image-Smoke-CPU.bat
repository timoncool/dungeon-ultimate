@echo off
setlocal
cd /d "%~dp0"

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\smoke-windows-image.ps1" -CpuOnly
if errorlevel 1 (
  echo.
  echo Open Dungeon CPU image smoke failed. See the PowerShell output above.
  pause
  exit /b 1
)

echo.
echo Open Dungeon CPU image smoke completed.
pause
