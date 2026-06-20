@echo off
chcp 65001 >/dev/null
setlocal enabledelayedexpansion
title Open Dungeon - portable
set "APP_DIR=%~dp0"
cd /d "%APP_DIR%"

echo ========================================
echo   Open Dungeon - запуск
echo ========================================

REM === Движки (на месте, без копий) ===
set "TEXT_VENV=D:\Projects\TEMP\shorts-dub\.venv\Scripts\python.exe"
set "TEXT_SERVER=D:\Projects\TEMP\od-text-server.py"
set "ULTRA_DIR=D:\ultra-fast-image-gen"
set "IMAGE_VENV=%ULTRA_DIR%\.venv\Scripts\python.exe"
set "IMAGE_SERVER=%APP_DIR%image_server\optimized_image_server.py"

REM === Изоляция: всё внутрь папки/на D:, ничего в C: ===
set "TEMP=%APP_DIR%temp"
set "TMP=%APP_DIR%temp"
set "HF_HOME=%ULTRA_DIR%\models"
set "HF_HUB_CACHE=%ULTRA_DIR%\models\hub"
set "HUGGINGFACE_HUB_CACHE=%ULTRA_DIR%\models\hub"
set "TRANSFORMERS_CACHE=%ULTRA_DIR%\models"
set "TORCH_HOME=%ULTRA_DIR%\models\torch"
set "XDG_CACHE_HOME=%ULTRA_DIR%\cache"
set "TRITON_CACHE_DIR=%ULTRA_DIR%\cache\triton"
set "MFLUX_DIR=%ULTRA_DIR%\models\mflux"
set "PYTHONIOENCODING=utf-8"
set "PYTHONUNBUFFERED=1"
set "KMP_DUPLICATE_LIB_OK=TRUE"
set "NEXT_TELEMETRY_DISABLED=1"

REM === Движки картинок ===
set "ULTRA_FAST_IMAGE_GEN_DIR=%ULTRA_DIR%"
set "ULTRA_FAST_IMAGE_GEN_PYTHON=%IMAGE_VENV%"
set "IMAGE_SERVER_DEVICE=cuda"
set "IMAGE_SERVER_DEFAULT_BACKEND=sdnq-hs"
set "FLUX_WORKER_URL=http://127.0.0.1:7869"
set "SQLITE_DB_PATH=%APP_DIR%data\local-roleplay.sqlite"
set "PORT=3000"

for %%D in (temp logs data) do if not exist "%APP_DIR%%%D" mkdir "%APP_DIR%%%D" >/dev/null 2>&1

REM === Проверки ===
if not exist "%TEXT_VENV%" ( echo ОШИБКА: нет Python текстового сервера: %TEXT_VENV% & goto fail )
if not exist "%TEXT_SERVER%" ( echo ОШИБКА: нет od-text-server.py & goto fail )
if not exist "%IMAGE_VENV%" ( echo ОШИБКА: нет Python сервера картинок & goto fail )
if not exist "%APP_DIR%.next\BUILD_ID" ( echo ОШИБКА: нет сборки .next - сначала npm run build & goto fail )

echo [1/3] Текстовый сервер Gemma 4 12B на 8080...
start "OD Text :8080" /min cmd /c ""%TEXT_VENV%" "%TEXT_SERVER%" > "%APP_DIR%logs\text.log" 2>&1"

echo [2/3] Сервер картинок FLUX.2 SDNQ на 7869...
start "OD Image :7869" /min cmd /c ""%IMAGE_VENV%" "%IMAGE_SERVER%" > "%APP_DIR%logs\image.log" 2>&1"

echo [TTS] Озвучка (Qwen3-TTS) на порту 8081...
start "OD TTS :8081" /min cmd /c ""%TEXT_VENV%" "D:\Projects\TEMP\od-tts-server.py" > "%APP_DIR%logs	ts.log" 2>&1"

echo Ожидание текстового сервера...
set "OK_T="
for /l %%i in (1,1,150) do if not defined OK_T (
  curl -s -f -o nul http://127.0.0.1:8080/health && set "OK_T=1"
  if not defined OK_T ( <nul set /p "=." & ping -n 3 127.0.0.1 >/dev/null ) else ( echo  готов. )
)

echo Ожидание сервера картинок...
set "OK_I="
for /l %%i in (1,1,150) do if not defined OK_I (
  curl -s -f -o nul http://127.0.0.1:7869/health && set "OK_I=1"
  if not defined OK_I ( <nul set /p "=." & ping -n 3 127.0.0.1 >/dev/null ) else ( echo  готов. )
)

echo [3/3] Веб-приложение на 3000...
start "OD Web :3000" /min cmd /c "node node_modules\next\dist\bin\next start -p 3000 > "%APP_DIR%logs\web.log" 2>&1"

echo Ожидание веб-приложения...
set "OK_W="
for /l %%i in (1,1,90) do if not defined OK_W (
  curl -s -f -o nul http://localhost:3000/ && set "OK_W=1"
  if not defined OK_W ( <nul set /p "=." & ping -n 2 127.0.0.1 >/dev/null ) else ( echo  готов. )
)
if not defined OK_W ( echo ОШИБКА: веб не ответил, см. logs\web.log & goto fail )

start "" http://localhost:3000
echo.
echo ========================================
echo   Open Dungeon запущен!  http://localhost:3000
echo   Текст 8080 · Картинки 7869 · Логи: logs\
echo   Остановить: stop.bat
echo ========================================
pause
exit /b 0

:fail
echo.
pause
exit /b 1
