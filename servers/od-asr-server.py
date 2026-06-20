"""Local ASR for Dungeon Ultimate voice input — NVIDIA Parakeet-TDT-0.6B-v3 via onnx-asr.

POST /asr with a 16 kHz mono WAV body -> {"text": "..."}.
Runs in the shorts-dub venv (onnx_asr + onnxruntime-GPU), same one as the text/TTS servers.
"""
import os
os.environ.setdefault("KMP_DUPLICATE_LIB_OK", "TRUE")

import tempfile
import torch  # import before onnxruntime-gpu so CUDA DLLs load first
import onnx_asr
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
import uvicorn

HOST, PORT = "127.0.0.1", 8082
MODEL = "nemo-parakeet-tdt-0.6b-v3"

_providers = (
    ["CUDAExecutionProvider", "CPUExecutionProvider"]
    if torch.cuda.is_available()
    else ["CPUExecutionProvider"]
)
print(f"[od-asr] loading {MODEL} (int8) providers={_providers}", flush=True)
_model = onnx_asr.load_model(MODEL, quantization="int8", providers=_providers)
print(f"[od-asr] ready -> http://{HOST}:{PORT}/asr", flush=True)

app = FastAPI()


@app.get("/health")
def health():
    return {
        "status": "ok",
        "model": MODEL,
        "device": "cuda" if torch.cuda.is_available() else "cpu",
    }


@app.post("/asr")
async def asr(request: Request):
    data = await request.body()
    if not data:
        return JSONResponse({"error": "empty audio"}, status_code=400)

    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as handle:
        handle.write(data)
        path = handle.name
    try:
        res = _model.recognize(path)
        text = res if isinstance(res, str) else getattr(res, "text", str(res))
        return {"text": (text or "").strip()}
    except Exception as exc:  # noqa: BLE001 — report the transcription error to the client
        return JSONResponse({"error": str(exc)}, status_code=500)
    finally:
        try:
            os.unlink(path)
        except Exception:
            pass


if __name__ == "__main__":
    uvicorn.run(app, host=HOST, port=PORT, log_level="warning")
