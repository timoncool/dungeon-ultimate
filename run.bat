@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion
title Dungeon Ultimate - run
set "APP_DIR=%~dp0"
cd /d "%APP_DIR%"

echo ========================================
echo   Dungeon Ultimate - запуск
echo ========================================

REM ============================================================
REM  Engines. Everything lives INSIDE the repo now:
REM    servers\        -> text (8080), TTS (8081), ASR (8082)
REM    image_server\   -> image (7869)
REM  Pythons are the embedded ones created by install.bat
REM  (python-text = torch 2.8 stack, python-image = torch 2.11 stack).
REM  Override any of these by setting the var before calling run.bat.
REM ============================================================
if not defined TEXT_VENV  set "TEXT_VENV=%APP_DIR%python-text\python.exe"
if not defined IMAGE_VENV set "IMAGE_VENV=%APP_DIR%python-image\python.exe"
set "TEXT_SERVER=%APP_DIR%servers\od-text-server.py"
set "TTS_SERVER=%APP_DIR%servers\od-tts-server.py"
set "ASR_SERVER=%APP_DIR%servers\od-asr-server.py"
set "IMAGE_SERVER=%APP_DIR%image_server\optimized_image_server.py"

REM Fallback: if the embedded python-text isn't there, reuse the old external
REM shorts-dub venv so the original dev box still runs unchanged.
if not exist "%TEXT_VENV%"  if exist "D:\Projects\TEMP\shorts-dub\.venv\Scripts\python.exe" set "TEXT_VENV=D:\Projects\TEMP\shorts-dub\.venv\Scripts\python.exe"
if not exist "%IMAGE_VENV%" if exist "D:\ultra-fast-image-gen\.venv\Scripts\python.exe"   set "IMAGE_VENV=D:\ultra-fast-image-gen\.venv\Scripts\python.exe"

REM === Image backend (ultra-fast-image-gen — auto-cloned into this folder by
REM     install.bat; the dev-box path is only a last-resort fallback) ===
if not defined ULTRA_DIR if exist "%APP_DIR%ultra-fast-image-gen\generate.py" set "ULTRA_DIR=%APP_DIR%ultra-fast-image-gen"
if not defined ULTRA_DIR set "ULTRA_DIR=D:\ultra-fast-image-gen"

REM === TTS engine is VENDORED at servers\tts_engine.py — od-tts-server loads it
REM     by default, so no checkout is needed. Set SHORTS_DUB_DIR or OD_TTS_ENGINE_PY
REM     only to override it with an external tts.py. ===

REM === Isolation: keep caches on D:/in-folder, nothing in C: ===
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

REM === Image worker wiring ===
set "ULTRA_FAST_IMAGE_GEN_DIR=%ULTRA_DIR%"
set "ULTRA_FAST_IMAGE_GEN_PYTHON=%IMAGE_VENV%"
set "IMAGE_SERVER_DEVICE=cuda"
set "IMAGE_SERVER_DEFAULT_BACKEND=sdnq-hs"
set "FLUX_WORKER_URL=http://127.0.0.1:7869"
set "TTS_WORKER_URL=http://127.0.0.1:8081"
set "SQLITE_DB_PATH=%APP_DIR%data\local-roleplay.sqlite"
set "PORT=3000"

for %%D in (temp logs data) do if not exist "%APP_DIR%%%D" mkdir "%APP_DIR%%%D" >nul 2>&1

REM === Sanity checks ===
if not exist "%TEXT_VENV%" ( echo ОШИБКА: нет Python текстового сервера: %TEXT_VENV%  ^(запустите install.bat^) & goto fail )
if not exist "%TEXT_SERVER%" ( echo ОШИБКА: нет servers\od-text-server.py & goto fail )
if not exist "%IMAGE_VENV%" ( echo ОШИБКА: нет Python сервера картинок: %IMAGE_VENV%  ^(запустите install.bat^) & goto fail )
if not exist "node_modules\next" ( echo ОШИБКА: нет node_modules - сначала install.bat ^(npm install^) & goto fail )

echo [1/4] Текстовый сервер Gemma 4 12B на 8080...
start "OD Text :8080" /min cmd /c ""%TEXT_VENV%" "%TEXT_SERVER%" > "%APP_DIR%logs\text.log" 2>&1"

echo [2/4] Сервер картинок FLUX.2 SDNQ на 7869...
start "OD Image :7869" /min cmd /c ""%IMAGE_VENV%" "%IMAGE_SERVER%" > "%APP_DIR%logs\image.log" 2>&1"

echo [3/4] Озвучка Qwen3-TTS на 8081 и распознавание речи Parakeet на 8082...
start "OD TTS :8081" /min cmd /c ""%TEXT_VENV%" "%TTS_SERVER%" > "%APP_DIR%logs\tts.log" 2>&1"
start "OD ASR :8082" /min cmd /c ""%TEXT_VENV%" "%ASR_SERVER%" > "%APP_DIR%logs\asr.log" 2>&1"

echo Ожидание текстового сервера...
set "OK_T="
for /l %%i in (1,1,150) do if not defined OK_T (
  curl -s -f -o nul http://127.0.0.1:8080/health && set "OK_T=1"
  if not defined OK_T ( <nul set /p "=." & ping -n 3 127.0.0.1 >nul ) else ( echo  готов. )
)

echo Ожидание сервера картинок...
set "OK_I="
for /l %%i in (1,1,150) do if not defined OK_I (
  curl -s -f -o nul http://127.0.0.1:7869/health && set "OK_I=1"
  if not defined OK_I ( <nul set /p "=." & ping -n 3 127.0.0.1 >nul ) else ( echo  готов. )
)

REM TTS/ASR are optional for play (voice in/out). Don't block startup on them;
REM they finish loading in their own windows and the UI picks them up when ready.

echo [4/4] Веб-приложение (npm run dev) на 3000...
REM Prefer the portable Node from install.bat; fall back to a system npm on PATH.
set "NPM_CMD=npm"
if exist "%APP_DIR%node\npm.cmd" set "NPM_CMD=%APP_DIR%node\npm.cmd"
if exist "%APP_DIR%node\node.exe" set "PATH=%APP_DIR%node;%PATH%"
start "OD Web :3000" /min cmd /c ""%NPM_CMD%" run dev > "%APP_DIR%logs\web.log" 2>&1"

echo Ожидание веб-приложения...
set "OK_W="
for /l %%i in (1,1,120) do if not defined OK_W (
  curl -s -f -o nul http://localhost:3000/ && set "OK_W=1"
  if not defined OK_W ( <nul set /p "=." & ping -n 2 127.0.0.1 >nul ) else ( echo  готов. )
)
if not defined OK_W ( echo ОШИБКА: веб не ответил, см. logs\web.log & goto fail )

start "" http://localhost:3000
echo.
echo ========================================
echo   Dungeon Ultimate запущен!  http://localhost:3000
echo   Текст 8080 · Картинки 7869 · Озвучка 8081 · Речь 8082
echo   Логи: logs\   ·   Остановить: stop.bat
echo ========================================
pause
exit /b 0

:fail
echo.
pause
exit /b 1
