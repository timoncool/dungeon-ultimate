#!/usr/bin/env bash
# Готовит ресурсы для установщика. Запускать из корня репозитория.
#
# В установщик идёт ТОЛЬКО собранное приложение: интерфейс и всё, что нельзя докачать.
# Движки и веса моделей сюда НЕ кладутся — их забирает сама игра на первом запуске,
# показывая размер и прогресс. Иначе установщик весил бы под два гигабайта на ровном
# месте: одна только библиотека CUDA к движку картинок — 900 МБ.
set -e
STAGE=desktop/src-tauri/staging
rm -rf "$STAGE"
mkdir -p "$STAGE/frontend"

if [ ! -f frontend/dist/index.html ]; then
  echo "нет собранного интерфейса: сначала cd frontend && yarn build" >&2
  exit 1
fi
cp -r frontend/dist "$STAGE/frontend/dist"

echo "готово: $STAGE"
du -sh "$STAGE" 2>/dev/null || true
