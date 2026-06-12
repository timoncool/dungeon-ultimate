#!/bin/bash
# Open Dungeon — first-run setup + launcher used inside the .app bundle.
# Runs in Terminal so the user can watch progress. Installs the app into
# ~/Library/Application Support/Open Dungeon using the bundled Node runtime.
set -u

RES="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$HOME/Library/Application Support/Open Dungeon"
APP_DIR="$INSTALL_DIR/app"
NODE_DIR="$INSTALL_DIR/node"
PORT="${PORT:-3000}"

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
fail() {
  printf '\n\033[31m%s\033[0m\n' "$1"
  [ -n "${2:-}" ] && open "$2"
  read -r -p "Press Enter to close..."
  exit 1
}

bold "Open Dungeon"
mkdir -p "$INSTALL_DIR"

# --- Bundled Node runtime ---
if [ ! -x "$NODE_DIR/bin/node" ]; then
  bold "Unpacking the bundled Node.js runtime (one time)..."
  rm -rf "$NODE_DIR" "$INSTALL_DIR/node-tmp"
  mkdir -p "$INSTALL_DIR/node-tmp"
  tar -xzf "$RES/node-runtime.tar.gz" -C "$INSTALL_DIR/node-tmp" --strip-components=1 \
    || fail "Could not unpack the Node runtime."
  mv "$INSTALL_DIR/node-tmp" "$NODE_DIR"
fi
export PATH="$NODE_DIR/bin:$PATH"

# --- App source (extract on first run or version change) ---
BUNDLED_VERSION="$(cat "$RES/VERSION" 2>/dev/null || echo dev)"
INSTALLED_VERSION="$(cat "$APP_DIR/.od-version" 2>/dev/null || echo none)"
if [ "$BUNDLED_VERSION" != "$INSTALLED_VERSION" ]; then
  bold "Installing Open Dungeon $BUNDLED_VERSION..."
  mkdir -p "$APP_DIR"
  tar -xzf "$RES/app-src.tar.gz" -C "$APP_DIR" || fail "Could not unpack the app."
  echo "$BUNDLED_VERSION" > "$APP_DIR/.od-version"
  rm -rf "$APP_DIR/.next"
fi
cd "$APP_DIR" || fail "Install directory missing."

# --- Ollama (checked over HTTP so the Ollama.app alone is enough) ---
if ! curl -s --max-time 2 http://127.0.0.1:11434/api/version >/dev/null 2>&1; then
  if [ -d "/Applications/Ollama.app" ]; then
    bold "Starting Ollama..."
    open -a Ollama
    for _ in $(seq 1 30); do
      curl -s --max-time 2 http://127.0.0.1:11434/api/version >/dev/null 2>&1 && break
      sleep 1
    done
  fi
fi
curl -s --max-time 2 http://127.0.0.1:11434/api/version >/dev/null 2>&1 \
  || fail "Ollama is required for the local narrator. Opening ollama.com — install it, open it once, then relaunch Open Dungeon." "https://ollama.com/download"

# --- Default narrator model ---
if ! curl -s http://127.0.0.1:11434/api/tags | grep -q "gemma4:12b-it-qat"; then
  bold "Downloading the default narrator model (gemma4:12b-it-qat, 7.2 GB, one time)..."
  curl -s -X POST http://127.0.0.1:11434/api/pull -d '{"name":"gemma4:12b-it-qat"}' \
    | while IFS= read -r line; do
        case "$line" in
          *error*) echo "$line" ;;
          *success*) echo " done." ;;
          *) printf '.' ;;
        esac
      done
  echo
  curl -s http://127.0.0.1:11434/api/tags | grep -q "gemma4:12b-it-qat" \
    || fail "Model download failed. Check your connection and relaunch."
fi

# --- Dependencies & build ---
if [ ! -d node_modules ]; then
  bold "Installing app dependencies (one time, ~a minute)..."
  npm install --no-fund --no-audit || fail "npm install failed."
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
bold "Starting Open Dungeon at http://localhost:$PORT"
if [ -z "${OPEN_DUNGEON_NO_BROWSER:-}" ]; then
  ( sleep 3 && open "http://localhost:$PORT" ) &
fi
PORT="$PORT" npm run start
