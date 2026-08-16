@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion
title Dungeon Ultimate

REM ============================================================================
REM  Запуск игры. Если сборки ещё нет — сам зовёт install.bat.
REM  Всё живёт внутри этой папки: модели, сохранения, кадры, временные файлы.
REM  Удалил папку — удалил игру, в системе ничего не остаётся.
REM ============================================================================

set "ROOT=%~dp0"
cd /d "%ROOT%"
set "TEMP=%ROOT%temp"
set "TMP=%ROOT%temp"
if not exist "temp" mkdir "temp"

set "EXE=target\release\du-server.exe"
if not exist "%EXE%" (
    echo  Сборки нет — запускаю установку.
    call "%ROOT%install.bat" || exit /b 1
)
if not exist "frontend\dist\index.html" (
    echo  Интерфейс не собран — запускаю установку.
    call "%ROOT%install.bat" || exit /b 1
)

REM Порт можно переопределить: DU_PORT=8899 run.bat
if "%DU_PORT%"=="" set "DU_PORT=8770"

echo.
echo   Dungeon Ultimate — http://127.0.0.1:%DU_PORT%
echo   Окно закроешь — игра остановится.
echo.

REM Браузер открываем с задержкой: сервер должен успеть поднять порт.
start "" /b cmd /c "timeout /t 2 >nul & start http://127.0.0.1:%DU_PORT%"
"%EXE%"

echo.
echo  Игра остановлена.
pause
