# Optimized Image Server

This folder is the Open Dungeon wrapper around the optimized uncensored
FLUX.2-klein backends from the `ultra-fast-image-gen` repo (expected at
`~/ultra-fast-image-gen` on macOS/Linux or
`%USERPROFILE%\ultra-fast-image-gen` on Windows; override with
`ULTRA_FAST_IMAGE_GEN_DIR`).

It does not reimplement the optimization logic. Requests are delegated through
`generate.py` to:

- `flux2-4b-uncensored-mflux-hs` (Apple Silicon / MLX)
- `flux2-4b-uncensored-sdnq-hs` (PyTorch SDNQ on MPS)
- `flux2-4b-sdnq` (PyTorch SDNQ on CUDA or CPU)

Defaults are 1024x1024, 4 steps, guidance 0.0, and square aspect. The backend
defaults to MFLUX HS on macOS and SDNQ HS on Windows/Linux. Portrait uses
768x1024 and landscape uses 1024x768. The slow size is 2048 square, 1536x2048
portrait, or 2048x1536 landscape.

Reference images are limited to two per request. They can be any aspect ratio;
the local wrappers fit them inside the requested output canvas with padding
instead of stretching them. MFLUX resident mode is used for text-only requests;
reference requests run through the reference-capable CLI path.

MFLUX runs in resident mode by default. The HTTP server starts a long-lived
worker process under the patched MFLUX checkout (default
`~/.cache/ultra-fast-image-gen/mflux`, created by
`ultra-fast-image-gen/scripts/setup_mflux_hs.sh`; override with `MFLUX_DIR`),
loads `Flux2Klein` once, keeps the uncensored
GGUF text encoder alive after first prompt encoding, and sends later generations
over JSON-lines IPC. On non-macOS platforms, MFLUX requests are automatically
mapped to `sdnq-hs` because MLX is Apple-only.

Device selection for `sdnq-hs` is controlled by `IMAGE_SERVER_DEVICE`. On MPS,
the worker uses the uncensored HS backend. On CUDA/CPU, it uses the standard
`flux2-4b-sdnq` backend because the HS attention patch imports the MPS-specific
chunked-attention module.

- `auto` (default): `mps` on macOS, `cuda` elsewhere; `generate.py` falls back
  to CPU when CUDA is unavailable.
- `cuda`: NVIDIA GPU via PyTorch CUDA wheels.
- `cpu`: CPU-only PyTorch wheels.
- `mps`: Apple Silicon / PyTorch MPS.

The default backend is `mflux-hs` on macOS and `sdnq-hs` elsewhere. Override
with `IMAGE_SERVER_DEFAULT_BACKEND`.

Run:

```bash
npm run image:server
```

On Windows, use the repo-level `Launch-Windows.bat`; it creates the backend
venv, installs CUDA or CPU PyTorch wheels, updates a clean
`ultra-fast-image-gen` checkout, and starts this worker in a second PowerShell
window. Use `Launch-Windows-CPU.bat` to force the CPU wheel path.

Check backend routing without loading any model:

```bash
npm run check:image-routing
npm run check:image-server-http
```

Health:

```bash
curl http://127.0.0.1:7869/health
```

Warm the default backend with a 512 smoke generation:

```bash
curl -X POST http://127.0.0.1:7869/warm \
  -H 'Content-Type: application/json' \
  -d '{"backend":"mflux-hs"}'
```

Health reports the resident PID and generation count:

```bash
curl http://127.0.0.1:7869/health
```
