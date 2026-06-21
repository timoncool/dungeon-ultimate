<div align="center">

# Dungeon Ultimate

**An offline AI dungeon master with real 3D dice, full D&D mechanics, uncensored on-device image generation and voice input — your adventures never leave your machine.**

[![License](https://img.shields.io/github/license/timoncool/dungeon-ultimate?style=flat-square)](LICENSE)
[![Stars](https://img.shields.io/github/stars/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/stargazers)
[![Forks](https://img.shields.io/github/forks/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/network/members)
[![Last Commit](https://img.shields.io/github/last-commit/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/commits)
[![Issues](https://img.shields.io/github/issues/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/issues)
[![Made with Next.js](https://img.shields.io/badge/Next.js-16-black?style=flat-square&logo=next.js)](https://nextjs.org/)
[![Local & Private](https://img.shields.io/badge/100%25-on--device-7b5cff?style=flat-square)](#why-its-different)

**[English](README.md)** · **[Русский](README_RU.md)**

![Dungeon Ultimate](./docs/hero.png)

</div>

## Overview

**Dungeon Ultimate** is a fully on-device AI roleplay engine — a tireless dungeon master that writes your story, runs real tabletop rules, rolls physics dice, illustrates the scene and reads it aloud. Everything happens on your own NVIDIA GPU: no cloud, no API keys, no accounts, no content filters, and nothing ever leaves your PC.

It is a heavily extended fork of [open-dungeon](https://github.com/newideas99/open-dungeon), rebuilt around a local text model, an uncensored local FLUX image pipeline, a D&D-style game engine with **real 3D physics dice**, and on-device voice input. Run the launchers and play at `http://localhost:3000`.

## Why it's different

Most "AI dungeon" apps are a thin wrapper around someone else's cloud LLM — your prompts get logged, the model is censored, and there are no actual game mechanics behind the prose. Dungeon Ultimate flips all of that:

- **It runs on your hardware.** The story model, the image model and the speech model all load on your GPU. Pull the network cable and it still works.
- **There are real rules.** A deterministic D&D 5e engine resolves checks, combat and damage with a server-side CSPRNG — the narrator declares the action, the engine decides the outcome, so the AI can't cheat.
- **The dice are real.** A genuine 3D physics die tumbles across the scene (three.js + cannon-es) and is forced to land on exactly the number the engine already rolled.
- **It's uncensored.** A local, unfiltered text model plus a local FLUX image pipeline — with an optional abliterated text encoder for the images — mean unrestricted, adult storytelling and art, entirely your call and entirely private.

## Features

### Real 3D physics dice
- A genuine d20 built on [`@3d-dice/dice-box-threejs`](https://github.com/3d-dice/dice-box-threejs) (three.js + cannon-es) tumbles across the scene with real physics.
- Rolls are **honest** — the deterministic engine rolls first with a Node `crypto` CSPRNG, then the on-screen die is pinned (`1d20@N`) to land on that exact value. No fudging, no re-rolls.
- The settled die is colour-tinted by outcome (gold crit, red fumble) and logged to the adventure journal.

### D&D game mode
- **Character sheet** — six D&D 5e ability scores (STR / DEX / CON / INT / WIS / CHA), AC, level, XP and conditions.
- **d20 ability checks** — the narrator declares a check (ability + DC); the engine rolls `d20 + modifier`, with natural 20 always a crit success and natural 1 always a fumble.
- **HP & death** — characters track current/max HP and flip to a `dead` state when they hit zero.
- **Turn-based combat** — the narrator can spawn enemies, resolve attack rolls against AC and apply damage; foes are tracked per-encounter.
- **Adventure journal** — every roll, hit, drop and death is appended to a player-facing log that doubles as the engine's audit trail.
- **Loot drops** — enemies and chests grant inventory items with slots, rarity tiers and stat modifiers; each drop can be illustrated and the portrait reused via image2image.
- Game mode is per-chat and **on by default** — keep it on for a full RPG session, or toggle it off for freeform narrative play.

### Uncensored on-device image generation
- Scenes are illustrated **locally** by a quantized **FLUX.2-klein-4B** pipeline — no cloud, no key, no filter.
- The narrator calls an image tool mid-story and the picture renders right inside the passage.
- Runs out of the box on the ungated FLUX.2-klein SDNQ weights (no token, no filter); an optional **abliterated text encoder** (opt-in, gated) drops the last content guard for fully unrestricted 18+ art.
- Ken-Burns animation on rendered images plus one-click retry.

### Voice input
- Speak your action instead of typing — a mic button captures audio and transcribes it on-device.
- Powered by **NVIDIA Parakeet-TDT-0.6B-v3** ASR (via `onnx-asr` + ONNX Runtime GPU), running locally with no upload.

### And the rest
- **Live token streaming** — the narrator's prose streams into the chat word by word.
- **Voice narration (TTS)** — turns can be read aloud by a local text-to-speech server.
- **One model on the GPU at a time** — the text LLM unloads while images render and reloads on the next turn, so each gets the whole GPU.
- **Editable prompts & per-chat settings** — narrator prompt, image prompt, world, style, characters, response length, voice.
- **7 play languages** — the narrator, action chips, suggestions and TTS all follow your chosen language (Russian, English, Spanish, French, German, Chinese, Japanese), switchable in-app. The UI chrome is Russian; image prompts stay English for FLUX.
- **Portable Windows launchers** — `install.bat` / `run.bat` / `stop.bat`; models, runtimes and caches stay on a non-system drive.

## Requirements

- **OS:** Windows 10/11 (`install.bat` + `run.bat` set up a fully portable, self-contained install)
- **GPU:** NVIDIA, 12+ GB VRAM (RTX 40xx/50xx fully supported; 20xx/30xx/Pascal selectable in the installer). The installer pins matching CUDA wheels (cu126 / cu128) per GPU.
- **Node.js:** bundled — `install.bat` downloads a portable Node 22 runtime into the project folder
- **Python:** bundled — `install.bat` creates two embedded Python 3.11 environments (text/TTS and image)
- **Disk:** ~30+ GB for the embedded runtimes plus model weights
- **Model weights — all auto-downloaded, nothing to provide by hand.** `install.bat` clones the two backend checkouts (TTS, image) and downloads a reference voice pack into `servers/voices/`; on first launch every model pulls itself from Hugging Face — the Gemma 4 12B GGUFs (text), the FLUX.2-klein SDNQ image weights, the Qwen3-TTS voice model, and the Parakeet ASR model. All of those are ungated. The fully-abliterated FLUX text encoder lives in a **gated** HF repo, so it stays opt-in: point `IMAGE_SERVER_DEFAULT_BACKEND=flux-uncensored` at it once you have repo access + an HF token. You can also drop your own `.mp3` reference clips into `servers/voices/`.

> The app is engineered to stay self-contained: temp files, Hugging Face caches, Torch caches and model stores are all redirected onto the project drive — nothing is written to `C:` or the registry.

## Quick start

1. **Clone**
   ```bash
   git clone https://github.com/timoncool/dungeon-ultimate.git
   cd dungeon-ultimate
   ```

2. **Install** — run `install.bat`, pick your GPU, and let it download the portable Node + Python runtimes, install dependencies and build the web app.
   ```
   install.bat
   ```

3. **Run** — start the text, image, TTS and web servers together.
   ```
   run.bat
   ```
   Your browser opens at `http://localhost:3000`. Stop everything with `stop.bat`.

## How to play

- Create a chat, set the world/style or pick a character, then type **or speak** an action — the narrator streams a story turn.
- Turn on **game mode** for a full D&D session: ability checks roll a real 3D die, combat resolves against AC, HP and loot are tracked, and every result lands in the adventure journal.
- Hit the **mic** button to dictate your action; it's transcribed locally by Parakeet.
- Toggle **narration** to have turns read aloud.
- Edit the narrator / image prompts in the side panels to retune tone and art direction.

## Other projects by [timoncool](https://github.com/timoncool)

| Project | Description |
|---------|-------------|
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

The 3D dice are powered by [@3d-dice/dice-box-threejs](https://github.com/3d-dice/dice-box-threejs). Speech recognition uses NVIDIA's [Parakeet-TDT-0.6B](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) via [onnx-asr](https://github.com/istupakov/onnx-asr). Imagery is produced with [FLUX.2](https://github.com/black-forest-labs/flux).

## Support the author

I build open-source software and do AI research. Most of what I create is free and available to everyone. Your donations help me keep creating without worrying about where the next meal comes from =)

**[All donation methods](https://github.com/timoncool/ACE-Step-Studio/blob/master/DONATE.md)** | **[dalink.to/nerual_dreming](https://dalink.to/nerual_dreming)** | **[boosty.to/neuro_art](https://boosty.to/neuro_art)**

- **BTC:** `1E7dHL22RpyhJGVpcvKdbyZgksSYkYeEBC`
- **ETH (ERC20):** `0xb5db65adf478983186d4897ba92fe2c25c594a0c`
- **USDT (TRC20):** `TQST9Lp2TjK6FiVkn4fwfGUee7NmkxEE7C`

## Star History

<a href="https://www.star-history.com/?repos=timoncool%2Fdungeon-ultimate&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/image?repos=timoncool/dungeon-ultimate&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/image?repos=timoncool/dungeon-ultimate&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/image?repos=timoncool/dungeon-ultimate&type=date&legend=top-left" />
 </picture>
</a>

## License

[MIT](LICENSE) — same as the upstream project. Do whatever you want; attribution appreciated.
