#!/usr/bin/env python3
"""HTTP wrapper for the optimized uncensored FLUX.2-klein backends.

This server intentionally delegates generation to the existing
ultra-fast-image-gen entrypoints instead of reimplementing optimization logic.
"""

from __future__ import annotations

import base64
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import random
import re
import select
import signal
import subprocess
import sys
import threading
import time
import uuid
from typing import Any
from urllib.parse import urlparse
import urllib.request


APP_ROOT = Path(__file__).resolve().parents[1]
ULTRA_REPO = Path(
    os.environ.get("ULTRA_FAST_IMAGE_GEN_DIR", str(Path.home() / "ultra-fast-image-gen"))
).expanduser()
RUNTIME_PLATFORM = os.environ.get("IMAGE_SERVER_PLATFORM_OVERRIDE", sys.platform)


def default_ultra_python() -> Path:
    if os.name == "nt":
        return ULTRA_REPO / ".venv" / "Scripts" / "python.exe"
    return ULTRA_REPO / ".venv" / "bin" / "python"


def default_image_device() -> str:
    configured = os.environ.get("IMAGE_SERVER_DEVICE", "").strip().lower()
    if configured and configured != "auto":
        return configured
    return "mps" if RUNTIME_PLATFORM == "darwin" else "cuda"


PYTHON = Path(os.environ.get("ULTRA_FAST_IMAGE_GEN_PYTHON", default_ultra_python())).expanduser()
GENERATE = ULTRA_REPO / "generate.py"
# Patched MFLUX checkout created by ultra-fast-image-gen/scripts/setup_mflux_hs.sh
MFLUX_DIR = Path(
    os.environ.get("MFLUX_DIR")
    or os.environ.get("ULTRA_FAST_MFLUX_HS_DIR")
    or str(Path.home() / ".cache/ultra-fast-image-gen/mflux")
).expanduser()
OUT_DIR = Path(os.environ.get("IMAGE_SERVER_OUTPUT_DIR", APP_ROOT / "public/generated"))
HOST = os.environ.get("IMAGE_SERVER_HOST", "127.0.0.1")
PORT = int(os.environ.get("IMAGE_SERVER_PORT", "7869"))
DEFAULT_TIMEOUT = int(os.environ.get("IMAGE_SERVER_TIMEOUT", "240"))
IMAGE_DEVICE = default_image_device()
DEFAULT_BACKEND = os.environ.get(
    "IMAGE_SERVER_DEFAULT_BACKEND",
    "mflux-hs" if RUNTIME_PLATFORM == "darwin" else "sdnq-hs",
)

RUNTIME_LOCK = threading.Lock()
STATUS: dict[str, Any] = {
    "lastWarm": None,
    "lastGenerate": None,
    "residentBackends": [],
    "mfluxResident": None,
}
BACKENDS = {
    "mflux-hs": "flux2-4b-uncensored-mflux-hs",
    "sdnq-hs": "flux2-4b-uncensored-sdnq-hs",
    "flux-uncensored": "flux2-4b-uncensored",
}
STANDARD_SDNQ_MODEL = "flux2-4b-sdnq"
MFLUX_BACKEND_CONFIGS = {
    "mflux-hs": {
        "model": "flux2-klein-4b",
        "ggufVariant": "4b",
        "label": "MFLUX/MLX 4B uncensored HS",
    },
}
MFLUX_RESIDENT_ENABLED = os.environ.get("MFLUX_RESIDENT", "1") not in (
    "",
    "0",
    "false",
    "False",
)
MFLUX_WORKER = Path(__file__).resolve().parent / "mflux_resident_worker.py"


@dataclass(frozen=True)
class Dimensions:
    width: int
    height: int
    aspect: str


def json_response(handler: BaseHTTPRequestHandler, status: int, payload: dict[str, Any]) -> None:
    body = json.dumps(payload).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json")
    handler.send_header("Content-Length", str(len(body)))
    handler.end_headers()
    handler.wfile.write(body)


def read_json(handler: BaseHTTPRequestHandler) -> dict[str, Any]:
    length = int(handler.headers.get("Content-Length", "0"))
    if length <= 0:
        return {}
    return json.loads(handler.rfile.read(length).decode("utf-8"))


def clamp_int(value: Any, default: int, lower: int, upper: int) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        parsed = default
    return max(lower, min(upper, parsed))


def resolve_dimensions(payload: dict[str, Any]) -> Dimensions:
    aspect = payload.get("aspect")
    if aspect not in ("square", "portrait", "landscape"):
        aspect = "square"

    if payload.get("width") and payload.get("height"):
        return Dimensions(
            width=clamp_int(payload.get("width"), 2048, 256, 2048),
            height=clamp_int(payload.get("height"), 2048, 256, 2048),
            aspect=aspect,
        )

    mode = payload.get("mode")
    long_side = 2048 if mode == "slow" else 1024

    if aspect == "portrait":
        return Dimensions(width=round(long_side * 0.75), height=long_side, aspect=aspect)

    if aspect == "landscape":
        return Dimensions(width=long_side, height=round(long_side * 0.75), aspect=aspect)

    return Dimensions(width=long_side, height=long_side, aspect=aspect)


def slug(value: str) -> str:
    cleaned = re.sub(r"[^a-zA-Z0-9_-]+", "-", value.strip().lower()).strip("-")
    return cleaned[:42] or "image"


def resolve_public_reference(url: str) -> Path | None:
    """Map an app-served public URL to a file under ./public, sandboxed.

    Accepts any path the Next app serves from its public/ root — both player
    uploads (/uploads/...) and generated scenes/items (/generated/...). Earlier
    this only matched /uploads/, so reusing a GENERATED image (the evolving hero
    reference or an item portrait) as an init image was silently dropped. The
    resolved path is confined to public/ to block traversal (e.g. /../).
    """
    if not url.startswith("/"):
        return None
    public_root = (APP_ROOT / "public").resolve()
    # Strip any query/hash a URL might carry before hitting the filesystem.
    clean_url = url.split("?", 1)[0].split("#", 1)[0]
    candidate = (public_root / clean_url.lstrip("/")).resolve()
    if public_root not in candidate.parents and candidate != public_root:
        return None
    if candidate.is_file():
        return candidate
    return None


def prepare_reference_paths(references: list[dict[str, Any]], image_id: str) -> tuple[list[Path], list[str]]:
    reference_paths: list[Path] = []
    warnings: list[str] = []
    refs_dir = OUT_DIR / "refs"
    refs_dir.mkdir(parents=True, exist_ok=True)

    for index, reference in enumerate(references[:2], start=1):
        data_url = str(reference.get("dataUrl") or "")
        url = str(reference.get("url") or "")

        if data_url.startswith("data:image/"):
            header, _, encoded = data_url.partition(",")
            mime = header.split(";", 1)[0].removeprefix("data:")
            extension = {
                "image/png": "png",
                "image/jpeg": "jpg",
                "image/webp": "webp",
                "image/gif": "gif",
            }.get(mime)
            if not extension or not encoded:
                warnings.append(f"Reference {index} was skipped because its data URL was invalid.")
                continue

            path = refs_dir / f"{image_id}-ref-{index}.{extension}"
            path.write_bytes(base64.b64decode(encoded))
            reference_paths.append(path)
            continue

        # Any local public path (uploads OR generated), not just /uploads/.
        local_path = resolve_public_reference(url)
        if local_path is not None:
            reference_paths.append(local_path)
            continue

        warnings.append(f"Reference {index} was skipped because no local image data was available.")

    return reference_paths, warnings


class MfluxResident:
    def __init__(self) -> None:
        self.proc: subprocess.Popen[str] | None = None
        self.lock = threading.Lock()
        self.last_stderr = ""

    def is_running(self) -> bool:
        return self.proc is not None and self.proc.poll() is None

    def start(self) -> None:
        if self.is_running():
            return

        if not MFLUX_WORKER.exists():
            raise FileNotFoundError(f"Missing resident worker: {MFLUX_WORKER}")

        env = mflux_env(steps=4)
        cmd = [
            "uv",
            "run",
            "--project",
            str(MFLUX_DIR),
            "--with",
            "gguf",
            "--with",
            "accelerate",
            "--with",
            "python-dotenv",
            "python",
            str(MFLUX_WORKER),
        ]
        self.proc = subprocess.Popen(
            cmd,
            cwd=str(ULTRA_REPO),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            start_new_session=True,
            env=env,
        )
        assert self.proc.stderr is not None
        threading.Thread(target=self._drain_stderr, args=(self.proc.stderr,), daemon=True).start()

    def _drain_stderr(self, stream) -> None:
        for line in stream:
            self.last_stderr = (self.last_stderr + line)[-8000:]
            print(f"[mflux-resident] {line}", end="", flush=True)

    def request(self, payload: dict[str, Any], timeout: int) -> dict[str, Any]:
        with self.lock:
            self.start()
            assert self.proc is not None
            assert self.proc.stdin is not None
            assert self.proc.stdout is not None

            request_id = str(uuid.uuid4())
            self.proc.stdin.write(json.dumps({"id": request_id, **payload}) + "\n")
            self.proc.stdin.flush()

            deadline = time.time() + timeout
            while time.time() < deadline:
                ready, _, _ = select.select([self.proc.stdout], [], [], 0.25)
                if not ready:
                    if self.proc.poll() is not None:
                        raise RuntimeError(
                            f"MFLUX resident worker exited with {self.proc.returncode}\n{self.last_stderr}"
                        )
                    continue
                line = self.proc.stdout.readline()
                if not line:
                    if self.proc.poll() is not None:
                        raise RuntimeError(
                            f"MFLUX resident worker exited with {self.proc.returncode}\n{self.last_stderr}"
                        )
                    continue
                response = json.loads(line)
                if response.get("id") != request_id:
                    continue
                if not response.get("ok"):
                    raise RuntimeError(
                        f"MFLUX resident request failed: {response.get('error')}\n"
                        f"{response.get('traceback', '')}\n{self.last_stderr}"
                    )
                return response

            raise TimeoutError(f"MFLUX resident request timed out after {timeout}s\n{self.last_stderr}")


MFLUX_RESIDENT = MfluxResident()


def is_mflux_backend(backend: str) -> bool:
    return backend in MFLUX_BACKEND_CONFIGS


def uses_standard_sdnq_backend(backend: str) -> bool:
    return backend == "sdnq-hs" and IMAGE_DEVICE != "mps"


def backend_model(backend: str) -> str:
    if uses_standard_sdnq_backend(backend):
        return STANDARD_SDNQ_MODEL
    return BACKENDS[backend]


def normalize_backend(backend: Any) -> tuple[str, list[str]]:
    selected = str(backend or DEFAULT_BACKEND)
    warnings: list[str] = []
    if selected not in BACKENDS:
        raise ValueError(f"Unsupported backend: {selected}")
    if is_mflux_backend(selected) and RUNTIME_PLATFORM != "darwin":
        warnings.append("MFLUX/MLX is Apple Silicon only; using PyTorch SDNQ instead.")
        selected = "sdnq-hs"
    if uses_standard_sdnq_backend(selected):
        warnings.append("Using standard FLUX.2 SDNQ because the HS path is MPS-only.")
    return selected, warnings


def popen_kwargs() -> dict[str, Any]:
    if os.name == "nt":
        return {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
    return {"start_new_session": True}


def terminate_process(proc: subprocess.Popen[str]) -> None:
    if proc.poll() is not None:
        return
    if os.name == "nt":
        proc.terminate()
        return
    os.killpg(proc.pid, signal.SIGTERM)


def kill_process(proc: subprocess.Popen[str]) -> None:
    if proc.poll() is not None:
        return
    if os.name == "nt":
        proc.kill()
        return
    os.killpg(proc.pid, signal.SIGKILL)


def hf_token_from_repo_env() -> str | None:
    """Read the HF token ultra-fast-image-gen's UI persists to its .env.

    The uncensored GGUF text encoder is downloaded from a gated Hugging Face
    repo; the resident worker doesn't load dotenv, so pass the token along.
    """
    env_path = ULTRA_REPO / ".env"
    if not env_path.exists():
        return None
    try:
        for line in env_path.read_text().splitlines():
            key, sep, value = line.strip().partition("=")
            if sep and key in ("HF_TOKEN", "HUGGING_FACE_HUB_TOKEN") and value:
                return value
    except OSError:
        return None
    return None


def mflux_env(steps: int, backend: str = "mflux-hs") -> dict[str, str]:
    config = MFLUX_BACKEND_CONFIGS.get(backend, MFLUX_BACKEND_CONFIGS["mflux-hs"])
    gguf_variant = config["ggufVariant"]
    env = os.environ.copy()
    hf_token = env.get("HF_TOKEN") or env.get("HUGGING_FACE_HUB_TOKEN") or hf_token_from_repo_env()
    if hf_token:
        env["HF_TOKEN"] = hf_token
    env.update(
        {
            "MFLUX_DISABLE_COMPILE": "1",
            "MFLUX_UNCENSORED_GGUF_TE": "1",
            "MFLUX_SKIP_STOCK_TEXT_ENCODER": "1",
            "MFLUX_UNCENSORED_GGUF_VARIANT": gguf_variant,
            "MFLUX_UNCENSORED_GGUF_QUANT": "q4_k_m",
            "MFLUX_UNCENSORED_GGUF_DEVICE": "mps",
            "MFLUX_UNCENSORED_GGUF_REPO_ROOT": str(ULTRA_REPO),
            "MFLUX_HS_STRIDE": "2",
            "MFLUX_HS_SKIP_TRANSFORMER_FORWARDS": "0",
            "MFLUX_HS_MAX_TRANSFORMER_FORWARD": str(max(0, steps - 1)),
            "MFLUX_HS_SINGLE_START_FRAC": "0.0",
            "MFLUX_HS_SINGLE_END_FRAC": "1.0",
        }
    )
    return env


def backend_command(
    *,
    backend: str,
    prompt: str,
    dimensions: Dimensions,
    steps: int,
    seed: int,
    guidance: float,
    output_path: Path,
    timeout: int,
    reference_paths: list[Path],
) -> tuple[list[str], dict[str, str]]:
    model = backend_model(backend)
    hs_stride = 1 if is_mflux_backend(backend) and reference_paths else 2
    hs_max_transformer_forward = 0 if is_mflux_backend(backend) and reference_paths else max(0, steps - 1)
    cmd = [
        str(PYTHON),
        str(GENERATE),
        model,
        prompt,
        "--width",
        str(dimensions.width),
        "--height",
        str(dimensions.height),
        "--steps",
        str(steps),
        "--seed",
        str(seed),
        "--guidance",
        str(guidance),
        "--output",
        str(output_path),
    ]

    env = os.environ.copy()
    env.setdefault("PYTORCH_MPS_FAST_MATH", "1")

    if is_mflux_backend(backend):
        cmd.extend(
            [
                "--gguf-quant",
                "q4_k_m",
                "--hs-stride",
                str(hs_stride),
                "--hs-skip-transformer-forwards",
                "0",
                "--hs-max-transformer-forward",
                str(hs_max_transformer_forward),
                "--hs-single-start-frac",
                "0.0",
                "--hs-single-end-frac",
                "1.0",
                "--mflux-dir",
                str(MFLUX_DIR),
                "--timeout",
                str(timeout),
                "--gguf-device",
                "mps",
                "--mflux-model",
                MFLUX_BACKEND_CONFIGS[backend]["model"],
                "--gguf-variant",
                MFLUX_BACKEND_CONFIGS[backend]["ggufVariant"],
            ]
        )
        env = mflux_env(steps, backend)
    elif uses_standard_sdnq_backend(backend) or backend == "flux-uncensored":
        cmd.extend(["--device", IMAGE_DEVICE])
    else:
        cmd.extend(["--device", IMAGE_DEVICE, "--qchunk", "1024"])
        cmd.extend(
            [
                "--gguf-quant",
                "q4_k_m",
                "--hs-stride",
                str(hs_stride),
                "--hs-skip-transformer-forwards",
                "0",
                "--hs-max-transformer-forward",
                str(hs_max_transformer_forward),
                "--hs-single-start-frac",
                "0.0",
                "--hs-single-end-frac",
                "1.0",
            ]
        )

    if reference_paths:
        cmd.extend(["--input-images", *[str(path) for path in reference_paths]])

    return cmd, env


def run_mflux_resident(
    *,
    prompt: str,
    dimensions: Dimensions,
    steps: int,
    seed: int,
    output_path: Path,
    timeout: int,
    reference_paths: list[Path],
    backend: str,
) -> dict[str, Any]:
    config = MFLUX_BACKEND_CONFIGS[backend]
    response = MFLUX_RESIDENT.request(
        {
            "action": "generate",
            "prompt": prompt,
            "width": dimensions.width,
            "height": dimensions.height,
            "steps": steps,
            "seed": seed,
            "mfluxGuidance": 1.0,
            "output_path": str(output_path),
            "image_paths": [str(path) for path in reference_paths[:2]],
            "mflux_model": config["model"],
            "gguf_variant": config["ggufVariant"],
        },
        timeout=timeout,
    )
    if not output_path.exists():
        raise RuntimeError(f"MFLUX resident completed but did not write {output_path}")

    STATUS["residentBackends"] = [backend]
    STATUS["mfluxResident"] = {
        "pid": MFLUX_RESIDENT.proc.pid if MFLUX_RESIDENT.proc else None,
        "loadSeconds": response.get("loadSeconds"),
        "generations": response.get("generations"),
        "maxRssGb": response.get("maxRssGb"),
        "generationKind": response.get("generationKind"),
        "mfluxModel": response.get("mfluxModel"),
        "ggufVariant": response.get("ggufVariant"),
        "activeModels": response.get("activeModels"),
        "singleModel": response.get("singleModel"),
    }
    return response


TEXT_SERVER_URL = os.environ.get("OD_TEXT_SERVER_URL", "http://127.0.0.1:8080").rstrip("/")
TEXT_SERVER_UNLOAD = os.environ.get("OD_TEXT_SERVER_UNLOAD", "1") not in ("0", "false", "False", "")


def free_text_server_vram() -> None:
    """Best-effort: tell the text LLM server to unload so the image backend gets
    the whole GPU. Only one model is ever needed at a time; the text server
    reloads lazily on its next request. Ignored if the server is down or old."""
    if not TEXT_SERVER_UNLOAD:
        return
    try:
        req = urllib.request.Request(f"{TEXT_SERVER_URL}/unload", data=b"", method="POST")
        # /unload waits on the text server's generation lock, so this call blocks
        # until any in-flight LLM turn finishes AND its VRAM is freed. Give it a long
        # window (a slow turn can take minutes) — loading FLUX before the LLM frees
        # VRAM is exactly what wedges the shared GPU, so we must not time out early.
        urllib.request.urlopen(req, timeout=180).read()
    except Exception:
        pass


def run_generation(payload: dict[str, Any]) -> dict[str, Any]:
    backend, backend_warnings = normalize_backend(payload.get("backend"))

    prompt = str(payload.get("prompt") or "").strip()
    if not prompt:
        raise ValueError("Missing prompt.")

    if not PYTHON.exists():
        raise FileNotFoundError(f"Missing Python interpreter: {PYTHON}")
    if not GENERATE.exists():
        raise FileNotFoundError(f"Missing generate.py: {GENERATE}")
    if is_mflux_backend(backend) and not MFLUX_DIR.exists():
        raise FileNotFoundError(f"Missing patched MFLUX checkout: {MFLUX_DIR}")

    free_text_server_vram()  # only one model on the GPU at a time — drop the LLM first

    standard_sdnq = uses_standard_sdnq_backend(backend)
    full_step = standard_sdnq or backend == "flux-uncensored"
    dimensions = resolve_dimensions(payload)
    steps = clamp_int(
        payload.get("steps"),
        12 if full_step else 4,
        1,
        32 if full_step else 8,
    )
    if full_step and steps <= 4:
        steps = 12
    guidance = float(payload.get("guidance", 3.5 if full_step else 0.0) or 0.0)
    if full_step and guidance == 0.0:
        guidance = 3.5
    seed = clamp_int(payload.get("seed"), random.randint(1, 2**31 - 1), 1, 2**32 - 1)
    timeout = clamp_int(payload.get("timeout"), DEFAULT_TIMEOUT, 30, 1200)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    image_id = f"{int(time.time())}-{seed}-{backend}-{slug(prompt)}"
    output_path = OUT_DIR / f"{image_id}.png"
    reference_paths, reference_warnings = prepare_reference_paths(
        payload.get("references") or [],
        image_id,
    )
    start = time.time()
    log = ""
    resident = False
    resident_meta: dict[str, Any] = {}
    if is_mflux_backend(backend) and MFLUX_RESIDENT_ENABLED:
        try:
            resident_response = run_mflux_resident(
                prompt=prompt,
                dimensions=dimensions,
                steps=steps,
                seed=seed,
                output_path=output_path,
                timeout=timeout,
                reference_paths=reference_paths,
                backend=backend,
            )
            elapsed = time.time() - start
            resident = True
            resident_meta = {
                "loadSeconds": resident_response.get("loadSeconds"),
                "generationTime": resident_response.get("generationTime"),
                "generations": resident_response.get("generations"),
                "maxRssGb": resident_response.get("maxRssGb"),
                "pid": MFLUX_RESIDENT.proc.pid if MFLUX_RESIDENT.proc else None,
                "generationKind": resident_response.get("generationKind"),
                "mfluxModel": resident_response.get("mfluxModel"),
                "ggufVariant": resident_response.get("ggufVariant"),
                "referenceCount": resident_response.get("referenceCount"),
                "activeModels": resident_response.get("activeModels"),
                "singleModel": resident_response.get("singleModel"),
                "hsStride": resident_response.get("hsStride"),
                "hsMaxTransformerForward": resident_response.get("hsMaxTransformerForward"),
            }
        except Exception as error:
            log = f"Resident MFLUX failed, falling back to CLI:\n{error}\n"

    if not resident:
        cmd, env = backend_command(
            backend=backend,
            prompt=prompt,
            dimensions=dimensions,
            steps=steps,
            seed=seed,
            guidance=guidance,
            output_path=output_path,
            timeout=timeout,
            reference_paths=reference_paths,
        )
        proc = subprocess.Popen(
            cmd,
            cwd=str(ULTRA_REPO),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env=env,
            **popen_kwargs(),
        )
        try:
            cli_log, _ = proc.communicate(timeout=timeout)
            log += cli_log
        except subprocess.TimeoutExpired:
            terminate_process(proc)
            try:
                cli_log, _ = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                kill_process(proc)
                cli_log, _ = proc.communicate()
            log += cli_log
            raise TimeoutError(f"{backend} timed out after {timeout}s\n{log}")

        elapsed = time.time() - start
        if proc.returncode != 0:
            raise RuntimeError(f"{backend} failed with code {proc.returncode}\n{log}")
        if not output_path.exists():
            raise RuntimeError(f"{backend} completed but did not write {output_path}\n{log}")

    warnings = [*backend_warnings, *reference_warnings]
    if payload.get("references") and not reference_paths:
        warnings.append("No usable local reference images were provided.")

    return {
        "id": image_id,
        "url": f"/generated/{output_path.name}",
        "prompt": prompt,
        "mode": "slow" if payload.get("mode") == "slow" else "fast",
        "backend": backend,
        "aspect": dimensions.aspect,
        "width": dimensions.width,
        "height": dimensions.height,
        "steps": steps,
        "guidance": guidance,
        "elapsedSeconds": round(elapsed, 2),
        "seed": seed,
        "resident": resident,
        "residentMeta": resident_meta,
        "warnings": warnings,
        "logTail": log[-4000:],
    }


class Handler(BaseHTTPRequestHandler):
    server_version = "LocalRoleplayOptimizedImageServer/1.0"

    def do_GET(self) -> None:
        path = urlparse(self.path).path
        if path == "/health":
            json_response(
                self,
                200,
                {
                    "ok": PYTHON.exists() and GENERATE.exists(),
                    "loaded": bool(STATUS["residentBackends"]),
                    "residentBackends": STATUS["residentBackends"],
                    "mfluxResident": STATUS["mfluxResident"],
                    "warmed": STATUS["lastWarm"],
                    "repo": str(ULTRA_REPO),
                    "python": str(PYTHON),
                    "platform": RUNTIME_PLATFORM,
                    "device": IMAGE_DEVICE,
                    "defaultBackend": DEFAULT_BACKEND,
                    "sdnqModel": backend_model("sdnq-hs"),
                    "mfluxDir": str(MFLUX_DIR),
                    "backends": {
                        "mflux-hs": RUNTIME_PLATFORM == "darwin" and MFLUX_DIR.exists(),
                        "sdnq-hs": PYTHON.exists() and GENERATE.exists(),
                    },
                },
            )
            return

        if path == "/backends":
            json_response(
                self,
                200,
                {
                    "backends": [
                        {
                            "id": "mflux-hs",
                            "label": MFLUX_BACKEND_CONFIGS["mflux-hs"]["label"],
                            "model": BACKENDS["mflux-hs"],
                            "referenceLimit": 2,
                        },
                        {
                            "id": "sdnq-hs",
                            "label": "PyTorch SDNQ",
                            "model": backend_model("sdnq-hs"),
                            "referenceLimit": 2,
                        },
                    ],
                    "aspects": ["square", "portrait", "landscape"],
                    "defaults": {"longSide": 1024, "steps": 4, "guidance": 0.0},
                    "defaultBackend": DEFAULT_BACKEND,
                    "device": IMAGE_DEVICE,
                    "sizes": [
                        {"mode": "fast", "longSide": 1024},
                        {"mode": "slow", "longSide": 2048},
                    ],
                    "warmNote": "MFLUX warm starts a resident worker on Apple Silicon; Windows/Linux use the PyTorch SDNQ backend.",
                },
            )
            return

        json_response(self, 404, {"error": "Not found."})

    def do_POST(self) -> None:
        path = urlparse(self.path).path
        if path not in ("/generate", "/warm"):
            json_response(self, 404, {"error": "Not found."})
            return

        try:
            payload = read_json(self)
            if path == "/warm":
                payload = {
                    "backend": payload.get("backend") or "mflux-hs",
                    "prompt": payload.get("prompt")
                    or "warmup image, simple portrait lighting, detailed face",
                    "width": int(payload.get("width") or 512),
                    "height": int(payload.get("height") or 512),
                    "steps": int(payload.get("steps") or 4),
                    "guidance": 0.0,
                    "seed": int(payload.get("seed") or 1234),
                    "timeout": int(payload.get("timeout") or DEFAULT_TIMEOUT),
                    "mode": "fast",
                    "aspect": payload.get("aspect") or "square",
                }
            with RUNTIME_LOCK:
                result = run_generation(payload)
            if path == "/warm":
                STATUS["lastWarm"] = {
                    "backend": result["backend"],
                    "elapsedSeconds": result["elapsedSeconds"],
                    "width": result["width"],
                    "height": result["height"],
                    "seed": result["seed"],
                    "resident": result.get("resident", False),
                    "residentMeta": result.get("residentMeta", {}),
                }
                result["warmNote"] = "MFLUX is resident when resident=true."
            else:
                STATUS["lastGenerate"] = {
                    "backend": result["backend"],
                    "elapsedSeconds": result["elapsedSeconds"],
                    "width": result["width"],
                    "height": result["height"],
                    "seed": result["seed"],
                }
            json_response(self, 200, result)
        except Exception as error:
            json_response(
                self,
                500,
                {
                    "error": "Optimized image generation failed.",
                    "detail": str(error)[-4000:],
                },
            )

    def log_message(self, fmt: str, *args: Any) -> None:
        print(f"[image-server] {self.address_string()} - {fmt % args}", flush=True)


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    print(
        f"Optimized image server on http://{HOST}:{PORT} "
        f"(repo={ULTRA_REPO}, mflux={MFLUX_DIR})",
        flush=True,
    )
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    server.serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
