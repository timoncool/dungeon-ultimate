@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo ========================================
echo   Dungeon Ultimate - Install
echo ========================================
echo.
echo This sets up everything the four local servers + the web app need:
echo   * python-text : torch 2.8 stack - runs servers\od-text-server.py (LLM, 8080),
echo                   servers\od-tts-server.py (Qwen3-TTS, 8081),
echo                   servers\od-asr-server.py (Parakeet ASR, 8082)
echo   * python-image: torch 2.11 stack - runs image_server (FLUX.2 SDNQ, 7869)
echo   * portable Node + npm install (the Next.js web app on 3000)
echo Both Pythons are embedded; everything stays in this folder (nothing in C:).
echo.
echo YOU MUST PROVIDE (these are NOT downloaded automatically):
echo   1. An NVIDIA GPU + recent driver. CUDA toolkit is bundled via the torch
echo      wheels, but the driver itself must already be installed.
echo   2. Gemma 4 12B GGUF weights (normal + uncensored) + their mmproj files.
echo      Put them in  servers\models\mt\  (and ...\mt\unc\ for the uncensored
echo      pair), OR set OD_MODELS_DIR. See servers\README.md for exact filenames.
echo   3. A shorts-dub checkout (provides the Qwen3-TTS engine for the TTS server).
echo      Set SHORTS_DUB_DIR to it. Without it, TTS is disabled (text still works).
echo   4. A voice pack: <name>.mp3 reference clips in servers\voices\ (or set
echo      OD_VOICES_DIR). Needed only for TTS. See servers\README.md.
echo   5. ultra-fast-image-gen checkout for image generation. Set
echo      ULTRA_FAST_IMAGE_GEN_DIR. Without it, images are disabled.
echo The Gemma GGUF / FLUX SDNQ / ASR model weights that ARE on Hugging Face get
echo pulled on first run; the items above are user-supplied.
echo.
pause

set "SCRIPT_DIR=%~dp0"
cd /d "%SCRIPT_DIR%"
set "TEMP=%SCRIPT_DIR%temp"
set "TMP=%SCRIPT_DIR%temp"

REM === Create directories (servers\models\mt + servers\voices are where you
REM     drop the user-supplied Gemma weights and voice clips) ===
for %%D in (downloads temp models cache logs data servers\models\mt servers\voices) do if not exist "%%D" mkdir "%%D"

REM ============================================================
REM  Step 1: GPU selection (sets CUDA + the pinned text-venv wheels)
REM ============================================================
echo Select your GPU:
echo.
echo   1. NVIDIA RTX 40xx (Ada)       - cu128  [fully supported]
echo   2. NVIDIA RTX 50xx (Blackwell) - cu128  [fully supported]
echo   3. NVIDIA RTX 30xx (Ampere)    - cu126  [needs matching llama-cpp/flash wheels]
echo   4. NVIDIA RTX 20xx (Turing)    - cu126  [no flash-attn]
echo   5. NVIDIA GTX 10xx (Pascal)    - cu118  [no flash-attn]
echo.
set /p GPU_CHOICE="Enter number (1-5): "

if "%GPU_CHOICE%"=="1" goto :gpu_cu128
if "%GPU_CHOICE%"=="2" goto :gpu_cu128
if "%GPU_CHOICE%"=="3" goto :gpu_cu126
if "%GPU_CHOICE%"=="4" goto :gpu_cu126
if "%GPU_CHOICE%"=="5" goto :gpu_cu118
echo Invalid choice!
pause
exit /b 1

:gpu_cu128
set "CUDA_VERSION=cu128"
set "TORCH_TEXT=2.8.0"
set "TORCH_IMAGE=2.11.0"
set "LLAMA_WHL=https://github.com/JamePeng/llama-cpp-python/releases/download/v0.3.40-cu128-win-20260608/llama_cpp_python-0.3.40+cu128-cp311-cp311-win_amd64.whl"
set "FLASH_WHL=https://github.com/kingbri1/flash-attention/releases/download/v2.8.3/flash_attn-2.8.3+cu128torch2.8.0cxx11abiFALSE-cp311-cp311-win_amd64.whl"
set "INSTALL_FLASH=1"
goto :gpu_done

:gpu_cu126
set "CUDA_VERSION=cu126"
set "TORCH_TEXT=2.8.0"
set "TORCH_IMAGE=2.11.0"
set "LLAMA_WHL="
set "FLASH_WHL="
set "INSTALL_FLASH=0"
echo.
echo [!] cu126 selected: pick the matching cu126 llama-cpp wheel from
echo     https://github.com/JamePeng/llama-cpp-python/releases  and set LLAMA_WHL.
goto :gpu_done

:gpu_cu118
set "CUDA_VERSION=cu118"
set "TORCH_TEXT=2.8.0"
set "TORCH_IMAGE=2.11.0"
set "LLAMA_WHL="
set "FLASH_WHL="
set "INSTALL_FLASH=0"
echo.
echo [!] cu118 selected: pick the matching cu118 llama-cpp wheel from
echo     https://github.com/JamePeng/llama-cpp-python/releases  and set LLAMA_WHL.
goto :gpu_done

:gpu_done
echo.
echo Selected CUDA: %CUDA_VERSION%  (text torch %TORCH_TEXT%, image torch %TORCH_IMAGE%)
echo.

REM ============================================================
REM  Step 2: Python 3.11.9 embedded x2  (python-text, python-image)
REM ============================================================
if not exist "downloads\python.zip" (
    echo [1/9] Downloading Python 3.11.9 embed...
    powershell -Command "& {[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri 'https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip' -OutFile 'downloads\python.zip'}"
)
if not exist "downloads\python.zip" ( echo ERROR: Python download failed & pause & exit /b 1 )

call :setup_python python-text
call :setup_python python-image

REM ============================================================
REM  Step 3: TEXT/TTS env  (torch %TORCH_TEXT% + llama-cpp + Qwen3-TTS)
REM ============================================================
echo [4/9] Installing PyTorch %TORCH_TEXT% (%CUDA_VERSION%) into python-text...
python-text\python.exe -m pip install torch==%TORCH_TEXT% torchaudio==%TORCH_TEXT% torchvision --index-url https://download.pytorch.org/whl/%CUDA_VERSION% --no-warn-script-location

echo [5/9] Installing text/TTS dependencies...
if defined LLAMA_WHL python-text\python.exe -m pip install "%LLAMA_WHL%" --no-warn-script-location
python-text\python.exe -m pip install "triton-windows>=3.7,<3.8" --no-warn-script-location
if "%INSTALL_FLASH%"=="1" if defined FLASH_WHL python-text\python.exe -m pip install "%FLASH_WHL%" --no-warn-script-location
python-text\python.exe -m pip install "transformers==4.57.3" accelerate bitsandbytes==0.49.2 soundfile rotary-embedding-torch torchdiffeq fastapi "uvicorn[standard]" sentencepiece --no-warn-script-location
python-text\python.exe -m pip install faster-qwen3-tts==0.2.6 qwen-tts==0.1.1 --no-warn-script-location
REM qwen3-tts-triton: vendored source (was installed from a local F: path in dev). Edit path if needed.
if exist "servers\qwen3-tts-triton\pyproject.toml" (
    python-text\python.exe -m pip install -e servers\qwen3-tts-triton --no-warn-script-location
) else (
    echo [!] servers\qwen3-tts-triton not vendored yet - TTS triton kernels will be missing.
)

REM ASR (servers\od-asr-server.py): Parakeet via onnx-asr on the GPU. Shares this
REM same python-text env. onnxruntime-gpu must match the installed CUDA runtime.
python-text\python.exe -m pip install onnx-asr onnxruntime-gpu --no-warn-script-location

call :patch_triton_headers python-text

REM ============================================================
REM  Step 4: IMAGE env  (torch %TORCH_IMAGE% + diffusers + sdnq)
REM ============================================================
echo [6/9] Installing PyTorch %TORCH_IMAGE% (%CUDA_VERSION%) into python-image...
python-image\python.exe -m pip install torch==%TORCH_IMAGE% torchvision --index-url https://download.pytorch.org/whl/%CUDA_VERSION% --no-warn-script-location

echo [7/9] Installing image dependencies (diffusers + SDNQ)...
python-image\python.exe -m pip install "git+https://github.com/huggingface/diffusers@7bf00006aa005eae37bcc639fd0f010c183365b4" --no-warn-script-location
python-image\python.exe -m pip install "git+https://github.com/Disty0/sdnq.git@b7f7dcd548487788c65038832183446c99311adf" --no-warn-script-location
python-image\python.exe -m pip install gguf==0.19.0 optimum-quanto==0.2.7 transformers accelerate sentencepiece fastapi "uvicorn[standard]" python-dotenv --no-warn-script-location

REM ============================================================
REM  Step 5: Node.js 22 LTS (portable) + npm install + build
REM ============================================================
if exist "node\node.exe" (
    echo [OK] Node.js already installed
) else (
    echo [8/9] Downloading Node.js 22 LTS...
    powershell -Command "& {[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri 'https://nodejs.org/dist/v22.18.0/node-v22.18.0-win-x64.zip' -OutFile 'downloads\node.zip'}"
    powershell -Command "& {Expand-Archive -Path 'downloads\node.zip' -DestinationPath 'downloads\node-extract' -Force}"
    powershell -Command "& {Get-ChildItem 'downloads\node-extract\node-*\*' | Move-Item -Destination 'node' -Force}"
    if exist "downloads\node-extract" rmdir /s /q "downloads\node-extract"
)
set "PATH=%SCRIPT_DIR%node;%PATH%"

echo [9/9] Installing npm dependencies...
call "%SCRIPT_DIR%node\npm.cmd" install
REM run.bat launches the app with `npm run dev`, so a production build is NOT
REM required. We still try it (so `npm start` works too); a build failure here
REM is non-fatal for dev mode.
call "%SCRIPT_DIR%node\npm.cmd" run build || echo [!] build failed - that's OK, run.bat uses `npm run dev`. Run `npm run build` later if you want `npm start`.

echo %CUDA_VERSION%> cuda_version.txt

echo.
echo ========================================
echo   Installation complete.
echo   Start with: run.bat   (launches text 8080, image 7869, TTS 8081, ASR 8082, web 3000)
echo.
echo   On first run, Hugging Face models (FLUX SDNQ, uncensored TE, ASR) download
echo   automatically. The Gemma GGUFs, the shorts-dub TTS engine, the voice pack
echo   and ultra-fast-image-gen are USER-SUPPLIED - see the list above and
echo   servers\README.md. Without them, those features stay disabled but the rest
echo   of the app still runs.
echo ========================================
pause
exit /b 0

REM ============================================================
REM  Helper: set up one embedded Python in %1 (+ pip)
REM ============================================================
:setup_python
set "PYDIR=%~1"
if exist "%PYDIR%\python.exe" ( echo [OK] %PYDIR% already set up & goto :eof )
echo   Extracting %PYDIR%...
powershell -Command "& {Expand-Archive -Path 'downloads\python.zip' -DestinationPath '%PYDIR%' -Force}"
cd "%PYDIR%"
if exist "python311._pth" (
    echo python311.zip> python311._pth
    echo .>> python311._pth
    echo ..\Lib\site-packages>> python311._pth
    echo import site>> python311._pth
)
cd ..
if not exist "%PYDIR%\Scripts\pip.exe" (
    if not exist "downloads\get-pip.py" powershell -Command "& {[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri 'https://bootstrap.pypa.io/get-pip.py' -OutFile 'downloads\get-pip.py'}"
    "%PYDIR%\python.exe" downloads\get-pip.py --no-warn-script-location
)
"%PYDIR%\python.exe" -m pip install --upgrade pip --no-warn-script-location
goto :eof

REM ============================================================
REM  Helper: Python headers for Triton launcher compilation in %1
REM ============================================================
:patch_triton_headers
set "PYDIR=%~1"
if exist "%PYDIR%\Include\Python.h" goto :eof
for /f "tokens=*" %%v in ('"%PYDIR%\python.exe" -c "import sys;print(f'{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}')"') do set "PY_VER=%%v"
powershell -Command "& {[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri 'https://www.python.org/ftp/python/!PY_VER!/amd64/dev.msi' -OutFile 'downloads\pydev.msi'}"
if exist "downloads\pydev.msi" (
    msiexec /a "downloads\pydev.msi" /qn TARGETDIR="%SCRIPT_DIR%downloads\pydev_extract"
    if not exist "%PYDIR%\Include" mkdir "%PYDIR%\Include"
    if not exist "%PYDIR%\libs" mkdir "%PYDIR%\libs"
    xcopy /E /Y "downloads\pydev_extract\include\*" "%PYDIR%\Include\" >nul 2>&1
    xcopy /E /Y "downloads\pydev_extract\libs\*" "%PYDIR%\libs\" >nul 2>&1
    if exist "downloads\pydev_extract" rmdir /s /q "downloads\pydev_extract"
)
goto :eof
