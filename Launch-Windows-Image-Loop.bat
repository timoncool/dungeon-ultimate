@echo off
setlocal
cd /d "%~dp0"

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run-windows-image-loop.ps1" -Device both -Count 1 -DiagnoseOnSuccess
if errorlevel 1 (
  echo.
  echo Open Dungeon image loop failed. See the PowerShell output above and logs\windows-image-loop-*.txt.
  pause
  exit /b 1
)

echo.
echo Open Dungeon image loop completed.
pause
