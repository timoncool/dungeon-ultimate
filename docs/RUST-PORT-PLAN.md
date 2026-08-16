# Dungeon Ultimate → Rust + Tauri: план порта

Ветка `rust-port`. Цель: заменить связку Next.js + 4 питон-сервера на нативную сборку по
образцу [dub-studio](https://github.com/timoncool/dub-studio) v3.1.1 — один exe, ноль Python,
плюс переработанная генерация картинок на свежих топовых моделях.

## 1. Что портируем (инвентарь текущей версии)

| Слой | Сейчас | Строк |
|---|---|---|
| UI | `src/app/page.tsx` (монолит) + `BookReader` | 6257 + 493 |
| Ход истории | `src/app/api/story/route.ts` | 1938 |
| Хранилище | `src/lib/db.ts` (better-sqlite3) | 1271 |
| RPG-движок | `src/lib/rpg/{apply,dice,derive,parse,types,prompt}.ts` | ~860 |
| Промпты | `src/lib/prompts/*` (7 языков) + `story-prompt.ts` | ~1300 |
| Картинки | `api/images` + `lib/flux-worker.ts` + `image_server/*.py` | ~1150 |
| Текст | `servers/od-text-server.py` (llama-cpp, Gemma 4 12B) | 390 |
| TTS | `servers/od-tts-server.py` + `tts_engine.py` (Qwen3-TTS) | 330 |
| ASR | `servers/od-asr-server.py` (onnx-asr, Parakeet) | 55 |

### Механизмы, которые нельзя потерять

1. **GPU-инвариант** — текст и картинка никогда не на GPU одновременно (одна 4090).
   Сейчас держится на клиентском `onGpu`/`gpuAcquire` в `page.tsx` + `/unload` между
   серверами. В порте — одна серверная очередь GPU-задач.
2. **Многопроходный ход** — нарратор пишет ТОЛЬКО чистую прозу; отдельными
   grammar-constrained проходами идут игровые события (`requestGameEvent`), кадр
   (`requestImageRequest`, «оператор»), и чипсы действий (`/api/actions`).
3. **Детерминированный D&D** — CSPRNG на сервере, крит удваивает КОСТИ, не модификатор;
   натуральная 20 всегда успех, 1 всегда провал; эффекты тикают по ходу; спавн врагов,
   атаки против КЗ, лут со слотами и редкостью.
4. **`RpgSnapshot`** — пред-ходовой снимок на сообщении, чтобы Retry/Erase откатывал HP,
   эффекты, врагов, предметы и журнал вместо двойного применения.
5. **Сцен-континуити** — стабильный ярлык `location`, `anchor`/`last` на локацию,
   `MAX_EDIT_HOPS = 6`, эволюционирующий референс персонажа, референсы предметов,
   максимум 3 референса на кадр.
6. **Компакция истории** — `packStoryHistory` выкидывает блоками по 16 сообщений, чтобы не
   ломать префикс-кэш модели; выкинутое сворачивается в «историю до сих пор».
7. **Честные 3D-кости** — движок катит первым, кубик пиннится на `1d20@N`.
8. **7 языков** игры, мультиголосая озвучка, голосовой ввод.

## 2. Целевая архитектура

```
crates/
  du-core      типы (Chat/Message/Settings/RPG/Item/Enemy/Scene) — зеркало lib/types.ts
  du-rpg       детерминированный движок: dice(CSPRNG)/apply/derive — порт lib/rpg/*
  du-prompts   7 языков + story-prompt (история, анти-повтор, системные промпты, чистка утечек)
  du-llm       ← из dub-studio: сайдкар llama-server + OpenAI-клиент (vision, JSON-schema, SSE)
  du-asr       ← из dub-studio: parakeet-rs + ensure_ort_dylib
  du-tts       ← из dub-studio: audiocpp (Higgs Audio v3) + паки голосов
  du-image     НОВЫЙ: FFI над stable-diffusion.dll
  du-db        rusqlite: chats/messages/characters/scenes/events/items + миграции
  du-server    axum: REST + SSE, ОДНА GPU-очередь, раздача SPA, докачка моделей
desktop/src-tauri   Tauri 2, du-server встроен в процесс (один exe), updater с GitHub
frontend/           React 19 + Vite + TS + Tailwind v4 + zustand + i18next + motion + lucide
```

## 3. Генерация картинок

**Движок:** [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) (6.7k★, MIT,
билды ежедневно). Официальный `sd-master-*-bin-win-cuda12-x64.zip` содержит
`stable-diffusion.dll` (52 МБ) + `ggml-cuda.dll` — то есть shared-lib уже собран, cmake и
CUDA-тулчейн в нашей сборке НЕ нужны. Линкуем динамически (`libloading`), ровно как
`audiocpp_engine.dll` в даб-студии. Версия пинуется, на старте сверяется `sd_commit()`.

Резервный путь, если DLL окажется нестабильной под нагрузкой (прецедент Higgs: залипание
CUDA-графа лечится только рестартом процесса) — тот же прибилд несёт `sd-server.exe` с async
API `/sdcpp/v1/img_gen`. Поэтому генерация спрятана за трейтом с двумя реализациями.

**Модель — ОДНА, универсальная: Krea-2 Turbo.** Лучшая открытая в Artificial Analysis
Text-to-Image Arena на август 2026 (ELO 1223, #15 — выше Ideogram-4 1217 и FLUX.2-klein 9B
1149), 8 шагов. Редактирование по референсу она тоже умеет: в таблице `docs/edit.md`
движка для неё есть пресеты `krea2_ostris_edit` и `krea2_edit` — консистентность героя и
локаций даёт community edit-LoRA поверх той же модели, второй набор весов не нужен.

| Компонент | Файл | Размер |
|---|---|---|
| DiT | Krea-2 Turbo, GGUF Q4_K_M | 7.2 ГБ |
| Текст-энкодер | Qwen3-VL-4B abliterated, GGUF | ~2.5 ГБ |
| VAE | Wan 2.1 | 0.25 ГБ |
| Edit-LoRA | community edit (ostris) | сотни МБ |

Одна резидентная модель на GPU, никаких свапов между кадрами. LoRA применяется в
рантайме (`LORA_APPLY_AUTO`).

**Замер на RTX 4090 (проверено живьём через наш FFI):**

| Режим | Шагов | На шаг | Итого с загрузкой |
|---|---|---|---|
| Рисование с нуля, 1024×1024 | 8 | 0.63 с | 11.8 с |
| Правка по референсу (+ edit-LoRA, `preset=krea2_edit`) | 8 | 2.0 с | 24.9 с |

Правка сохраняет локацию покадрово — арка, факел, плющ, вода и свет остаются теми же,
меняется только то, что просит промпт. Это и закрывает консистентность героя и мест.

**Квант:** GGUF **Q4_K_M** — тот же квант, что уже проверен на этом железе для текстовой
модели в Dub Studio. Квантование не изобретаем, берём принятый в проекте дефолт.

**Без цензуры:** у всех трёх цензура сидит прежде всего в текст-энкодере, а он тут — обычный
Qwen3 / Qwen3-VL. Ставим abliterated-версии и снимаем отказы, не трогая гейтнутые репы.
Это заменяет нынешнюю схему с `IMAGE_SERVER_DEFAULT_BACKEND=flux-uncensored`, которая
требует HF-токен и доступ к закрытому репозиторию.

## 4. Майлстоуны

- **M0** Каркас: workspace, `du-core`, `du-db`, скелет `du-server`, оболочка Tauri. Сборка зелёная.
- **M1** `du-image`: FFI над DLL, докачка весов, обе модели, кадр проверен глазами.
- **M2** `du-llm`: сайдкар llama-server, стриминг, structured-проходы по JSON-схеме.
- **M3** `du-rpg` + `du-prompts`: порт 1:1 с юнит-тестами на поведение TS-версии.
- **M4** `du-server`: полный контракт REST/SSE, GPU-очередь, сцен-континуити.
- **M5** Фронт: порт `page.tsx` в компоненты, 3D-кости, i18n на 7 языков.
- **M6** `du-tts` + `du-asr`.
- **M7** Портатив, инсталлятор, авто-обновление, CI, README.

После каждого майлстоуна — цикл код-ревью по диффу и состязательное ревью.
