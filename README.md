<div align="center">

# Dungeon Ultimate

**An offline AI dungeon master with real 3D dice, full D&D mechanics, uncensored on-device image generation and voice input — your adventures never leave your machine.**

[![License](https://img.shields.io/github/license/timoncool/dungeon-ultimate?style=flat-square)](LICENSE)
[![Stars](https://img.shields.io/github/stars/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/stargazers)
[![Forks](https://img.shields.io/github/forks/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/network/members)
[![Last Commit](https://img.shields.io/github/last-commit/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/commits)
[![Issues](https://img.shields.io/github/issues/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/issues)
[![Made with Rust](https://img.shields.io/badge/Rust-one%20binary-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Local & Private](https://img.shields.io/badge/100%25-on--device-7b5cff?style=flat-square)](#why-its-different)

**[English](README.md)** · **[Русский](README_RU.md)**

![Dungeon Ultimate](./docs/hero.png)

</div>

## Overview

**Dungeon Ultimate** is a fully on-device AI roleplay engine — a tireless dungeon master that writes your story, runs real tabletop rules, rolls physics dice, illustrates the scene and reads it aloud. Everything can happen on your own NVIDIA GPU: no accounts, no content filters, and nothing leaves your PC. A cloud mode is there if you want it — every stage of a turn can run through OpenRouter instead, so a machine without a GPU plays the same game.

It is a heavily extended fork of [open-dungeon](https://github.com/newideas99/open-dungeon), rebuilt around a local text model, an uncensored local image pipeline, a D&D-style game engine with **real 3D physics dice**, and on-device voice input.

**As of v0.2 the whole app is a single Rust binary.** The previous build needed Next.js plus four Python servers; now one executable serves the UI, runs the turn pipeline and drives every engine through native libraries. No Python, no Node at runtime, nothing installed into the system — delete the folder and the app is gone.

## Why it's different

- **It runs on your hardware.** The story model, the image model and the speech model all load on your GPU. Pull the network cable and it still works.
- **There are real rules.** A deterministic D&D 5e engine resolves checks, combat and damage with a CSPRNG — the narrator declares the action, the engine decides the outcome, so the AI can't cheat.
- **The dice are real.** A genuine 3D physics die tumbles across the scene (three.js + cannon-es) and is forced to land on exactly the number the engine already rolled.
- **It's uncensored.** A local, unfiltered text model plus a local image pipeline with an abliterated text encoder mean unrestricted, adult storytelling and art — entirely your call and entirely private.
- **It also runs with no GPU at all.** Every stage of a turn — narration, illustration, narration audio, speech input — can be switched to the cloud independently, so a laptop can play the same game.

## Features

### Real 3D physics dice
- A genuine d20 built on [`@3d-dice/dice-box-threejs`](https://github.com/3d-dice/dice-box-threejs) (three.js + cannon-es) tumbles across the scene with real physics.
- Rolls are **honest** — the deterministic engine rolls first with a CSPRNG, then the on-screen die is pinned (`1d20@N`) to land on that exact value. No fudging, no re-rolls.
- The settled die is colour-tinted by outcome (gold crit, red fumble) and logged to the adventure journal.

### D&D game mode
- **Character sheet** — six D&D 5e ability scores (STR / DEX / CON / INT / WIS / CHA), AC, level, XP and conditions.
- **d20 ability checks** — the narrator declares a check (ability + DC); the engine rolls `d20 + modifier`, with natural 20 always a crit success and natural 1 always a fumble.
- **HP & death** — characters track current/max HP and flip to a `dead` state at zero; healing above zero brings them back.
- **Turn-based combat** — the narrator spawns enemies, attack rolls resolve against AC, crits double the dice (not the flat modifier), and foes are tracked per-encounter.
- **Adventure journal** — every roll, hit, drop and death is appended to a player-facing log that doubles as the engine's audit trail, **in the language you play in**.
- **Loot drops** — enemies and chests grant inventory items with slots, rarity tiers and stat modifiers; each drop can be illustrated and the portrait reused as an image reference.
- **Random events** — blessings and curses can strike between turns, with real stat modifiers on a timer.
- Game mode is per-chat and **on by default**.

### Uncensored on-device image generation
- Scenes are illustrated **locally** by **Krea-2 Turbo** (GGUF Q4_K_M) running on [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) — one universal model that both generates and edits.
- The text encoder is an **abliterated Qwen3-VL-4B**, so there is no content guard between you and the art.
- **Scene continuity** — a location keeps an anchor frame; later shots are edits of it, so the same room stays the same room. After a few hops the scene re-establishes itself to stop drift.
- Hero and item portraits are generated too, and reused as references.
- Ken-Burns drift on rendered frames plus one-click retry.

### Voice
- **Narration** — turns are read aloud by **Higgs Audio v3** with voice cloning; a voice pack ships with the app and you can drop in your own reference clip.
- **Streaming speech** — sentences are synthesized as the narrator writes them, so audio starts within seconds instead of after the whole turn.
- **Per-character voices** — quoted dialogue is spoken in the voice of the character who said it.
- **Voice input** — speak your action; **NVIDIA Parakeet-TDT-0.6B-v3** transcribes it on-device.

### Cloud mode (optional)
- Each stage — narrator, frame, narration audio, speech input — flips between "on my GPU" and "in the cloud" independently, through [OpenRouter](https://openrouter.ai).
- Model lists are pulled live: 400+ text models, 40+ image models, 18 speech models, 19 transcription models. Voices are labelled by gender and by whether they actually speak Russian.
- With every stage in the cloud the game needs no GPU at all.

### And the rest
- **Live token streaming** — the narrator's prose streams into the chat word by word.
- **Book mode** — read the story as paginated spreads, with the illustrations in place.
- **One model on the GPU at a time** — a single queue arbitrates the card between text, image and speech, so each gets the whole GPU. Narration audio runs in parallel with the frame when VRAM allows.
- **Resource monitor** — a floating widget shows GPU load, VRAM, power and RAM while a turn runs.
- **Editable prompts & per-chat settings** — narrator prompt, image prompt, world, style, characters, response length, voices, KV-cache size, sampling steps and more.
- **7 play languages** — narration, action chips, suggestions, the adventure journal and speech all follow your chosen language (Russian, English, Spanish, French, German, Chinese, Japanese). Image prompts stay English, because that is what the image model reads.
- **Windows launchers** — `install.bat` / `run.bat` / `update.bat`; everything lives inside the app folder.

## Requirements

- **OS:** Windows 10/11.
- **GPU:** NVIDIA with 12+ GB VRAM for the full local experience (24 GB lets narration audio and the frame run in parallel). **No GPU is fine** if you play in cloud mode.
- **Build tools, once:** [Rust](https://rustup.rs) and [Node.js](https://nodejs.org) — `install.bat` checks for both and tells you what is missing.
- **Disk:** ~17.4 GB for the required models, ~23 GB with narration audio and speech input.
- **Model weights are downloaded by the app itself.** Nothing to fetch by hand, no tokens, no gated repos: the first launch opens the **"What to download"** panel with every component, its size and what it is for. Downloads resume where they stopped.

> Everything stays inside the app folder: models, saves, generated frames, temp files. Nothing is written to `C:` or the registry.

## Quick start

1. **Clone**
   ```bash
   git clone https://github.com/timoncool/dungeon-ultimate.git
   cd dungeon-ultimate
   ```

2. **Install** — builds the UI and the binary.
   ```
   install.bat
   ```

3. **Run**
   ```
   run.bat
   ```
   Your browser opens at `http://127.0.0.1:8770`. On the first launch, open **"What to download"** and let it fetch the models.

4. **Update later**
   ```
   update.bat
   ```
   Pulls the new version and rebuilds it. Models, saves and settings are untouched. The app also checks GitHub for new releases and tells you when one is out.

## How to play

<div align="center">
<table>
<tr>
<td width="50%" valign="top"><img src="./docs/shots/new-story-ru.png" alt="New story setup" /><br /><sub><b>Start a new story</b> — pick a genre, say who you are, and the narrator writes the opening scene.</sub></td>
<td width="50%" valign="top"><img src="./docs/shots/book-ru.png" alt="Narrative reading view" /><br /><sub><b>Pure storytelling</b> — the narrator's prose streams in word by word, book-style.</sub></td>
</tr>
</table>
</div>

- Create a story, pick a setting and say who you are — a character sheet and a hero portrait are generated for you.
- Type **or speak** an action; the narrator streams a turn, the engine resolves the mechanics, the die rolls, the frame renders and the passage is read aloud.
- Toggle **game mode** off for freeform narrative play.
- Switch any stage to the cloud in **"Engines and cloud"** if your card is busy — or if you have no card.

## Architecture

| Layer | What it is |
|---|---|
| Server | Rust + axum, one binary; serves the SPA same-origin and runs the turn pipeline |
| UI | React + Vite + Tailwind, prebuilt into the binary's static folder |
| Story engine | multi-pass turn: prose → mechanics → frame request → action chips, each pass schema-constrained |
| Rules | deterministic D&D 5e engine in its own crate, CSPRNG rolls, 200+ tests |
| Text | llama.cpp sidecar (Gemma 4 12B uncensored, Q4_K_M) |
| Images | stable-diffusion.cpp via FFI (Krea-2 Turbo Q4_K_M + abliterated Qwen3-VL encoder + Wan 2.1 VAE) |
| Speech out | Higgs Audio v3 through its native engine, streaming per sentence |
| Speech in | Parakeet-TDT-0.6B-v3 via ONNX Runtime |
| Cloud | OpenRouter, per-stage, with live model and voice catalogues |
| Storage | SQLite; saves from the previous version open unchanged |

## Other projects by [timoncool](https://github.com/timoncool)

| Project | Description |
|---------|-------------|
| [Dub Studio](https://github.com/timoncool/dub-studio) | AI dubbing studio — transcribe, translate, voice, mix |
| [ACE-Step Studio](https://github.com/timoncool/ACE-Step-Studio) | AI music studio — songs, vocals, covers, videos |
| [VideoSOS](https://github.com/timoncool/videosos) | AI video production in the browser |
| [Foundation Music Lab](https://github.com/timoncool/Foundation-Music-Lab) | Music generation + timeline editor |
| [Qwen3-TTS](https://github.com/timoncool/Qwen3-TTS_portable_rus) | Portable text-to-speech with voice cloning |
| [SuperCaption Qwen3-VL](https://github.com/timoncool/SuperCaption_Qwen3-VL) | Portable image captioning |
| [civitai-mcp-ultimate](https://github.com/timoncool/civitai-mcp-ultimate) | Civitai API as an MCP server |
| [ScreenSavy](https://github.com/timoncool/ScreenSavy.com) | Ambient screen generator |

## Authors

- **Nerual Dreming** — [Telegram](https://t.me/nerual_dreming) | [neuro-cartel.com](https://neuro-cartel.com) | [ArtGeneration.me](https://artgeneration.me)
- **Нейро-Софт** — [Telegram](https://t.me/neuroport) | portable neural-network apps

## Acknowledgements

Built on [**open-dungeon**](https://github.com/newideas99/open-dungeon) by [@newideas99](https://github.com/newideas99) — the original local AI roleplay app this fork extends. Huge thanks for the foundation.

Achievement art comes from [**game-icons.net**](https://github.com/game-icons/icons) — 4176 icons by Lorc, Delapouite, Skoll, Caro Asercion, Viscious Speed, sbed, Carl Olsen and other contributors, used under [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/) (see `frontend/public/game-icons/license.txt`). The idea of gamified, icon-backed achievements is borrowed from [prompt-warrior](https://github.com/timoncool/prompt-warrior).

The 3D dice are powered by [@3d-dice/dice-box-threejs](https://github.com/3d-dice/dice-box-threejs). Images run on [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) by [@leejet](https://github.com/leejet) with [Krea-2](https://huggingface.co/realrebelai/KREA-2_GGUFs) weights. Text runs on [llama.cpp](https://github.com/ggml-org/llama.cpp). Speech recognition uses NVIDIA's [Parakeet-TDT-0.6B](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3); narration uses [Higgs Audio v3](https://huggingface.co/drbaph/Higgs-Audio-v3-Studio).

## Support the author

I build open-source software and do AI research. Most of what I create is free and available to everyone. Your donations help me keep creating without worrying about where the next meal comes from =)

**[All donation methods](DONATE.md)** | **[dalink.to/nerual_dreming](https://dalink.to/nerual_dreming)** | **[boosty.to/neuro_art](https://boosty.to/neuro_art)**

- **BTC:** `1E7dHL22RpyhJGVpcvKdbyZgksSYkYeEBC`
- **ETH (ERC20):** `0xb5db65adf478983186d4897ba92fe2c25c594a0c`
- **USDT (TRC20):** `TQST9Lp2TjK6FiVkn4fwfGUee7NmkxEE7C`

## Star History

<a href="https://github.com/timoncool/dungeon-ultimate/stargazers">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="docs/stars-dark.svg" />
   <source media="(prefers-color-scheme: light)" srcset="docs/stars-light.svg" />
   <img alt="Star History Chart" src="docs/stars-light.svg" />
 </picture>
</a>

## License

[MIT](LICENSE) — same as the upstream project. Do whatever you want; attribution appreciated.
