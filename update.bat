@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion
title Dungeon Ultimate — обновление

REM ============================================================================
REM  Обновление до свежей версии: забрать изменения и пересобрать.
REM  Модели, сохранения и настройки НЕ трогаются — они лежат в папках, которые
REM  под присмотром git не находятся.
REM ============================================================================

set "ROOT=%~dp0"
cd /d "%ROOT%"
set "TEMP=%ROOT%temp"
set "TMP=%ROOT%temp"

echo.
echo  ╔══════════════════════════════════════════════════════════╗
echo  ║             Dungeon Ultimate — обновление                ║
echo  ╚══════════════════════════════════════════════════════════╝
echo.

where git >nul 2>&1
if errorlevel 1 (
    echo  [!] Не найден git — обновляться нечем.
    echo      Скачай свежую версию вручную:
    echo      https://github.com/timoncool/dungeon-ultimate/releases
    echo.
    pause
    exit /b 1
)

REM Свои правки в файлах игры остановили бы обновление — предупреждаем заранее.
git diff --quiet
if errorlevel 1 (
    echo  [!] В файлах игры есть несохранённые изменения.
    echo      Обновление их затрёт. Сохрани их или откати, потом запусти снова.
    echo.
    git status --short
    echo.
    pause
    exit /b 1
)

echo  [1/3] Забираю свежую версию…
git pull --ff-only || (
    echo  [!] Обновиться не вышло. Скачай свежую версию вручную:
    echo      https://github.com/timoncool/dungeon-ultimate/releases
    pause
    exit /b 1
)

echo  [2/3] Пересобираю интерфейс…
pushd frontend
call npm install --no-fund --no-audit || (echo  [!] npm install не прошёл & popd & pause & exit /b 1)
call npm run build || (echo  [!] сборка интерфейса не прошла & popd & pause & exit /b 1)
popd

echo  [3/3] Пересобираю игру…
cargo build --release -p du-server || (echo  [!] сборка не прошла & pause & exit /b 1)

echo.
echo  ══════════════════════════════════════════════════════════
echo   Обновлено. Сохранения и модели на месте — запускай run.bat.
echo  ══════════════════════════════════════════════════════════
echo.
pause
