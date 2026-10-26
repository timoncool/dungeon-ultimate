#!/bin/bash
# Builds the Linux x86-64 Dungeon Ultimate: a .deb and an AppImage. The engines, CUDA libraries and models are
# downloaded in the game («What to download»); the window is inside the binary (du-server embeds frontend/dist).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
npm --prefix frontend ci --no-audit --no-fund
npm --prefix frontend run build
cargo test --workspace -j 8

npm --prefix desktop ci --no-audit --no-fund
cd desktop
npm exec tauri build -- --config src-tauri/tauri.linux-release.conf.json --bundles deb,appimage

find src-tauri/target/release/bundle \( -name '*.deb' -o -name '*.AppImage' \)
