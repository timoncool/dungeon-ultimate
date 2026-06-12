# Open Dungeon

The first **easy-to-use, fully local** AI roleplay app. The story and the
**inline scene images** are both generated on your own machine — no accounts,
no API keys, no cloud, no GPU rig. Your stories never leave your computer.

![A story scene with an inline generated image](docs/hero.png)

- **Local text generation** via [Ollama](https://ollama.com) using Google's
  Gemma 4 QAT (Q4_0) models, selectable per chat. OpenRouter is supported as an
  optional cloud alternative.
- **Local image generation**: the narrator can call a `generate_image` tool and
  scenes are rendered inline by FLUX.2-klein on Apple Silicon (optional).
- **Quick starts**: pick a setting, say who you are, and the narrator writes
  a custom opening scene.
- **Full play controls**: Do / Say / Story input modes, plus Continue, Retry,
  Erase, and inline Edit on any passage.
- **Long-story memory**: history fills the model's full context window
  (128K–256K tokens), and passages that eventually scroll out are compacted
  into a rolling "story so far" summary instead of being forgotten.
- **Characters with visual continuity**: save character portraits and the app
  passes them to both the narrator (vision context) and the image generator
  (reference images).
- **Private by design**: chats, characters, and images live in a local SQLite
  database and `public/` folders on your disk.
- **Play from your phone** over Tailscale.

<table>
  <tr>
    <td><img src="docs/prose.png" alt="Serif story prose" /></td>
    <td><img src="docs/modal.png" alt="New story dialog with setting presets" /></td>
  </tr>
</table>

## Requirements

- Node.js 20+
- [Ollama](https://ollama.com) for local text generation
- Text-only works on any platform Ollama supports. Inline image generation
  currently targets Apple Silicon (see [Image generation](#image-generation-optional)).

## Quick start

**Easiest (Mac, Apple Silicon):** grab the DMG from
[Releases](https://github.com/newideas99/open-dungeon/releases), drag
**Open Dungeon** to Applications, and open it (right-click → Open the first
time — it's unsigned). It walks you through everything: a bundled Node
runtime, the narrator model download, and first build. You just need
[Ollama](https://ollama.com/download) installed.

**From a clone:** double-click `Launch.command`, which does the same checks
and setup. Or by hand:

```bash
git clone https://github.com/newideas99/open-dungeon && cd open-dungeon
npm install

# pull a local model (7.2 GB — see the table below for other sizes)
ollama pull gemma4:12b-it-qat

npm run dev
```

Open http://localhost:3000 and start writing. The Text Model panel in the
sidebar picks the provider and model per chat.

## Local models

The app uses the Gemma 4 quantization-aware-trained (QAT, Q4_0) builds, which
keep close to full-precision quality at a fraction of the memory. Any of these
work — pull what fits your RAM:

```bash
ollama pull gemma4:e2b-it-qat       # 4.3 GB
ollama pull gemma4:e4b-it-qat       # 6.1 GB
ollama pull gemma4:12b-it-qat       # 7.2 GB (default)
ollama pull gemma4:26b-a4b-it-qat   # 16 GB
ollama pull gemma4:31b-it-qat       # 19 GB
```

Measured on an M2 Max (32 GB), real story prompts, ~300-token turns. RAM is
resident memory from `ollama ps` at the app's context settings (the E-models
stream per-layer embeddings, so they sit below their download size):

| Model | Disk | RAM | Context | Generation | Typical turn | Cold load |
|---|---|---|---|---|---|---|
| E2B | 4.3 GB | 4.5 GB | 128K | 56 tok/s | 3.4 s | ~7 s |
| E4B | 6.1 GB | 3.2 GB | 128K | 44 tok/s | 3.8 s | ~9 s |
| 12B | 7.2 GB | 7.7 GB | 256K | 21 tok/s | 11.5 s | ~9 s |
| 26B MoE | 16 GB | 15 GB | 256K | 48 tok/s | 4.7 s | ~30 s |

The app runs each model at its full native context window by default. Gemma 4
uses sliding-window attention for most layers, so the KV cache stays small —
the 12B measured ~7.6 GB of RAM even with 50K+ tokens of story in context.

## Playing

The composer has three input modes:

- **Do** — a player action (`> ...`); the narrator responds in second person.
- **Say** — dialogue; your text is wrapped as `> You say "..."`.
- **Story** — write narration directly into the story yourself.

Above the composer:

- **Continue** — let the narrator advance the scene with no action from you.
- **Retry** — discard the latest passage and regenerate it.
- **Erase** — remove the most recent exchange (your action and its response).

Hover any message and hit **Edit** to rewrite it in place — useful for fixing a
detail so the narrator stops repeating it. Edits and erasures are saved to the
local database.

## Long-story memory

Story history is packed into the context window with a token budget that stays
~10% under the limit. When a story finally outgrows the window, the oldest
passages are evicted in blocks of 16 — keeping the prompt prefix append-only so
Ollama's prompt cache makes each turn pay only for its new tokens — and the
evicted passages are folded into a rolling "story so far" summary (the same
handoff-summary approach Codex CLI uses for context compaction, adapted for
fiction). The narrator keeps plot threads, character details, debts, and
secrets even after the raw text has scrolled out of context.

Deep prefill is the one real cost of huge contexts (~160 tok/s at 50K depth on
the 12B), and it only bites when the cache goes cold on a very long story; set
`LOCAL_TEXT_CONTEXT` to cap the window if you'd rather bound that.

The 26B MoE is both the strongest writer and nearly the fastest (only ~4B
params active per token), but wants headroom: prefer 24 GB+ RAM, more if you
run local image generation alongside it.

Two implementation notes baked into the app:

- Gemma 4 is a hybrid reasoning model; the app disables its thinking channel
  (`think: false`) so the whole token budget goes to story text. Models that
  don't support the flag are retried without it.
- If a local model's chat template doesn't support function tools, the turn is
  retried without the image tool, so the story continues without auto images.

## OpenRouter (optional)

Switch a chat's provider to OpenRouter in the Text Model panel, and put your
key in `.env.local` or `.env.server`:

```bash
OPENROUTER_API_KEY=...
OPENROUTER_MODEL=google/gemini-3.5-flash
OPENROUTER_MAX_TOKENS=16384
```

## Image generation (optional)

Inline images are produced by a small HTTP worker that wraps the optimized
FLUX.2-klein backends from the `ultra-fast-image-gen` project (Apple
Silicon / MLX + PyTorch MPS). The app expects that repo at
`~/ultra-fast-image-gen` (override with `ULTRA_FAST_IMAGE_GEN_DIR`).

The uncensored text encoder is downloaded once from a gated Hugging Face repo:
accept the terms on the model page, then set your token in
`ultra-fast-image-gen`'s web UI (or add `HF_TOKEN=...` to that repo's `.env`).

Start the worker in a second terminal:

```bash
npm run image:server
```

Without the worker running, everything else still works — image requests show
a Generate button that succeeds once the worker is up.

Backends exposed in the app:

- MFLUX/MLX uncensored HS: `flux2-4b-uncensored-mflux-hs`
- PyTorch SDNQ uncensored HS: `flux2-4b-uncensored-sdnq-hs`

Defaults are 1024 long-side, 4 steps, guidance 0.0; the slow size is 2048
long-side. Square, portrait, and landscape aspects are exposed in the UI.
Reference images are capped at two per request. MFLUX runs resident by
default: the worker keeps the model loaded between generations. See
[image_server/README.md](image_server/README.md) for details.

## The story image tool

The narrator can call a `generate_image` function tool through the selected
text provider (local Ollama model or OpenRouter). The app turns that tool call
into a local FLUX request, using the current image settings for backend, mode,
and aspect ratio. Old or interrupted image requests show as
`Image tool requested` with a Generate button instead of pretending a job is
still running.

## Playing from your phone

Run the app on all interfaces:

```bash
npm run dev:tailscale
```

Add your phone-facing hostname/IP to `ALLOWED_DEV_ORIGINS` in `.env.local`
(comma-separated), then open `http://<your-machine>:3002` from the phone on
the same tailnet. The FLUX worker and Ollama can stay on `127.0.0.1` because
browser requests go through the Next.js server.

## Configuration

Copy `.env.example` to `.env.local` and adjust. Everything is optional; the
defaults run fully local. Highlights:

| Variable | Default | Purpose |
|---|---|---|
| `OLLAMA_BASE_URL` | `http://127.0.0.1:11434` | Local text server |
| `LOCAL_TEXT_MAX_TOKENS` | `4096` | Max tokens generated per local turn |
| `LOCAL_TEXT_CONTEXT` | model max | Cap on the local context window |
| `OPENROUTER_API_KEY` | — | Enables the OpenRouter provider |
| `FLUX_WORKER_URL` | `http://127.0.0.1:7869` | Image worker |
| `ULTRA_FAST_IMAGE_GEN_DIR` | `~/ultra-fast-image-gen` | FLUX backends repo |
| `SQLITE_DB_PATH` | `data/local-roleplay.sqlite` | Database location |

## Local data

Chats and messages are stored in SQLite at `data/local-roleplay.sqlite` by
default. Deleting a chat removes its messages through SQLite cascade deletes.

Uploaded images are stored under `public/uploads/`. Generated images are
stored under `public/generated/`, with temporary generation refs under
`public/generated/refs/`. The sidebar's Local Data clear button deletes all
local stories, messages, characters, uploaded photos, generated images, and
temporary refs, then vacuums the SQLite database.

## Content note

This app is built for private, local fiction. The default narrator prompt
permits consensual adult content between adults; everything is generated and
stored only on your machine. Edit the system prompt in
`src/lib/story-prompt.ts` if you want different defaults.

## License

MIT
