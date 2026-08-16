#!/usr/bin/env bash
# Готовит ресурсы для установщика. Запускать из корня репозитория.
set -e
STAGE=desktop/src-tauri/staging
rm -rf "$STAGE"
mkdir -p "$STAGE/frontend" "$STAGE/models/runtime" "$STAGE/tools"

if [ ! -f frontend/dist/index.html ]; then
  echo "нет собранного фронта: сначала cd frontend && npm run build" >&2
  exit 1
fi
cp -r frontend/dist "$STAGE/frontend/dist"

# Рантайм движка картинок кладём в установщик: без него приложение не нарисует ни кадра,
# а весит он немного по сравнению с весами.
if [ -d models/runtime/sd ]; then
  cp -r models/runtime/sd "$STAGE/models/runtime/sd"
fi
# Сайдкар текстовой модели — тоже рантайм, не веса.
if [ -d tools/llama ]; then
  cp -r tools/llama "$STAGE/tools/llama"
fi

echo "готово: $STAGE"
du -sh "$STAGE" 2>/dev/null || true
