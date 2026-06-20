# `servers/` — Dungeon Ultimate local AI servers

Three small FastAPI servers that the web app talks to over HTTP. They run in the
**embedded `python-text`** environment that `install.bat` creates (torch 2.8 +
CUDA), the same one used for all three. The image server lives separately under
`../image_server/`.

| File | Port | Role | App env-var that points at it |
|------|------|------|-------------------------------|
| `od-text-server.py` | 8080 | OpenAI-compatible LLM (Gemma 4 12B GGUF, vision + tool calls) | `OPENAI_COMPAT_BASE_URL=http://127.0.0.1:8080/v1` |
| `od-tts-server.py`  | 8081 | TTS reader — zero-shot voice clone (Qwen3-TTS) | `TTS_WORKER_URL=http://127.0.0.1:8081` |
| `od-asr-server.py`  | 8082 | Voice input — Parakeet-TDT ASR (onnx-asr) | (ASR endpoint) |

`run.bat` starts all three (plus the image server and the web app). They are
launched with the embedded Python and write logs to `../logs/`.

---

## What you must provide (NOT shipped in the repo)

These are large/gated/external, so they are **not** in git and **not** auto-installed:

### 1. Gemma 4 12B GGUF weights — required for text (the core of the app)
`od-text-server.py` loads two Gemma 4 12B models (you can ship just one and edit
the `MODELS` dict, but both are referenced by default). Place the files here:

```
servers/models/mt/
  gemma-4-12b-it-qat-q4_0.gguf
  mmproj-gemma-4-12b-it-qat-q4_0.gguf
  unc/
    gemma-4-12b-it-uncensored-Q4_K_M.gguf
    mmproj-gemma-4-12B-it-bf16.gguf
```

`servers/models/mt/` is auto-detected when present. To keep the weights elsewhere,
set **`OD_MODELS_DIR`** to the folder that contains `gemma-4-12b-it-qat-q4_0.gguf`
(and the `unc/` subfolder). If neither is found, the server falls back to the
original dev-box path `D:\Projects\TEMP\shorts-dub\models\mt` — fine on that one
machine, missing everywhere else.

> The `mmproj-*` files are the multimodal projector (vision). Both models are
> Gemma 4, so they share one chat handler; only one is held in VRAM at a time.

### 2. shorts-dub checkout — required for TTS only
`od-tts-server.py` imports the **Qwen3-TTS engine** from the separate `shorts-dub`
project (`shorts_dub/tts.py`). It is an external dependency, not vendored here.
Set **`SHORTS_DUB_DIR`** to your `shorts-dub` checkout, or **`OD_TTS_ENGINE_PY`**
to the `tts.py` file directly. A sibling `../shorts-dub` next to this repo is also
auto-detected. Without it the TTS server exits with a clear message; text + images
still work.

The `python-text` env already installs the TTS runtime wheels (`faster-qwen3-tts`,
`qwen-tts`, `bitsandbytes`, `soundfile`, and — when vendored — `qwen3-tts-triton`).

### 3. Voice pack — required for TTS only
A folder of `<name>.mp3` reference clips (optionally a matching `<name>.txt`
transcript next to each, which improves prosody cloning). Put them in
`servers/voices/` (auto-detected) or set **`OD_VOICES_DIR`**. The default voice id
is `RU_Male_Gabidullin_ruslan` (override with `OD_DEFAULT_VOICE`). User-uploaded
clone clips from the app are written to `../public/uploads/voices/` and merged in.

### 4. NVIDIA GPU + driver
All three load on CUDA. A recent NVIDIA driver must be installed; the CUDA runtime
itself rides along in the torch wheels chosen by `install.bat`. The reference box
is a single 4090 holding the LLM (~10 GB) + TTS (~2.6 GB) + the FLUX image model
simultaneously. On smaller cards, expect the image server to ask the text server
to `/unload` first (that handshake is built in).

### Auto-downloaded (no action needed)
- The **Parakeet ASR** model (`nemo-parakeet-tdt-0.6b-v3`) is fetched by `onnx_asr`
  into the Hugging Face cache on first run. Set `HF_HOME` to relocate the cache
  (`run.bat` already points it inside the image repo's `models/`).

---

## Environment variables

All have sensible defaults; set only what you need to relocate.

### `od-text-server.py`
| Var | Default | Meaning |
|-----|---------|---------|
| `OD_MODELS_DIR` | `servers/models/mt` if present, else the dev-box path | Folder with the Gemma GGUFs + mmproj |
| `OD_TEXT_PORT` | `8080` | Listen port |

### `od-tts-server.py`
| Var | Default | Meaning |
|-----|---------|---------|
| `SHORTS_DUB_DIR` | sibling `../shorts-dub` if present, else dev-box path | shorts-dub checkout (Qwen3-TTS engine) |
| `OD_TTS_ENGINE_PY` | — | Direct path to `shorts_dub/tts.py` (overrides `SHORTS_DUB_DIR`) |
| `OD_VOICES_DIR` | `servers/voices` if present, else dev-box pack | Built-in voice pack folder |
| `OD_VOICE_UPLOADS_DIR` | `../public/uploads/voices` | Where the app writes uploaded clone refs |
| `OD_DEFAULT_VOICE` | `RU_Male_Gabidullin_ruslan` | Voice id used when none is requested |
| `OD_TTS_PORT` | `8081` | Listen port |
| `OD_TTS_CACHE` | `../.tts-cache` | Rendered-wav cache dir |

### `od-asr-server.py`
| Var | Default | Meaning |
|-----|---------|---------|
| `OD_ASR_PORT` | `8082` | Listen port |
| `OD_ASR_MODEL` | `nemo-parakeet-tdt-0.6b-v3` | onnx-asr model id |
| `HF_HOME` | (HF default) | Relocates the downloaded ASR model cache |

---

## Running one server by hand (debugging)

From the repo root, with the embedded Python:

```bat
set "SHORTS_DUB_DIR=D:\path\to\shorts-dub"
set "OD_MODELS_DIR=%CD%\servers\models\mt"
python-text\python.exe servers\od-text-server.py
python-text\python.exe servers\od-tts-server.py
python-text\python.exe servers\od-asr-server.py
```

Health checks: `http://127.0.0.1:8080/health`, `:8081/health`, `:8082/health`.
