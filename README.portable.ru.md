# Open Dungeon — портативка (Windows)

[![Stars](https://img.shields.io/github/stars/timoncool/Open-Dungeon-portable-ru?style=social)](https://github.com/timoncool/Open-Dungeon-portable-ru/stargazers)
[![License](https://img.shields.io/github/license/timoncool/Open-Dungeon-portable-ru)](LICENSE)

**Полностью локальная** нейро-ролевая игра: и текст истории, и встроенные
картинки сцен генерируются прямо на вашей машине. Без аккаунтов, без API-ключей,
без облака. Истории никогда не покидают компьютер.

Это **портативная Windows-сборка**: приложение, оба локальных Python-сервиса,
все модели и кэш лежат в ОДНОЙ папке на несистемном диске. Ничего не ставится в
систему, не пишет в `C:` / `AppData`, не лезет в PATH. Удалил папку — удалил
приложение целиком.

> Оригинальный проект: [newideas99/open-dungeon](https://github.com/newideas99/open-dungeon)
> (Next.js 16 / Node). Эта сборка добавляет нативный Windows-движок: текст на
> локальной Gemma 4 12B (CUDA llama-cpp-python) и встроенную генерацию картинок
> FLUX.2 SDNQ на RTX.

![Сцена истории со встроенной сгенерированной картинкой](docs/hero.png)

## Что умеет

- **Локальный текст** — рассказчик пишет историю на Google Gemma 4 12B QAT
  (Q4_0 GGUF) через CUDA-сборку `llama-cpp-python`. Всё на GPU, длинная память
  истории (контекст до 65K токенов), скрытый «thinking»-канал отключён — весь
  бюджет токенов идёт в текст.
- **Локальные картинки** — рассказчик вызывает инструмент `generate_image`, и
  сцены отрисовываются встроенно бэкендом **FLUX.2 SDNQ** из проекта
  `ultra-fast-image-gen` (PyTorch, CUDA).
- **Режимы ввода** — Do / Say / Story, плюс Continue, Retry, Erase и правка
  любого абзаца на месте.
- **Память длинной истории** — старые абзацы сворачиваются в скользящее резюме
  «что было ранее», а не теряются.
- **Персонажи с визуальной преемственностью** — портреты персонажей уходят и
  рассказчику (vision-контекст), и генератору картинок (референсы).
- **Приватность по умолчанию** — чаты, персонажи и картинки лежат в локальной
  SQLite-базе и папках `public/` на вашем диске.

<table>
  <tr>
    <td><img src="docs/prose.png" alt="Текст истории с засечками" /></td>
    <td><img src="docs/modal.png" alt="Диалог новой истории с пресетами" /></td>
  </tr>
</table>

## Системные требования

- Windows 10/11, 64-bit.
- **NVIDIA RTX-видеокарта** для GPU-режима. Эталон сборки — **RTX 4090**
  (Ada, SM 8.9, CUDA 12.8 / cu128). PyTorch и `llama-cpp-python` ставятся под
  cu128. Работает и на других RTX (30xx/40xx/50xx) — выбор CUDA при установке.
- ОЗУ: 16 ГБ минимум, 24 ГБ+ комфортно (текст + картинки одновременно).
- Диск: ~25–30 ГБ под Python/Node-рантаймы и веса (GGUF Gemma 4 ~7 ГБ + FLUX.2).
- Текст работает и без картинок. Генерация картинок — опциональна.

## Что внутри портативки

Всё нестится в корне папки приложения (имена каталогов — в `.gitignore`):

| Папка | Что это |
|-------|---------|
| `node/` | Портативный Node.js рантайм (для Next.js-фронтенда) |
| `python/` | Embedded Python + CUDA `llama-cpp-python` для текст-сервера |
| `ultra-fast-image-gen/` | Клон image-воркера FLUX.2 SDNQ (со своим `.venv`) |
| `models/` | GGUF Gemma 4 12B + mmproj, веса FLUX.2 SDNQ, HF/torch кэш |
| `cache/` | Общий кэш (HF, torch, XDG) |
| `temp/` | Временные файлы (TEMP/TMP перенаправлены сюда) |
| `generated/` `output/` | Сгенерированные картинки и результаты |
| `data/` | Локальная SQLite-база историй |

Два локальных Python-сервиса, которые оборачивает портативка:

- **Текст-сервер** (`od-text-server.py`) — минимальный OpenAI-совместимый
  сервер поверх локального GGUF Gemma 4 12B. Open Dungeon в режиме
  **Connect a server** ходит на `POST /v1/chat/completions`
  (`http://127.0.0.1:8080/v1`). Грузит все слои на 4090
  (`n_gpu_layers=-1`, `flash_attn=True`).
- **Image-воркер** — HTTP-сервис из `ultra-fast-image-gen`, бэкенд
  `flux2-4b-sdnq` на CUDA (`http://127.0.0.1:7869`).

## Установка

1. Скачать архив сборки из [Releases](https://github.com/timoncool/Open-Dungeon-portable-ru/releases)
   и распаковать в папку на несистемном диске (например `D:\Open-Dungeon`).
   В пути не должно быть кириллицы.
2. Двойной клик по **`install.bat`**. Установщик:
   - спросит GPU (выбор CUDA-сборки, эталон — RTX 40xx / cu128);
   - развернёт портативный Node.js в `node/` и поставит зависимости приложения;
   - развернёт embedded Python в `python/`, поставит PyTorch + CUDA
     `llama-cpp-python` и зависимости текст-сервера;
   - склонирует `ultra-fast-image-gen` внутрь папки и поднимет его `.venv`
     с PyTorch CUDA-колёсами;
   - соберёт Next.js-приложение (`next build`).
3. Двойной клик по **`run.bat`** — поднимает текст-сервер, image-воркер и
   веб-приложение, открывает http://localhost:3000.

> Веса Gemma 4 12B (GGUF) и FLUX.2 SDNQ скачиваются автоматически при первом
> запуске в `models/` внутри папки. Ручных действий не требуется.

## Запуск

- **`run.bat`** — запустить всё (текст + картинки + веб).
- **`Stop-Windows.bat`** — остановить веб-приложение и Python-сервисы.
- **`update.bat`** — `git pull` портативки и обновление клона image-воркера.

После старта откройте http://localhost:3000. В панели **Text Model** выберите
провайдера и модель для чата (локальный текст-сервер или внешний
OpenAI-совместимый бэкенд).

## Изоляция (важно)

Лаунчеры перенаправляют ВСЕ переменные окружения внутрь папки приложения, чтобы
ничего не утекало в систему:

- `HF_HOME`, `HUGGINGFACE_HUB_CACHE`, `TRANSFORMERS_CACHE` → `models/`
- `TORCH_HOME` → `models/torch`
- `XDG_CACHE_HOME` → `cache/`
- `TEMP`, `TMP` → `temp/`
- `npm_config_cache`, `NPM_CONFIG_PREFIX` → внутрь `node/` / `cache/`
- `KMP_DUPLICATE_LIB_OK=TRUE`, `import torch` до `llama_cpp` (порядок загрузки
  CUDA DLL)

## Играть

Композер истории — три режима ввода: **Do** (действие игрока), **Say**
(реплика), **Story** (написать повествование самому). Сверху — **Continue**,
**Retry**, **Erase**; наведение на сообщение → **Edit** для правки на месте.

## Картинки (опционально)

Встроенные картинки рисует image-воркер `ultra-fast-image-gen`, бэкенд
`flux2-4b-sdnq` на CUDA. Дефолт: 1024 по длинной стороне, 4 шага, guidance 0.0;
медленный режим — 2048. Доступны квадрат / портрет / пейзаж, до двух референсов
на запрос. Без запущенного воркера всё остальное работает — у картинок будет
кнопка «Сгенерировать», которая сработает, как только воркер поднят.

## Локальные данные

Чаты и сообщения — в SQLite (`data/local-roleplay.sqlite`). Загруженные картинки
— `public/uploads/`, сгенерированные — `public/generated/`. Кнопка очистки в
панели **Local Data** удаляет все локальные истории, сообщения, персонажей,
картинки и временные референсы.

## Лицензия

MIT. © оригинальный автор проекта (см. файл [LICENSE](LICENSE)).

---

## Другие портативные нейросети

| Проект | Описание |
|--------|----------|
| [Foundation Music Lab](https://github.com/timoncool/Foundation-Music-Lab) | Генерация музыки + таймлайн-редактор |
| [VibeVoice ASR](https://github.com/timoncool/VibeVoice_ASR_portable_ru) | Распознавание речи (ASR) |
| [LavaSR](https://github.com/timoncool/LavaSR_portable_ru) | Улучшение качества аудио |
| [Qwen3-TTS](https://github.com/timoncool/Qwen3-TTS_portable_rus) | Синтез речи (TTS) от Qwen |
| [SuperCaption Qwen3-VL](https://github.com/timoncool/SuperCaption_Qwen3-VL) | Генерация описаний изображений |
| [VideoSOS](https://github.com/timoncool/videosos) | AI-видеопродакшн в браузере |
| [RC Stable Audio Tools](https://github.com/timoncool/RC-stable-audio-tools-portable) | Генерация музыки и аудио |

## Авторы

- **Nerual Dreming** ([t.me/nerual_dreming](https://t.me/nerual_dreming)) — [neuro-cartel.com](https://neuro-cartel.com) | основатель [ArtGeneration.me](https://artgeneration.me)
- **Нейро-Софт** ([t.me/neuroport](https://t.me/neuroport)) — репаки и портативки нейросетей

---

> **Если проект полезен — поставьте звёздочку!** Это помогает другим находить проект и мотивирует на развитие.

## Star History

<a href="https://www.star-history.com/?repos=timoncool%2FOpen-Dungeon-portable-ru&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/image?repos=timoncool/Open-Dungeon-portable-ru&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/image?repos=timoncool/Open-Dungeon-portable-ru&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/image?repos=timoncool/Open-Dungeon-portable-ru&type=date&legend=top-left" />
 </picture>
</a>
