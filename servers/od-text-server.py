"""
OpenAI-compatible server for Open Dungeon, backed by local Gemma 4 12B GGUFs.

Supports MULTIPLE models, switchable from the app's model dropdown: /v1/models
lists them and the request's "model" field selects one. Only one model is held
in VRAM at a time — switching unloads the old one first. Both models are Gemma 4
so they share Gemma4ChatHandler (vision via mmproj, thinking channel suppressed).

Non-streaming: the app POSTs /v1/chat/completions and reads choices[0].message.
"""
import os
os.environ.setdefault("KMP_DUPLICATE_LIB_OK", "TRUE")

import gc
import re
import threading
import torch  # MUST import before llama_cpp so its CUDA DLLs load first
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, StreamingResponse
import uvicorn
from llama_cpp import Llama
from llama_cpp.llama_chat_format import Gemma4ChatHandler

# ============================================================================
# CONFIG  — override with env-vars; defaults work from a clone, with the old
#           dev-box path kept only as a last-resort fallback.
# ----------------------------------------------------------------------------
#   OD_MODELS_DIR : folder holding the Gemma 4 12B GGUFs + their mmproj files.
#                   These weights are NOT shipped in the repo (multi-GB, gated)
#                   — download them yourself (see servers/README.md). Either set
#                   OD_MODELS_DIR, or drop the files in <repo>/servers/models/mt
#                   and leave the env-var unset (that folder is auto-detected).
#   OD_TEXT_PORT  : listen port (default 8080; the app's OPENAI_COMPAT_BASE_URL).
# ============================================================================
_HERE = os.path.dirname(os.path.abspath(__file__))
_BUNDLED_MT = os.path.join(_HERE, "models", "mt")        # <repo>/servers/models/mt
_DEV_DEFAULT_MT = r"D:\Projects\TEMP\shorts-dub\models\mt"  # original dev box only
_MT = os.environ.get("OD_MODELS_DIR") or (
    _BUNDLED_MT if os.path.isdir(_BUNDLED_MT) else _DEV_DEFAULT_MT
)

# id -> metadata. Both are Gemma 4 12B → same chat handler.
MODELS = {
    "gemma-4-12b-it-qat": {
        "label": "Gemma 4 12B — обычная",
        "gguf": os.path.join(_MT, "gemma-4-12b-it-qat-q4_0.gguf"),
        "mmproj": os.path.join(_MT, "mmproj-gemma-4-12b-it-qat-q4_0.gguf"),
    },
    "gemma-4-12b-uncensored": {
        "label": "Gemma 4 12B — Uncensored (NSFW)",
        "gguf": os.path.join(_MT, "unc", "gemma-4-12b-it-uncensored-Q4_K_M.gguf"),
        "mmproj": os.path.join(_MT, "unc", "mmproj-gemma-4-12B-it-bf16.gguf"),
    },
}
DEFAULT_ID = "gemma-4-12b-uncensored"
HOST, PORT = "127.0.0.1", int(os.environ.get("OD_TEXT_PORT", "8080"))

print(f"[od-text-server] torch {torch.__version__} cuda={torch.cuda.is_available()} "
      f"({torch.cuda.get_device_name(0) if torch.cuda.is_available() else 'CPU'})", flush=True)

_lock = threading.Lock()  # protects model load + create_chat_completion (not thread-safe)
_llm = None
_handler = None
_current_id = None


def _load(model_id):
    """Ensure `model_id` is the resident model. Unloads the previous one first so
    only one 12B sits in VRAM. Caller must hold _lock."""
    global _llm, _handler, _current_id
    if model_id not in MODELS:
        model_id = DEFAULT_ID
    if _current_id == model_id and _llm is not None:
        return
    if _llm is not None:
        try:
            _llm.close()   # synchronously free the old GGUF's VRAM (ggml, not torch)
        except Exception:
            pass
    _llm = None
    _handler = None
    gc.collect()
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
        torch.cuda.synchronize()
    spec = MODELS[model_id]
    print(f"[od-text-server] loading {model_id} ({spec['label']})...", flush=True)
    _handler = Gemma4ChatHandler(clip_model_path=spec["mmproj"], enable_thinking=False)
    _llm = Llama(
        model_path=spec["gguf"],
        chat_handler=_handler,
        n_gpu_layers=-1,      # offload everything to the 4090
        n_ctx=24576,          # 24K story context
        flash_attn=True,      # required for KV-cache quantization below
        type_k=8,             # KV cache Q8_0 (~half the VRAM of F16)
        type_v=8,
        verbose=False,
    )
    _current_id = model_id
    print(f"[od-text-server] ready: {model_id}", flush=True)


with _lock:
    _load(DEFAULT_ID)
print(f"[od-text-server] serving -> http://{HOST}:{PORT}/v1  (models: {', '.join(MODELS)})", flush=True)


_TOOL_RE = re.compile(r"generate_image\s*\{(.*?)\}", re.S)


def _field(blob, name):
    m = re.search(name + r'\s*:\s*<\|"\|>(.*?)<\|"\|>', blob, re.S)
    if m:
        return m.group(1).strip()
    m = re.search(name + r'\s*:\s*"(.*?)"', blob, re.S)
    return m.group(1).strip() if m else None


def _postprocess(result):
    """Gemma 4 emits its thinking channel + tool calls as literal <|...|> markup.
    Pull a generate_image call into proper OpenAI tool_calls and strip ALL control
    markup so the visible story text stays clean."""
    try:
        msg = result["choices"][0]["message"]
    except Exception:
        return result
    content = msg.get("content") or ""
    if not msg.get("tool_calls"):
        m = _TOOL_RE.search(content)
        if m:
            prompt = _field(m.group(1), "prompt")
            reason = _field(m.group(1), "reason")
            if prompt:
                import json as _json
                args = {"prompt": prompt}
                if reason:
                    args["reason"] = reason
                msg["tool_calls"] = [{
                    "id": "call_genimg",
                    "type": "function",
                    "function": {"name": "generate_image", "arguments": _json.dumps(args, ensure_ascii=False)},
                }]
    content = re.sub(r"<\|?tool_call\|?>.*?<\|?/?tool_call\|?>", "", content, flags=re.S)
    content = re.sub(r"<\|?tool_call\|?>.*\Z", "", content, flags=re.S)
    content = re.sub(r"(?:call:)?generate_image\s*\{.*?\}", "", content, flags=re.S)
    content = re.sub(r"<\|?channel\|?>\s*\w*", "", content)
    content = re.sub(r"<\|[^>]*?>", "", content)
    content = re.sub(r"<[^<>]*?\|>", "", content)
    msg["content"] = content.strip()
    return result


app = FastAPI()


@app.get("/health")
def health():
    return {"status": "ok", "model": _current_id, "models": list(MODELS.keys()),
            "device": "cuda" if torch.cuda.is_available() else "cpu"}


@app.get("/v1/models")
def models():
    return {"object": "list", "data": [
        {"id": k, "object": "model", "owned_by": "local", "label": v["label"]}
        for k, v in MODELS.items()
    ]}


@app.post("/unload")
def unload():
    """Free the LLM's VRAM so the image backend can use the whole GPU. Only one
    model is needed at any moment; the next /v1/chat/completions reloads it
    lazily (the GGUF stays in the OS page cache, so reload is quick)."""
    global _llm, _handler
    with _lock:
        if _llm is not None:
            try:
                _llm.close()
            except Exception:
                pass
        _llm = None
        _handler = None
        gc.collect()
        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
            torch.cuda.synchronize()
    print("[od-text-server] unloaded (VRAM freed for image gen)", flush=True)
    return {"status": "unloaded", "model": _current_id}


@app.post("/v1/chat/completions")
async def chat_completions(request: Request):
    body = await request.json()
    model_id = body.get("model") or DEFAULT_ID
    kwargs = dict(
        messages=body.get("messages", []),
        temperature=body.get("temperature", 0.9),
        max_tokens=body.get("max_tokens", 2048),
    )
    tools = body.get("tools")
    tool_choice = body.get("tool_choice")

    if body.get("stream"):
        import json as _json
        def _gen():
            with _lock:
                _load(model_id)
                call = dict(kwargs, stream=True)
                if tools:
                    call["tools"] = tools
                    if tool_choice:
                        call["tool_choice"] = tool_choice
                try:
                    for chunk in _llm.create_chat_completion(**call):
                        yield "data: " + _json.dumps(chunk, ensure_ascii=False) + "\n\n"
                except Exception as exc:
                    yield "data: " + _json.dumps({"error": {"message": str(exc)}}) + "\n\n"
                yield "data: [DONE]\n\n"
        return StreamingResponse(_gen(), media_type="text/event-stream")

    with _lock:
        _load(model_id)
        try:
            call = dict(kwargs)
            if tools:
                call["tools"] = tools
                if tool_choice:
                    call["tool_choice"] = tool_choice
            result = _llm.create_chat_completion(**call)
        except Exception as exc:
            if tools:
                try:
                    result = _llm.create_chat_completion(**kwargs)
                except Exception as exc2:
                    return JSONResponse({"error": {"message": f"generation failed: {exc2}"}}, status_code=500)
            else:
                return JSONResponse({"error": {"message": f"generation failed: {exc}"}}, status_code=500)
    result = _postprocess(result)
    result.setdefault("model", _current_id)
    return JSONResponse(result)


if __name__ == "__main__":
    uvicorn.run(app, host=HOST, port=PORT, log_level="warning")
