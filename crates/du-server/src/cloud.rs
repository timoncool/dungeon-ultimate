//! Облачный режим: играть можно вообще без видеокарты.
//!
//! Каждая стадия хода умеет работать двумя путями — на своей карте или через OpenRouter, — и
//! переключается независимо. Это же даёт запасной вариант, когда карта занята или её просто
//! нет: рассказчик, кадр, озвучка и распознавание речи включаются в облако по отдельности.
//!
//! Ключ уходит сайдкару через переменную окружения, а не аргументом командной строки: аргументы
//! видны в списке процессов.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::bytes::{base64_decode, base64_encode, wrap_pcm16_wav};
use crate::runtime::Runtime;

/// Стадия, которую можно увести в облако.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Narrator,
    Image,
    Tts,
    Asr,
}

/// Включена ли стадия в облаке: нужен и флаг, и ключ, и выбранная модель. Иначе работаем
/// на своей карте — молча, без падения.
pub fn stage_enabled(runtime: &Runtime, stage: Stage) -> bool {
    if runtime.openrouter_key.trim().is_empty() {
        return false;
    }
    let (on, model) = match stage {
        Stage::Narrator => (runtime.openrouter_narrator_on, &runtime.openrouter_narrator_model),
        Stage::Image => (runtime.openrouter_image_on, &runtime.openrouter_image_model),
        Stage::Tts => (runtime.openrouter_tts_on, &runtime.openrouter_tts_model),
        Stage::Asr => (runtime.openrouter_asr_on, &runtime.openrouter_asr_model),
    };
    on && !model.trim().is_empty()
}

/// Путь к сайдкару, который умеет речь и распознавание: обычным чат-запросом их не сделать.
pub fn helper_bin(root: &Path) -> PathBuf {
    if let Ok(path) = std::env::var("DU_OPENROUTER_HELPER") {
        return PathBuf::from(path);
    }
    let exe = if cfg!(windows) { "openrouter-helper.exe" } else { "openrouter-helper" };
    root.join("tools").join("openrouter-helper").join(exe)
}

/// Выполнить операцию сайдкара: имя операции аргументом, тело — JSON на вход.
fn run_helper(root: &Path, key: &str, op: &str, payload: &Value) -> Result<Value, String> {
    let bin = helper_bin(root);
    if !bin.is_file() {
        return Err(format!("сайдкар OpenRouter не найден: {}", bin.display()));
    }
    let mut child = Command::new(&bin)
        .arg(op)
        // Ключ только через окружение: аргументы командной строки видны в списке процессов.
        .env("OPENROUTER_API_KEY", key)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("не запустить сайдкар: {error}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .map_err(|error| format!("не передать запрос сайдкару: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("сайдкар не ответил: {error}"))?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr);
        return Err(format!("сайдкар вернул ошибку: {}", details.trim()));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("непонятный ответ сайдкара: {error}"))
}

/// Частота и разрядность потока, который отдаёт провайдер: сырой 24 кГц моно, 16 бит.
const CLOUD_PCM_RATE: u32 = 24_000;

/// Озвучить текст в облаке и записать WAV.
///
/// Два разных мира моделей, поэтому и путей два:
///
/// * Настоящие модели озвучки (их 18: Google, Grok, Deepgram, Kokoro и прочие) живут на
///   отдельной ручке речи — туда ходит сайдкар, тот же самый, что в dub-studio.
/// * Чат-модели с аудио на выходе (`openai/gpt-audio`) на той ручке ОТСУТСТВУЮТ: их зовут
///   обычным запросом к чату, и аудио оттуда идёт только потоком — обычный запрос
///   отвергается с прямым указанием «Audio output requires stream: true».
///
/// Пробуем сайдкар, а на чат откатываемся: так работает любая выбранная модель.
pub fn speak(root: &Path, runtime: &Runtime, text: &str, out: &Path) -> Result<(), String> {
    let voice = default_voice_for(&runtime.openrouter_tts_model, &runtime.openrouter_tts_voice);
    let payload = json!({
        "model": runtime.openrouter_tts_model,
        "input": text,
        "voice": voice,
        "out": out.to_string_lossy(),
    });
    // Сначала пробуем сами: у сайдкара свой короткий таймаут, и на длинном ходе он
    // обрывает чтение ответа на полпути («context deadline exceeded»), хотя провайдер
    // ещё договаривает. Здесь срок держим под стать длине текста.
    match speak_via_speech_endpoint(runtime, text, &voice, out) {
        Ok(()) => return Ok(()),
        Err(error) => tracing::warn!("прямая озвучка не вышла ({error}), пробуем сайдкар"),
    }
    let primary = match run_helper(root, &runtime.openrouter_key, "tts", &payload) {
        Ok(_) if out.is_file() => return Ok(()),
        Ok(_) => "сайдкар озвучки не записал файл".to_string(),
        Err(error) => error,
    };
    tracing::warn!("озвучка через сайдкар не вышла ({primary}), пробуем чат");
    // Если не вышло и через чат — показываем ошибку ПЕРВОГО пути: он основной, и именно его
    // причина объясняет, что пошло не так. Ошибка запасного пути только путала бы.
    speak_via_chat(runtime, text, out, &voice).map_err(|fallback| {
        tracing::warn!("озвучка через чат тоже не вышла: {fallback}");
        primary
    })
}

/// Голос для облачной озвучки: выбранный игроком, а если не выбран — первый подходящий из
/// справочника этой модели.
///
/// Пустой голос провайдер отвергает сразу, поэтому подставить хоть какой-то обязательно:
/// иначе озвучка молча пропадала бы всякий раз, когда игрок сменил модель и не тронул голос.
fn default_voice_for(model: &str, chosen: &str) -> String {
    let chosen = chosen.trim();
    if !chosen.is_empty() {
        return chosen.to_string();
    }
    let voices = crate::voice_catalog::voices(model);
    let pick = voices.and_then(|voices| {
        // Русская речь для этой игры — норма, поэтому при прочих равных берём голос, который
        // её тянет.
        voices.iter().find(|voice| voice.ru).or_else(|| voices.first())
    });
    match pick {
        Some(voice) => voice.name.clone(),
        // Совсем незнакомая модель: имя голоса по умолчанию у чат-моделей провайдера.
        None => "onyx".to_string(),
    }
}

/// Озвучка настоящей моделью речи: отдельная ручка провайдера отдаёт готовый звук целиком.
///
/// Формат просим `wav`: он ложится на диск как есть. Срок ожидания считаем от длины текста —
/// на длинном ходе синтез идёт заметно дольше, и жёсткий короткий таймаут рвал бы его зря.
fn speak_via_speech_endpoint(
    runtime: &Runtime,
    text: &str,
    voice: &str,
    out: &Path,
) -> Result<(), String> {
    // Формат СЫРОЙ, всегда: так же сделано в Dub Studio, и там это выстрадано — pcm
    // (24 кГц, 16 бит, моно) отдают все модели речи, включая мультиязычные, а перебора по
    // моделям не требуется. На `wav` ручка отвечает отказом. Заголовок дописываем сами.
    let body = json!({
        "model": runtime.openrouter_tts_model,
        "input": text,
        "voice": voice,
        "response_format": "pcm",
    });
    // Минута на запрос плюс секунда на каждые сто символов: на длинном ходе синтез идёт
    // заметно дольше, и жёсткий короткий срок рвал бы его зря.
    let timeout = std::time::Duration::from_secs(60 + (text.chars().count() as u64) / 100);
    let agent = tolerant_agent(timeout);

    let response = agent
        .post("https://openrouter.ai/api/v1/audio/speech")
        .header("Authorization", &format!("Bearer {}", runtime.openrouter_key))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|error| format!("облачная озвучка: {error}"))?;
    let status = response.status().as_u16();
    if status >= 400 {
        let raw = response.into_body().read_to_string().unwrap_or_default();
        return Err(explain("облачная озвучка", status, &raw));
    }
    // Для сырого потока частота приходит В ЗАГОЛОВКЕ: `audio/pcm;rate=24000;channels=1`.
    // Считывать её оттуда надёжнее, чем полагаться на то, что она всегда одна и та же.
    let rate = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split(';')
                .find_map(|part| part.trim().strip_prefix("rate="))
                .and_then(|rate| rate.trim().parse::<u32>().ok())
        })
        .unwrap_or(CLOUD_PCM_RATE);
    let mut audio = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut audio)
        .map_err(|error| format!("облачная озвучка: обрыв ответа: {error}"))?;
    if audio.len() < 2_000 {
        // Короткий ответ — это не звук, а сообщение об ошибке: показываем его как есть.
        return Err(format!(
            "облачная озвучка: {}",
            String::from_utf8_lossy(&audio).trim().chars().take(200).collect::<String>()
        ));
    }
    // Часть моделей всё же отдаёт готовый WAV — тогда оборачивать нечего.
    let bytes = if audio.starts_with(b"RIFF") {
        audio
    } else {
        wrap_pcm16_wav(&audio, rate)
    };
    std::fs::write(out, bytes).map_err(|error| format!("не записать облачную озвучку: {error}"))
}

/// Озвучка чат-моделью: аудио приходит кусками в base64 внутри событий потока, поэтому
/// собираем их и оборачиваем в заголовок WAV сами — заодно не тянем сюда ffmpeg.
fn speak_via_chat(runtime: &Runtime, text: &str, out: &Path, voice: &str) -> Result<(), String> {
    let body = json!({
        "model": runtime.openrouter_tts_model,
        "modalities": ["text", "audio"],
        "audio": { "voice": voice, "format": "pcm16" },
        "stream": true,
        "messages": [{ "role": "user", "content": text }],
    });

    let response = ureq::post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", &format!("Bearer {}", runtime.openrouter_key))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|error| format!("облачная озвучка: {error}"))?;

    let stream = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("облачная озвучка: обрыв потока: {error}"))?;

    let mut pcm: Vec<u8> = Vec::new();
    for line in stream.lines() {
        let Some(payload) = line.strip_prefix("data:") else { continue };
        let payload = payload.trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(payload) else { continue };
        let chunk = event
            .pointer("/choices/0/delta/audio/data")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if chunk.is_empty() {
            continue;
        }
        let decoded = base64_decode(chunk)?;
        pcm.extend_from_slice(&decoded);
    }
    if pcm.len() < 2_000 {
        return Err(format!("облачная озвучка вернула слишком мало данных ({} байт)", pcm.len()));
    }

    std::fs::write(out, wrap_pcm16_wav(&pcm, CLOUD_PCM_RATE))
        .map_err(|error| format!("не записать облачную озвучку: {error}"))
}

/// Распознать речь в облаке.
///
/// Как и с озвучкой, миров два: настоящие модели расшифровки (Whisper, Chirp, Parakeet и
/// прочие — их 19) знает сайдкар, а мультимодальные чат-модели на ручке расшифровки
/// отсутствуют, и запись им отдают куском сообщения. Пробуем сайдкар, откатываемся на чат.
pub fn transcribe(root: &Path, runtime: &Runtime, wav: &Path, language: &str) -> Result<String, String> {
    let mut payload = json!({
        "model": runtime.openrouter_asr_model,
        "audio": wav.to_string_lossy(),
    });
    if !language.trim().is_empty() && !language.eq_ignore_ascii_case("auto") {
        payload["language"] = json!(language);
    }
    // Сначала своей ручкой: по документации `verbose_json` понимают только совместимые с
    // OpenAI провайдеры, остальные отвечают отказом, — а сайдкар просит именно его.
    // Нам разметка по секундам не нужна вовсе, поэтому просим простой `json`.
    match transcribe_direct(runtime, wav, language) {
        Ok(text) => return Ok(text),
        Err(error) => tracing::warn!("прямое распознавание не вышло ({error}), пробуем сайдкар"),
    }
    match run_helper(root, &runtime.openrouter_key, "stt", &payload) {
        Ok(value) => {
            let text = value
                .get("text")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string);
            if let Some(text) = text {
                return Ok(text);
            }
            let joined = value
                .get("segments")
                .and_then(Value::as_array)
                .map(|segments| {
                    segments
                        .iter()
                        .filter_map(|segment| segment.get("text").and_then(Value::as_str))
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            if !joined.is_empty() {
                return Ok(joined);
            }
            tracing::warn!("сайдкар распознавания вернул пустой текст, пробуем чат");
        }
        Err(error) => {
            tracing::warn!("распознавание через ручку расшифровки не вышло ({error}), пробуем чат");
            return transcribe_via_chat(runtime, wav, language).map_err(|fallback| {
                tracing::warn!("распознавание через чат тоже не вышло: {fallback}");
                error
            });
        }
    }
    transcribe_via_chat(runtime, wav, language)
}

/// Распознавание своей ручкой: запись уходит в base64, ответ — обычный json с текстом.
///
/// Формат записи объявляем `wav`: по документации его принимают все провайдеры, а браузер
/// именно его и пишет. Ссылкой звук на этой ручке передавать нельзя.
fn transcribe_direct(runtime: &Runtime, wav: &Path, language: &str) -> Result<String, String> {
    let audio = std::fs::read(wav).map_err(|error| format!("запись не прочиталась: {error}"))?;
    let mut body = json!({
        "model": runtime.openrouter_asr_model,
        "input_audio": { "data": base64_encode(&audio), "format": "wav" },
        "response_format": "json",
    });
    let language = language.trim();
    if !language.is_empty() && !language.eq_ignore_ascii_case("auto") {
        body["language"] = json!(language);
    }
    // Провайдер отводит на распознавание около минуты — столько же ждём и мы, с запасом.
    let agent = tolerant_agent(std::time::Duration::from_secs(120));

    let response = agent
        .post("https://openrouter.ai/api/v1/audio/transcriptions")
        .header("Authorization", &format!("Bearer {}", runtime.openrouter_key))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|error| format!("облачное распознавание: {error}"))?;
    let status = response.status().as_u16();
    let raw = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("облачное распознавание: обрыв ответа: {error}"))?;
    if status >= 400 {
        return Err(explain("облачное распознавание", status, &raw));
    }
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("облачное распознавание: ответ не разобрать: {error}"))?;
    let text = parsed.get("text").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(match parsed.pointer("/error/message").and_then(Value::as_str) {
            Some(message) => format!("облачное распознавание: {message}"),
            None => "облачное распознавание вернуло пустой текст".to_string(),
        });
    }
    Ok(text)
}

/// Распознавание чат-моделью: запись уходит куском сообщения.
fn transcribe_via_chat(runtime: &Runtime, wav: &Path, language: &str) -> Result<String, String> {
    let audio = std::fs::read(wav).map_err(|error| format!("запись не прочиталась: {error}"))?;
    let language = language.trim();
    let hint = if language.is_empty() || language.eq_ignore_ascii_case("auto") {
        String::new()
    } else {
        format!(" Речь на языке: {language}.")
    };
    let body = json!({
        "model": runtime.openrouter_asr_model,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "text",
                    // Просим ГОЛУЮ расшифровку: иначе модель отвечает «в записи сказано…»,
                    // и это уезжает в поле ввода вместо самой реплики игрока.
                    "text": format!(
                        "Запиши текстом ровно то, что произнесено в записи. Верни только сам \
                         текст, без пояснений, без кавычек и без вводных слов.{hint}"
                    )
                },
                {
                    "type": "input_audio",
                    "input_audio": { "data": base64_encode(&audio), "format": "wav" }
                }
            ]
        }]
    });

    let response = ureq::post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", &format!("Bearer {}", runtime.openrouter_key))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|error| format!("облачное распознавание: {error}"))?;
    let raw = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("облачное распознавание: обрыв ответа: {error}"))?;
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("облачное распознавание: ответ не разобрать: {error}"))?;

    let text = parsed
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(match parsed.pointer("/error/message").and_then(Value::as_str) {
            Some(message) => format!("облачное распознавание: {message}"),
            None => "облачное распознавание вернуло пустой текст".to_string(),
        });
    }
    Ok(text)
}

/// Клиент, для которого отказ провайдера — это ответ, а не обрыв связи.
///
/// По умолчанию библиотека превращает 400 в ошибку `http status: 400` и ВЫБРАСЫВАЕТ тело,
/// а объяснение провайдера лежит именно в теле. Без него игрок видит голый номер и не
/// понимает, кончились ли деньги, отказала ли модель или ей не понравился запрос.
fn tolerant_agent(timeout: std::time::Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Достать из отказа человеческое объяснение.
fn explain(stage: &str, status: u16, raw: &str) -> String {
    let message = serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|parsed| {
            parsed
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| raw.trim().chars().take(200).collect());
    if message.is_empty() {
        format!("{stage}: провайдер отказал ({status})")
    } else {
        format!("{stage}: {message} ({status})")
    }
}

/// Сколько картинок-основ отдаём за раз. Каждая — это её вес в теле запроса.
const MAX_REFERENCES: usize = 3;
/// Картинка тяжелее этого в основу не идёт: тело запроса раздувается втрое от base64.
const MAX_REFERENCE_BYTES: u64 = 6 * 1024 * 1024;

/// Переложить файл в ту форму, которую ждёт ручка рисования.
///
/// Форма строгая: `{"type": "image_url", "image_url": {"url": "data:image/png;base64,…"}}`.
/// Ни голая строка, ни объект без `type` не принимаются.
fn as_reference(path: &PathBuf) -> Option<Value> {
    let size = std::fs::metadata(path).ok()?.len();
    if size == 0 || size > MAX_REFERENCE_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let kind = match path.extension().and_then(|ext| ext.to_str()) {
        Some("jpg") | Some("jpeg") => "jpeg",
        Some("webp") => "webp",
        _ => "png",
    };
    let encoded = base64_encode(&bytes);
    Some(json!({
        "type": "image_url",
        "image_url": { "url": format!("data:image/{kind};base64,{encoded}") },
    }))
}

/// Нарисовать кадр в облаке.
///
/// Идём в отдельную ручку рисования, а не в чат с модальностью `image`: через чат отвечают
/// не все — модели, умеющие ТОЛЬКО картинку (Grok Imagine, Krea, gpt-image), возвращают там
/// 404. Проверено живыми запросами: эта ручка принимает и их, и мультимодальные Gemini.
/// Сайдкар тут не нужен — это один HTTP-вызов.
pub fn draw(
    _root: &Path,
    runtime: &Runtime,
    prompt: &str,
    size: (u32, u32),
    references: &[PathBuf],
    out: &Path,
) -> Result<PathBuf, String> {
    let (width, height) = size;
    let mut body = json!({
        "model": runtime.openrouter_image_model,
        "prompt": prompt,
        // Без размера провайдер рисует в своих пропорциях, и заказанный портрет приходил
        // горизонтальным. Точное разрешение он выбирает сам, но соотношение сторон держит.
        "size": format!("{width}x{height}"),
    });

    // Основа кадра: прошлая картинка этого места или портреты персонажей. Без неё каждая
    // иллюстрация рисуется с нуля, и сцена «переезжает» из кадра в кадр. Входную картинку
    // принимают ВСЕ модели картинок — у каждой в каталоге `image` стоит во входных
    // модальностях, поэтому перебирать их не нужно.
    let attached: Vec<Value> = references.iter().filter_map(as_reference).collect();
    if !attached.is_empty() {
        body["input_references"] = Value::Array(attached);
    }

    let response = tolerant_agent(std::time::Duration::from_secs(180))
        .post("https://openrouter.ai/api/v1/images/generations")
        .header("Authorization", &format!("Bearer {}", runtime.openrouter_key))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|error| format!("облачный кадр: {error}"))?;
    let status = response.status().as_u16();
    let raw = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("облачный кадр: обрыв ответа: {error}"))?;
    if status >= 400 {
        return Err(explain("облачный кадр", status, &raw));
    }
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("облачный кадр: ответ не разобрать: {error}"))?;

    let first = parsed.pointer("/data/0");
    let bytes = match first.and_then(|item| item.get("b64_json")).and_then(Value::as_str) {
        Some(encoded) => base64_decode(encoded)?,
        None => {
            // Часть провайдеров отдаёт ссылку вместо данных.
            let url = first
                .and_then(|item| item.get("url"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    // Ошибку провайдера показываем как есть: «нет картинки» само по себе не
                    // подсказывает, что модель не рисует или кончились деньги.
                    match parsed.pointer("/error/message").and_then(Value::as_str) {
                        Some(message) => format!("облачный кадр: {message}"),
                        None => "облачный кадр: модель не вернула картинку".to_string(),
                    }
                })?;
            let response = ureq::get(url)
                .call()
                .map_err(|error| format!("облачный кадр: не скачался: {error}"))?;
            let mut buffer = Vec::new();
            response
                .into_body()
                .into_reader()
                .read_to_end(&mut buffer)
                .map_err(|error| format!("облачный кадр: не скачался: {error}"))?;
            buffer
        }
    };
    if bytes.len() < 1_000 {
        return Err(format!("облачный кадр: слишком мало данных ({} байт)", bytes.len()));
    }
    // Формат выбирает провайдер: Grok отдаёт JPEG, другие — PNG. Расширение ставим по
    // содержимому, иначе файл называется не тем, что в нём лежит.
    let out = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { out.with_extension("jpg") } else { out.to_path_buf() };
    std::fs::write(&out, &bytes).map_err(|error| format!("облачный кадр: не сохранился: {error}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_reference_goes_in_the_shape_the_endpoint_demands() {
        // Форма строгая: голая строка и объект без `type` отвергаются с 400.
        let dir = std::env::temp_dir().join("du-ref-test");
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("кадр.png");
        std::fs::write(&path, "не пустой файл").unwrap();
        let value = super::as_reference(&path).expect("файл на месте — референс собран");
        assert_eq!(value["type"], "image_url");
        let url = value["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"), "{url}");

        // Пустой файл в основу не идёт: провайдер на него отвечает отказом.
        let empty = dir.join("пусто.png");
        std::fs::write(&empty, "").unwrap();
        assert!(super::as_reference(&empty).is_none());

        // Расширение определяет тип: jpg не должен уехать как png.
        let jpeg = dir.join("кадр.jpg");
        std::fs::write(&jpeg, "не пустой файл").unwrap();
        let value = super::as_reference(&jpeg).unwrap();
        assert!(value["image_url"]["url"].as_str().unwrap().starts_with("data:image/jpeg;"));
    }

    #[test]
    fn a_refusal_carries_the_providers_own_words() {
        let raw = r#"{"error":{"message":"Invalid size for this model","code":400}}"#;
        assert_eq!(
            super::explain("облачный кадр", 400, raw),
            "облачный кадр: Invalid size for this model (400)"
        );
        // Не JSON — показываем начало тела, а не голый номер.
        assert!(super::explain("облачный кадр", 502, "upstream down").contains("upstream down"));
        assert!(super::explain("облачный кадр", 500, "").contains("500"));
    }
    use super::*;

    fn with_key() -> Runtime {
        Runtime { openrouter_key: "ключ".into(), ..Runtime::default() }
    }

    #[test]
    fn a_stage_needs_the_flag_the_key_and_a_model() {
        let mut runtime = Runtime::default();
        runtime.openrouter_narrator_on = true;
        runtime.openrouter_narrator_model = "some/model".into();
        assert!(!stage_enabled(&runtime, Stage::Narrator), "без ключа облако невозможно");

        runtime.openrouter_key = "ключ".into();
        assert!(stage_enabled(&runtime, Stage::Narrator));

        runtime.openrouter_narrator_model = "  ".into();
        assert!(!stage_enabled(&runtime, Stage::Narrator), "без выбранной модели тоже нет");
    }

    #[test]
    fn stages_switch_independently() {
        let mut runtime = with_key();
        runtime.openrouter_tts_on = true;
        runtime.openrouter_tts_model = "tts/model".into();
        assert!(stage_enabled(&runtime, Stage::Tts));
        // Остальные стадии остаются на своей карте.
        assert!(!stage_enabled(&runtime, Stage::Narrator));
        assert!(!stage_enabled(&runtime, Stage::Image));
        assert!(!stage_enabled(&runtime, Stage::Asr));
    }

    #[test]
    fn a_wav_header_describes_the_pcm_that_follows() {
        let pcm = vec![0u8; 480];
        let wav = wrap_pcm16_wav(&pcm, 24_000);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), pcm.len() + 44);
        // Частота и разрядность обязаны совпадать с тем, что реально прислал провайдер.
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 24_000);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 480);
    }

    #[test]
    fn base64_round_trips_including_padding() {
        assert_eq!(base64_decode("QUJD").unwrap(), b"ABC");
        assert_eq!(base64_decode("QQ==").unwrap(), b"A");
        assert_eq!(base64_decode("QUI=").unwrap(), b"AB");
        assert!(base64_decode("не base64").is_err());
    }

    #[test]
    fn a_failure_reports_the_primary_path_not_the_fallback() {
        // Распознавание идёт через ручку расшифровки, а чат — только запасной путь. Когда не
        // вышло нигде, показать надо причину ОСНОВНОГО пути: она и объясняет поломку.
        let dir = tempfile::tempdir().unwrap();
        let runtime = with_key();
        let error = transcribe(dir.path(), &runtime, &dir.path().join("in.wav"), "ru").unwrap_err();
        assert!(error.contains("сайдкар"), "показана ошибка запасного пути: {error}");
    }
}



#[cfg(test)]
mod voice_default_tests {
    use super::default_voice_for;

    #[test]
    fn the_players_choice_wins() {
        assert_eq!(default_voice_for("openai/gpt-audio", "  onyx "), "onyx");
    }

    #[test]
    fn an_unset_voice_falls_back_to_one_the_model_actually_has() {
        // Пустой голос провайдер отвергает, поэтому подставляем из справочника модели.
        let voice = default_voice_for("google/gemini-3.1-flash-tts-preview", "");
        assert!(!voice.is_empty());
        let known = crate::voice_catalog::voices("google/gemini-3.1-flash-tts-preview").unwrap();
        assert!(known.iter().any(|entry| entry.name == voice), "выбран голос не из справочника");
    }

    #[test]
    fn an_unknown_model_still_gets_a_usable_name() {
        assert_eq!(default_voice_for("нет/такой-модели", ""), "onyx");
    }
}
