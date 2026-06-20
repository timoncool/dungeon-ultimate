<div align="center">

# Dungeon Ultimate

**Локальный AI-данжен-мастер с генерацией картинок, озвучкой и расцензуренной моделью — твои истории не покидают твой компьютер.**

[![License](https://img.shields.io/github/license/timoncool/dungeon-ultimate?style=flat-square)](LICENSE)
[![Stars](https://img.shields.io/github/stars/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/stargazers)
[![Last Commit](https://img.shields.io/github/last-commit/timoncool/dungeon-ultimate?style=flat-square)](https://github.com/timoncool/dungeon-ultimate/commits)

**[English](README.md)** · **[Русский](README_RU.md)**

![Dungeon Ultimate](docs/screenshots/hero.png)

</div>

## О проекте

Dungeon Ultimate — полностью локальное AI-приложение для ролевых игр: неутомимый данжен-мастер, который пишет интерактивные истории, иллюстрирует сцены и озвучивает их вслух — всё на твоей видеокарте, без облака, без API-ключей и без цензуры. Это сильно расширенный форк [open-dungeon](https://github.com/newideas99/open-dungeon), перестроенный вокруг локального сервера Gemma, расцензуренного пайплайна картинок, потокового вывода и русскоязычного интерфейса. Работает на Windows с видеокартой NVIDIA — запусти обвязку и играй на `localhost:3000`.

## Возможности

- **100% локально и приватно** — локальный сервер Gemma + локальный воркер картинок FLUX, без облака, без ключей, ничего не уходит с ПК
- **Потоковый вывод** — текст рассказчика появляется в чате слово за словом
- **Картинки прямо в истории** — модель вызывает инструмент `generate_image`, и FLUX рисует сцену внутри повествования
- **Озвучка (TTS)** — ▶ на каждом сообщении, автоозвучка, пак из 39 голосов, клонирование голоса из своего `.mp3`, громкость и скорость
- **Расцензуренный режим** — переключение на расцензуренную текстовую модель и abliterated текст-энкодер картинок для ничем не ограниченного контента 18+
- **Выбор модели в чате** — меняй текстовую модель в любом чате в любой момент
- **Одна модель на GPU за раз** — текстовая LLM выгружается на время рендера картинки и грузится обратно на следующем ходе, так что каждой достаётся вся видеопамять
- **Редактируемые промпты и настройки на чат** — промпт рассказчика, промпт картинок, мир, стиль, персонажи, длина ответа, голос
- **Русскоязычный интерфейс** — весь UI и все промпты локализованы (промпты картинок остаются на английском для FLUX)
- **Портативные Windows-лаунчеры** — `run.bat` / `stop.bat`; модели, рантаймы и кэш живут на несистемном диске

## Системные требования

- **ОС:** Windows 11 (Linux/macOS — через лаунчеры из upstream)
- **GPU:** NVIDIA с 12+ ГБ VRAM (RTX 4090 рекомендуется для связки расцензуренная 12B + FLUX)
- **Рантаймы:** Node.js 22+ и Python 3.11 venv для локальных серверов текста/картинок/TTS
- **Диск:** ~30 ГБ под GGUF-модель текста и веса FLUX

## Быстрый старт

1. **Клонировать**
   ```bash
   git clone https://github.com/timoncool/dungeon-ultimate.git
   cd dungeon-ultimate
   ```

2. **Установить**
   ```
   install.bat
   ```

3. **Запустить**
   ```
   run.bat
   ```
   Открой `http://localhost:3000`.

## Использование

- Создай чат, задай мир/стиль или выбери персонажа, напиши действие — рассказчик потоком выдаёт ход истории.
- Включи **Озвучку**, чтобы ходы читались вслух; выбери голос или загрузи `.mp3` для клонирования.
- Через дропдаун модели переключайся между обычной и расцензуренной текстовой моделью прямо по ходу.
- Правь промпты рассказчика / картинок в боковых панелях, чтобы настроить тон и арт-дирекшн.

## Конфигурация

Всё опционально — приложение работает полностью локально без ключей. См. [`.env.example`](.env.example). Ключевые переменные:

| Переменная | Назначение |
|----------|---------|
| `OPENAI_COMPAT_BASE_URL` | Локальный сервер текста (по умолчанию `http://127.0.0.1:8080/v1`) |
| `OPENAI_COMPAT_MODEL` | ID текстовой модели (напр. `gemma-4-12b-uncensored`) |
| `FLUX_WORKER_URL` | Локальный воркер картинок (по умолчанию `http://127.0.0.1:7869`) |
| `IMAGE_SERVER_DEFAULT_BACKEND` | Бэкенд картинок (`flux-uncensored` для NSFW) |
| `TTS_WORKER_URL` | Локальный TTS-сервер |

## Другие портативные нейросети

| Проект | Описание |
|--------|----------|
| [ACE-Step Studio](https://github.com/timoncool/ACE-Step-Studio) | AI-студия музыки — песни, вокал, каверы, клипы |
| [VideoSOS](https://github.com/timoncool/videosos) | AI-видеопродакшн в браузере |
| [Foundation Music Lab](https://github.com/timoncool/Foundation-Music-Lab) | Генерация музыки + таймлайн-редактор |
| [Qwen3-TTS](https://github.com/timoncool/Qwen3-TTS_portable_rus) | Синтез речи с клонированием голоса |
| [SuperCaption Qwen3-VL](https://github.com/timoncool/SuperCaption_Qwen3-VL) | Генерация описаний изображений |
| [civitai-mcp-ultimate](https://github.com/timoncool/civitai-mcp-ultimate) | Civitai API как MCP-сервер |
| [ScreenSavy](https://github.com/timoncool/ScreenSavy.com) | Генератор эмбиент-экранов |

## Авторы

- **Nerual Dreming** — [Telegram](https://t.me/nerual_dreming) | [neuro-cartel.com](https://neuro-cartel.com) | [ArtGeneration.me](https://artgeneration.me)
- **Нейро-Софт** — [Telegram](https://t.me/neuroport) | портативные нейросети

## Благодарности

Сделано на основе [**open-dungeon**](https://github.com/newideas99/open-dungeon) от [@newideas99](https://github.com/newideas99) — оригинального локального AI-приложения для ролёвок, которое этот форк расширяет. Огромное спасибо за фундамент.

## Поддержать автора

Я создаю опенсорс софт и занимаюсь исследованиями в области ИИ. Большая часть всего, что я делаю, находится в открытом доступе. Ваши пожертвования позволяют мне создавать и исследовать больше, не отвлекаясь на поиск еды для продолжения существования =)

**[Все способы поддержки](https://github.com/timoncool/ACE-Step-Studio/blob/master/DONATE.md)** | **[dalink.to/nerual_dreming](https://dalink.to/nerual_dreming)** | **[boosty.to/neuro_art](https://boosty.to/neuro_art)**

- **BTC:** `1E7dHL22RpyhJGVpcvKdbyZgksSYkYeEBC`
- **ETH (ERC20):** `0xb5db65adf478983186d4897ba92fe2c25c594a0c`
- **USDT (TRC20):** `TQST9Lp2TjK6FiVkn4fwfGUee7NmkxEE7C`

## Star History

<a href="https://www.star-history.com/?repos=timoncool%2Fdungeon-ultimate&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=timoncool/dungeon-ultimate&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=timoncool/dungeon-ultimate&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=timoncool/dungeon-ultimate&type=date&legend=top-left" />
 </picture>
</a>

## Лицензия

[MIT](LICENSE) — как и у upstream-проекта. Делай что хочешь; ссылка на авторство приветствуется.
