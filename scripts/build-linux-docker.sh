#!/bin/bash
# The Linux x86-64 .deb and AppImage, built in an Ubuntu 22.04 container (glibc 2.35, so they run there and on anything
# newer; Docker under WSL on Windows) from the committed HEAD.
#   scripts/build-linux-docker.sh <output folder> <cache folder> [--check]
# The cache keeps Rust, Node and the cargo target between runs. --check runs the workspace tests only, no packages.
set -euo pipefail

if [ "${1:-}" = "--inside" ]; then
  export DEBIAN_FRONTEND=noninteractive APPIMAGE_EXTRACT_AND_RUN=1 RUSTUP_TOOLCHAIN=stable CARGO_TERM_COLOR=never
  export CARGO_HOME=/cache/cargo RUSTUP_HOME=/cache/rustup npm_config_cache=/cache/npm PATH=/cache/cargo/bin:/cache/node/bin:$PATH
  echo "[..] packages"
  apt-get update -q >/dev/null
  apt-get install -y -q --no-install-recommends build-essential pkg-config curl ca-certificates git unzip xz-utils \
    libssl-dev libwayland-dev libxkbcommon-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
    librsvg2-dev libdbus-1-dev libxdo-dev patchelf file libfuse2 >/dev/null
  if [ ! -x /cache/node/bin/node ]; then
    echo "[..] node 24"
    v=$(curl -s https://nodejs.org/dist/latest-v24.x/ | grep -o 'node-v24[0-9.]*-linux-x64.tar.xz' | head -1)
    mkdir -p /cache/node && curl -sL "https://nodejs.org/dist/latest-v24.x/$v" | tar -xJ -C /cache/node --strip-components=1
  fi
  if [ ! -x /cache/cargo/bin/cargo ]; then
    echo "[..] rust stable"
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
  fi
  echo "[..] source $(git -c safe.directory=/src -C /src rev-parse --short HEAD)"
  rm -rf /cache/src && mkdir -p /cache/src
  git -c safe.directory=/src -C /src archive HEAD | tar -x -C /cache/src
  cd /cache/src
  export CARGO_TARGET_DIR=/cache/target
  if [ "${2:-}" = "--check" ]; then
    echo "[..] tests (log: check.log)"
    npm --prefix frontend ci --no-audit --no-fund > /out/check.log 2>&1
    npm --prefix frontend run build >> /out/check.log 2>&1
    cargo test --workspace -j 8 >> /out/check.log 2>&1 || { echo "[ERROR] tests failed"; grep -E "^error|panicked|FAILED" /out/check.log | head -40; exit 1; }
    grep -E "^test result" /out/check.log
    echo "[OK] Dungeon Ultimate builds and passes its tests on Ubuntu 22.04"
    exit 0
  fi
  mkdir -p desktop/src-tauri
  ln -sfn /cache/target desktop/src-tauri/target
  echo "[..] build (log: build.log)"
  bash scripts/build-release-linux.sh > /out/build.log 2>&1 || { echo "[ERROR] build failed"; tail -40 /out/build.log; exit 1; }
  version="$(node -p "require('./desktop/src-tauri/tauri.conf.json').version")"
  deb="$(find /cache/target/release/bundle/deb -name "*_${version}_amd64.deb" | head -n 1)"
  appimage="$(find /cache/target/release/bundle/appimage -name "*_${version}_amd64.AppImage" | head -n 1)"
  [ -f "$deb" ] && [ -f "$appimage" ] || { echo "[ERROR] no ${version} package in the bundle folder"; exit 1; }
  cp "$deb" "/out/DungeonUltimate-${version}-linux-amd64.deb"
  cp "$appimage" "/out/DungeonUltimate-${version}-linux-x86_64.AppImage"
  ls -la /out/*.deb /out/*.AppImage
  echo "[OK] Dungeon Ultimate ${version} for Linux"
  exit 0
fi

out="${1:?output folder}"
cache="${2:?cache folder}"
mode="${3:-}"
repo="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$out" "$cache"
cp "$repo/scripts/build-linux-docker.sh" "$out/.build-linux-docker.sh"
docker run --rm -v "$repo:/src:ro" -v "$(cd "$out" && pwd):/out" -v "$(cd "$cache" && pwd):/cache" ubuntu:22.04 \
  bash /out/.build-linux-docker.sh --inside "$mode"
