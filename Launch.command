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

ENV_SERVER=".env.server"

set_env_value() {
  key="$1"
  value="$(printf '%s' "$2" | tr -d '\r\n')"
  tmp="$(mktemp)"
  touch "$ENV_SERVER"
  awk -v key="$key" -v line="$key=$value" '
    BEGIN { done = 0 }
    $0 ~ "^" key "=" { print line; done = 1; next }
    { print }
    END { if (!done) print line }
  ' "$ENV_SERVER" > "$tmp" && mv "$tmp" "$ENV_SERVER"
}

env_value() {
  [ -f "$ENV_SERVER" ] || return 0
  awk -F= -v key="$1" '$1 == key { sub(/^[^=]*=/, ""); print; exit }' "$ENV_SERVER"
}

ensure_ollama_backend() {
  set_env_value "DEFAULT_TEXT_PROVIDER" "local"
  set_env_value "OLLAMA_BASE_URL" "http://127.0.0.1:11434"

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

  if ! ollama list 2>/dev/null | grep -q "gemma4:12b-it-qat"; then
    bold "Downloading the default narrator model (gemma4:12b-it-qat, 7.2 GB, one time)..."
    ollama pull gemma4:12b-it-qat || fail "Model download failed. Check your connection and relaunch."
  fi
}

configure_custom_backend() {
  bold "Connect a server"
  read -r -p "Backend URL [http://localhost:1234/v1]: " backend_url
  backend_url="${backend_url:-http://localhost:1234/v1}"
  read -r -p "Model name (leave blank to fill in the app): " backend_model
  read -r -s -p "API key (optional, hidden): " backend_key
  printf '\n'

  set_env_value "DEFAULT_TEXT_PROVIDER" "custom"
  set_env_value "OPENAI_COMPAT_BASE_URL" "$backend_url"
  set_env_value "OPENAI_COMPAT_MODEL" "$backend_model"
  if [ -n "$backend_key" ]; then
    set_env_value "OPENAI_COMPAT_API_KEY" "$backend_key"
  fi

  bold "Saved custom backend defaults to $ENV_SERVER"
}

configure_text_backend() {
  configured_provider="$(env_value DEFAULT_TEXT_PROVIDER)"
  configured_custom_url="$(env_value OPENAI_COMPAT_BASE_URL)"
  if [ -n "$configured_provider" ] || [ -n "$configured_custom_url" ]; then
    if [ "$configured_provider" = "custom" ] || [ -n "$configured_custom_url" ]; then
      bold "Using saved custom text backend settings."
    else
      bold "Using saved Ollama text settings."
    fi
    return
  fi

  bold "Text model setup"
  printf 'Choose a narrator backend:\n'
  printf '  1) Ollama on this Mac (default)\n'
  printf '  2) OpenAI-compatible server URL (LM Studio, llama.cpp, OpenRouter, remote Ollama)\n'
  read -r -p "Press Enter for Ollama, or type 2 for a custom server: " backend_choice
  backend_choice="$(printf '%s' "$backend_choice" | tr '[:upper:]' '[:lower:]')"

  case "$backend_choice" in
    2|c|custom)
      configure_custom_backend
      ;;
    *)
      ensure_ollama_backend
      ;;
  esac
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

# --- Text backend ---
configure_text_backend

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
