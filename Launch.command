#!/bin/bash
# Open Dungeon — one-click launcher.
# Double-click this file in Finder. It checks dependencies, builds the app
# the first time, starts everything, and opens your browser.
set -u

cd "$(dirname "$0")" || exit 1

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
fail() {
  printf '\n\033[31m%s\033[0m\n' "$1"
  [ -n "${2:-}" ] && open "$2"
  printf 'Fix the above, then double-click Launch.command again.\n'
  read -r -p "Press Enter to close..."
  exit 1
}

bold "Open Dungeon launcher"

# --- Node ---
if ! command -v node >/dev/null 2>&1; then
  fail "Node.js is required but not installed. Opening nodejs.org — install the LTS version." "https://nodejs.org"
fi
NODE_MAJOR=$(node -p 'process.versions.node.split(".")[0]')
if [ "$NODE_MAJOR" -lt 22 ]; then
  fail "Node.js 22+ is required (you have $(node -v)). Opening nodejs.org." "https://nodejs.org"
fi

# --- Ollama ---
if ! command -v ollama >/dev/null 2>&1 && [ ! -d "/Applications/Ollama.app" ]; then
  fail "Ollama is required for the local narrator. Opening ollama.com — install it, then relaunch." "https://ollama.com/download"
fi

if ! curl -s --max-time 2 http://127.0.0.1:11434/api/version >/dev/null 2>&1; then
  bold "Starting Ollama..."
  if [ -d "/Applications/Ollama.app" ]; then
    open -a Ollama
  else
    nohup ollama serve >/tmp/open-dungeon-ollama.log 2>&1 &
  fi
  for _ in $(seq 1 30); do
    curl -s --max-time 2 http://127.0.0.1:11434/api/version >/dev/null 2>&1 && break
    sleep 1
  done
  curl -s --max-time 2 http://127.0.0.1:11434/api/version >/dev/null 2>&1 \
    || fail "Ollama did not start. Open the Ollama app manually, then relaunch."
fi

# --- Default model ---
if ! ollama list 2>/dev/null | grep -q "gemma4:12b-it-qat"; then
  bold "Downloading the default narrator model (gemma4:12b-it-qat, 7.2 GB, one time)..."
  ollama pull gemma4:12b-it-qat || fail "Model download failed. Check your connection and relaunch."
fi

# --- App dependencies & build ---
if [ ! -d node_modules ]; then
  bold "Installing app dependencies (one time)..."
  npm install || fail "npm install failed."
fi
if [ ! -f .next/BUILD_ID ]; then
  bold "Building the app (one time, ~a minute)..."
  npm run build || fail "Build failed."
fi

# --- Optional local image generation ---
ULTRA_DIR="${ULTRA_FAST_IMAGE_GEN_DIR:-$HOME/ultra-fast-image-gen}"
if [ -d "$ULTRA_DIR" ] && ! curl -s --max-time 2 http://127.0.0.1:7869/health >/dev/null 2>&1; then
  bold "Starting the local image server..."
  nohup npm run image:server >/tmp/open-dungeon-images.log 2>&1 &
fi

# --- Run ---
bold "Starting Open Dungeon at http://localhost:3000"
( sleep 3 && open "http://localhost:3000" ) &
npm run start
