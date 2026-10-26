#!/bin/bash
# Builds Dungeon Ultimate for Apple silicon: the .app and a .dmg. The engines (llama.cpp and sd.cpp with Metal,
# transcribe.cpp) and the models are downloaded in the game («What to download»).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
npm --prefix frontend ci --no-audit --no-fund
npm --prefix frontend run build
cargo test --workspace -j 8
cargo build --release -p du-server

npm --prefix desktop ci --no-audit --no-fund
cd desktop
npm exec tauri build -- --config src-tauri/tauri.macos-release.conf.json --bundles app,dmg

find src-tauri/target/release/bundle \( -name '*.dmg' -o -name '*.app' \) -maxdepth 2
