"""
TTS reader server for Open Dungeon — zero-shot voice clone via the shorts-dub
Qwen3-TTS engine. Reads the narrator's Russian text aloud in a chosen voice.

POST /tts   {text, voice?, language?}  -> audio/wav (24 kHz)
GET  /voices                            -> {default, voices:[...]}  (the voice pack)
GET  /health

Voice pack = a folder of <name>.mp3 reference clips (timbre-cloned per request).
Default game voice = Руслан Габидулин. Engine ~2.6GB VRAM (bnb-NF4 + Triton),
coexists with the Gemma LLM (~10GB) and the FLUX image model on the 4090.
Run with the shorts-dub venv python (has torch, qwen_tts, faster-qwen3-tts,
bitsandbytes, qwen3-tts-triton, soundfile, fastapi, uvicorn).
"""
import os
os.environ.setdefault("KMP_DUPLICATE_LIB_OK", "TRUE")

import glob
import hashlib
import importlib.util
import io
import re
import threading

import numpy as np  # noqa: F401  (soundfile/engine rely on numpy being importable)
import soundfile as sf
import torch  # MUST import before the Qwen/llama CUDA stack
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, Response
import uvicorn

# ============================================================================
# CONFIG  — override with env-vars; defaults work from a clone where possible,
#           with the old dev-box paths kept only as last-resort fallbacks.
# ----------------------------------------------------------------------------
#   SHORTS_DUB_DIR    : checkout of the shorts-dub project (provides the
#                       Qwen3-TTS engine module shorts_dub/tts.py). REQUIRED —
#                       this engine is an EXTERNAL dependency, not vendored here.
#   OD_TTS_ENGINE_PY  : direct path to that tts.py (overrides SHORTS_DUB_DIR).
#   OD_VOICES_DIR     : folder of <name>.mp3 reference clips = the voice pack
#                       (you supply these; see servers/README.md). Defaults to
#                       <repo>/servers/voices when present.
#   OD_VOICE_UPLOADS_DIR : where the app writes user-uploaded clone refs.
#                       Defaults to <repo>/public/uploads/voices (in-repo).
#   OD_DEFAULT_VOICE  : voice id used when none is requested.
#   OD_TTS_PORT       : listen port (default 8081; app's TTS_WORKER_URL).
#   OD_TTS_CACHE      : rendered-wav cache dir. Defaults to <repo>/.tts-cache.
# ============================================================================
_HERE = os.path.dirname(os.path.abspath(__file__))
_REPO = os.path.dirname(_HERE)  # <repo>  (servers/ is one level down)

# Locate shorts-dub's tts.py (the Qwen3-TTS engine). Priority:
#   1) OD_TTS_ENGINE_PY (explicit file)  2) SHORTS_DUB_DIR/shorts_dub/tts.py
#   3) a sibling ../shorts-dub checkout  4) the original dev-box path.
_DEV_DEFAULT_TTS_PY = r"D:\Projects\TEMP\shorts-dub\shorts_dub\tts.py"
_SHORTS_DUB_DIR = os.environ.get("SHORTS_DUB_DIR")
_TTS_PY = (
    os.environ.get("OD_TTS_ENGINE_PY")
    or (os.path.join(_SHORTS_DUB_DIR, "shorts_dub", "tts.py") if _SHORTS_DUB_DIR else None)
    or (lambda p: p if os.path.exists(p) else None)(
        os.path.join(os.path.dirname(_REPO), "shorts-dub", "shorts_dub", "tts.py")
    )
    or _DEV_DEFAULT_TTS_PY
)
if not os.path.exists(_TTS_PY):
    raise SystemExit(
        f"[od-tts] shorts-dub TTS engine not found at {_TTS_PY}\n"
        "         Set SHORTS_DUB_DIR (or OD_TTS_ENGINE_PY) to your shorts-dub checkout.\n"
        "         See servers/README.md."
    )

# Load shorts-dub's tts.py standalone (no package __init__ side effects).
_spec = importlib.util.spec_from_file_location("sd_tts", _TTS_PY)
sd_tts = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sd_tts)

_BUNDLED_VOICES = os.path.join(_HERE, "voices")  # <repo>/servers/voices (optional)
VOICES_DIR = os.environ.get("OD_VOICES_DIR") or (
    _BUNDLED_VOICES if os.path.isdir(_BUNDLED_VOICES)
    else r"D:\Projects\TEMP\DotsTTS-Studio\downloads\vp\voice-pack"  # dev box only
)
# User-uploaded clone references (written by the /api/tts-voice Next route).
# Lives inside the repo, so the repo-relative default works from a clone.
UPLOADS_DIR = os.environ.get(
    "OD_VOICE_UPLOADS_DIR", os.path.join(_REPO, "public", "uploads", "voices")
)
os.makedirs(UPLOADS_DIR, exist_ok=True)
DEFAULT_VOICE = os.environ.get("OD_DEFAULT_VOICE", "RU_Male_Gabidullin_ruslan")
HOST, PORT = "127.0.0.1", int(os.environ.get("OD_TTS_PORT", "8081"))
CACHE_DIR = os.environ.get("OD_TTS_CACHE", os.path.join(_REPO, ".tts-cache"))
os.makedirs(CACHE_DIR, exist_ok=True)


class Cfg:
    device = "cuda"
    tts_model = "Qwen/Qwen3-TTS-12Hz-1.7B-Base"
    tts_quant = "nf4"
    tts_triton = True
    tts_cuda_graphs = True


def list_voices():
    # Built-in pack first, then user uploads (uploads win on name collision).
    out = {}
    for d in (VOICES_DIR, UPLOADS_DIR):
        for f in glob.glob(os.path.join(d, "*.mp3")):
            out[os.path.splitext(os.path.basename(f))[0]] = f
    return out


def resolve_ref(voice):
    """Resolve a requested voice to a reference .mp3 path.

    Accepts: a pack/upload voice id, a bare "custom" (-> newest upload), or a
    literal .mp3 path (absolute, or a bare filename inside the uploads dir).
    Rescans the dirs every call so uploads added at runtime work without a
    server restart. Returns None when nothing usable is found.
    """
    voices = list_voices()
    if voice:
        # Direct id hit (built-in pack or an already-known upload).
        if voice in voices:
            return voices[voice]
        # A path was passed straight through.
        if voice.lower().endswith(".mp3"):
            if os.path.isabs(voice) and os.path.exists(voice):
                return voice
            cand = os.path.join(UPLOADS_DIR, os.path.basename(voice))
            if os.path.exists(cand):
                return cand
        # "custom" (or an id we lost track of) -> newest uploaded clip.
        uploads = glob.glob(os.path.join(UPLOADS_DIR, "*.mp3"))
        if voice == "custom" and uploads:
            return max(uploads, key=os.path.getmtime)
    return voices.get(DEFAULT_VOICE)


VOICES = list_voices()
print(f"[od-tts] torch {torch.__version__} cuda={torch.cuda.is_available()} | "
      f"{len(VOICES)} voices, default={DEFAULT_VOICE}", flush=True)
print("[od-tts] loading Qwen3-TTS (combo nf4+triton)...", flush=True)
_engine = sd_tts.make(Cfg())
_lock = threading.Lock()
print(f"[od-tts] ready -> http://{HOST}:{PORT}", flush=True)

app = FastAPI()


def _clean(text: str) -> str:
    # The narrator uses *italic* / **bold**; strip markup so it isn't spoken.
    text = re.sub(r"[*_`#>]+", "", text)
    return re.sub(r"[ \t]+", " ", text).strip()


@app.get("/health")
def health():
    return {"status": "ok", "voices": len(VOICES), "default": DEFAULT_VOICE,
            "device": "cuda" if torch.cuda.is_available() else "cpu"}


@app.get("/voices")
def voices():
    # Rescan so uploads added after boot appear on the next UI refresh.
    return {"default": DEFAULT_VOICE, "voices": sorted(list_voices().keys())}


@app.post("/tts")
async def tts(request: Request):
    body = await request.json()
    text = _clean(body.get("text") or "")
    if not text:
        return JSONResponse({"error": "empty text"}, status_code=400)
    voice = body.get("voice") or DEFAULT_VOICE
    ref = resolve_ref(voice)
    if not ref:
        return JSONResponse({"error": f"no reference voice (voice={voice})"}, status_code=400)
    cache_key = hashlib.sha256(f"{voice}|{text}".encode("utf-8")).hexdigest()
    cache_file = os.path.join(CACHE_DIR, cache_key + ".wav")
    if os.path.exists(cache_file):
        with open(cache_file, "rb") as fh:
            return Response(content=fh.read(), media_type="audio/wav",
                            headers={"X-Cache": "HIT", "Cache-Control": "no-store"})
    language = body.get("language") or "ru"
    # Pass the voice's reference transcript (the .mp3's .txt) so it clones with
    # the speaker's prosody, not just timbre.
    ref_text = ""
    txt_path = os.path.splitext(ref)[0] + ".txt"
    if os.path.exists(txt_path):
        try:
            ref_text = open(txt_path, encoding="utf-8-sig", errors="replace").read().strip()
        except Exception:
            ref_text = ""
    with _lock:
        audio, sr = sd_tts.clone(_engine, text, ref_text, ref, language=language, x_vector_only=not ref_text)
    buf = io.BytesIO()
    sf.write(buf, np.asarray(audio, dtype="float32"), int(sr), format="WAV")
    data = buf.getvalue()
    try:
        with open(cache_file, "wb") as fh:
            fh.write(data)
    except Exception:
        pass
    return Response(content=data, media_type="audio/wav",
                    headers={"X-Sample-Rate": str(int(sr)), "X-Cache": "MISS", "Cache-Control": "no-store"})


if __name__ == "__main__":
    uvicorn.run(app, host=HOST, port=PORT, log_level="warning")
