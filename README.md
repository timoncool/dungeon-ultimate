<div align="center">

# Dungeon Ultimate

**A local AI dungeon master with inline image generation, voice narration and an uncensored model — your stories never leave your machine.**

[![License](https://img.shields.io/github/license/timoncool/dungeon-ultimate?style=flat-square)](LICENSE)
[![Stars](https://img.shields.io/github/stars/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/stargazers)
[![Last Commit](https://img.shields.io/github/last-commit/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/commits)

**[English](README.md)** · **[Русский](README.portable.ru.md)**

![Dungeon Ultimate](docs/screenshots/hero.png)

</div>

## About

Dungeon Ultimate is a fully on-device AI roleplay app — a tireless dungeon master that writes interactive stories, illustrates the scenes and narrates them aloud, all on your own GPU with no cloud, no API keys and no content filters. It is a heavily extended fork of [open-dungeon](https://github.com/newideas99/open-dungeon), rebuilt around a local Gemma text server, an uncensored image pipeline, streaming narration and a Russian-first interface. Runs on Windows with an NVIDIA GPU — start the backends and play at `localhost:3000`.

## Features

- **100% local & private** — local Gemma text server + local FLUX image worker, no cloud, no keys, nothing leaves your PC
- **Live token streaming** — the narrator's prose streams into the chat word by word
- **Inline scene images** — the model calls a `generate_image` tool and FLUX renders the scene right inside the story
- **Voice narration (TTS)** — per-message ▶ Play, autoplay, a 39-voice pack, voice cloning from your own `.mp3`, volume & speed
- **Uncensored mode** — swap to an uncensored text model and an abliterated image text-encoder for unrestricted 18+ storytelling
- **In-chat model selector** — switch the text model in any chat at any moment
- **One model on the GPU at a time** — the text LLM unloads while images render and reloads on the next turn, so each gets the whole GPU
- **Editable prompts & per-chat settings** — narrator prompt, image prompt, world, style, characters, response length, voice
- **Russian-first UI** — the whole interface and all prompts are localized (image prompts stay English for FLUX)
- **Portable Windows launchers** — `run.bat` / `stop.bat`; models, runtimes and caches stay on a non-system drive

## System Requirements

- **OS:** Windows 11 (Linux/macOS supported via the upstream launchers)
- **GPU:** NVIDIA with 12+ GB VRAM (RTX 4090 recommended for the uncensored 12B text model + FLUX combo)
- **Runtimes:** Node.js 22+ and a Python 3.11 venv for the local text/image/TTS servers
- **Disk:** ~30 GB for the GGUF text model and FLUX image weights

## Quick Start

1. **Clone**
   ```bash
   git clone https://github.com/timoncool/dungeon-ultimate.git
   cd dungeon-ultimate
   ```

2. **Install**
   ```
   npm install
   ```

3. **Run**
   ```
   run.bat
   ```
   Then open `http://localhost:3000`.

## Usage

- Create a chat, set the world/style or pick a character, then type an action — the narrator streams a story turn.
- Toggle **Озвучка** to have turns read aloud; pick a voice or upload an `.mp3` to clone one.
- Use the model dropdown to switch between the standard and uncensored text model mid-story.
- Edit the narrator / image prompts in the side panels to retune tone and art direction.

## Configuration

Everything is optional — the app runs fully local with no keys. See [`.env.example`](.env.example). Key variables:

| Variable | Purpose |
|----------|---------|
| `OPENAI_COMPAT_BASE_URL` | Local text server (default `http://127.0.0.1:8080/v1`) |
| `OPENAI_COMPAT_MODEL` | Text model id (e.g. `gemma-4-12b-uncensored`) |
| `FLUX_WORKER_URL` | Local image worker (default `http://127.0.0.1:7869`) |
| `IMAGE_SERVER_DEFAULT_BACKEND` | Image backend (`flux-uncensored` for NSFW) |
| `TTS_WORKER_URL` | Local TTS server |

## Other Projects by [@timoncool](https://github.com/timoncool)

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
- **Нейро-Софт** — [Telegram](https://t.me/neuroport) | портативные нейросети

## Acknowledgements

Built on [**open-dungeon**](https://github.com/newideas99/open-dungeon) by [@newideas99](https://github.com/newideas99) — the original local AI roleplay app this fork extends. Huge thanks for the foundation.

## Support the Author

I build open-source software and do AI research. Most of what I create is free and available to everyone. Your donations help me keep creating without worrying about where the next meal comes from =)

**[All donation methods](https://github.com/timoncool/ACE-Step-Studio/blob/master/DONATE.md)** | **[dalink.to/nerual_dreming](https://dalink.to/nerual_dreming)** | **[boosty.to/neuro_art](https://boosty.to/neuro_art)**

- **BTC:** `1E7dHL22RpyhJGVpcvKdbyZgksSYkYeEBC`
- **ETH (ERC20):** `0xb5db65adf478983186d4897ba92fe2c25c594a0c`
- **USDT (TRC20):** `TQST9Lp2TjK6FiVkn4fwfGUee7NmkxEE7C`

## Star History

<a href="https://www.star-history.com/?repos=timoncool%2Fdungeon-ultimate&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=timoncool/dungeon-ultimate&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=timoncool/dungeon-ultimate&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=timoncool/dungeon-ultimate&type=date&legend=top-left" />
 </picture>
</a>

## License

[MIT](LICENSE) — same as the upstream project. Do whatever you want; attribution appreciated.
