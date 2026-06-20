@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo ========================================
echo   Dungeon Ultimate - Update
echo ========================================

set "SCRIPT_DIR=%~dp0"
cd /d "%SCRIPT_DIR%"

where git >nul 2>&1
if errorlevel 1 ( echo ERROR: Git not found - https://git-scm.com/downloads & pause & exit /b 1 )

if exist ".git" (
    echo Pulling latest sources...
    git pull
)

if exist "node\node.exe" (
    set "PATH=%SCRIPT_DIR%node;%PATH%"
    echo Updating npm dependencies...
    call "%SCRIPT_DIR%node\npm.cmd" install
    echo Rebuilding the web app...
    call "%SCRIPT_DIR%node\npm.cmd" run build
)

echo.
echo Update complete. Start with: run.bat
pause
