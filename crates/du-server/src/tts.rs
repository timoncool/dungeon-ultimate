//! Озвучка хода.
//!
//! Голос задаётся эталонным клипом из пака: движок клонирует тембр по нему, поэтому свои
//! голоса добавляются простым складыванием файлов в каталог. Готовый звук сохраняется
//! рядом с картинками и переигрывается оттуда — заново синтезировать одну и ту же реплику
//! незачем, это минуты работы карты.

use std::path::PathBuf;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::routes::ApiError;
use crate::state::AppState;

type ApiResult<T> = Result<T, ApiError>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsBody {
    pub text: String,
    #[serde(default)]
    pub voice: String,
    /// Сообщение, к которому относится реплика: по нему звук кладётся на диск и переиграется.
    #[serde(default)]
    pub message_id: Option<String>,
    /// Номер куска, когда длинный ход читается частями.
    #[serde(default)]
    pub chunk_index: Option<u32>,
}

/// Имя файла из произвольной строки: пользовательские имена и идентификаторы в путь пускать
/// нельзя.
pub fn safe_name(value: &str, max: usize) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(max)
        .collect()
}

/// Эталонный клип голоса и его расшифровка, если она лежит рядом.
fn voice_reference(root: &PathBuf, voice: &str) -> Option<(PathBuf, Option<String>)> {
    let dir = root.join("models").join("voices");
    let stem = safe_name(voice, 120);
    if stem.is_empty() {
        return None;
    }
    let clip = ["wav", "mp3", "flac"]
        .iter()
        .map(|extension| dir.join(format!("{stem}.{extension}")))
        .find(|path| path.is_file())?;
    let transcript = std::fs::read_to_string(dir.join(format!("{stem}.txt")))
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    Some((clip, transcript))
}

/// Озвучить ОДНУ фразу уже загруженным движком и записать клип. Возвращает адрес клипа.
pub fn speak_sentence(
    engine: &du_tts::AudiocppEngine,
    generated: &std::path::Path,
    reference: &std::path::Path,
    transcript: Option<&str>,
    sentence: &str,
    key: &str,
    index: usize,
) -> Result<String, String> {
    let name = format!("tts-{}-{index}.wav", safe_name(key, 80));
    let path = generated.join(&name);
    let url = format!("/generated/{name}");
    // Уже озвученную фразу не пересчитываем: повтор хода не должен занимать карту.
    if path.is_file() {
        return Ok(url);
    }
    let (samples, rate) = engine
        .voice_clone(sentence, &reference.to_string_lossy(), transcript, "{}")
        .map_err(|error| error.to_string())?;
    let wav = du_tts::AudiocppEngine::encode_wav(&samples, rate, 1);
    std::fs::write(&path, wav).map_err(|error| error.to_string())?;
    Ok(url)
}

/// Эталонный клип голоса, готовый к передаче движку.
pub fn prepared_reference(
    root: &std::path::Path,
    voice: &str,
    seconds: u32,
) -> Result<(std::path::PathBuf, Option<String>), String> {
    let (reference, transcript) = voice_reference(&root.to_path_buf(), voice)
        .ok_or_else(|| format!("нет эталонного клипа для голоса «{voice}»"))?;
    Ok((ensure_wav_reference(&reference, seconds)?, transcript))
}

/// Разбить пассаж на фразы. Озвучка идёт пофразно, поэтому первая фраза уходит игроку через
/// секунду-две, а не после синтеза всего хода. Короткие хвосты приклеиваются к предыдущей
/// фразе: отдельный клип из «Да.» звучит рвано.
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '…') {
            let trimmed = current.trim();
            if trimmed.chars().count() >= 24 {
                out.push(trimmed.to_string());
                current.clear();
            }
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        if tail.chars().count() < 24 {
            if let Some(last) = out.last_mut() {
                last.push(' ');
                last.push_str(tail);
            } else {
                out.push(tail.to_string());
            }
        } else {
            out.push(tail.to_string());
        }
    }
    out
}

/// Привести эталонный клип к WAV 16 кГц. Результат кладётся рядом и переиспользуется.
fn ensure_wav_reference(source: &std::path::Path, seconds: u32) -> Result<std::path::PathBuf, String> {
    if source.extension().map(|e| e.eq_ignore_ascii_case("wav")).unwrap_or(false) {
        return Ok(source.to_path_buf());
    }
    let prepared = source.with_extension(format!("ref{seconds}s.wav"));
    if prepared.is_file() {
        return Ok(prepared);
    }
    let status = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source)
        .args(["-t", &seconds.to_string(), "-ac", "1", "-ar", "16000"])
        .arg(&prepared)
        .status()
        .map_err(|error| format!("не запустить ffmpeg для эталона голоса: {error}"))?;
    if !status.success() || !prepared.is_file() {
        return Err(format!("не удалось подготовить эталон голоса из {}", source.display()));
    }
    Ok(prepared)
}

/// Первый голос из пака — им озвучиваем, когда игрок не выбрал свой.
pub fn default_voice(root: &std::path::Path) -> Option<String> {
    let dir = root.join("models").join("voices");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let extension = path.extension()?.to_string_lossy().to_lowercase();
            ["wav", "mp3", "flac"]
                .contains(&extension.as_str())
                .then(|| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))?
        })
        .collect();
    names.sort();
    names.into_iter().next()
}

/// Синтез речи БЕЗ очереди: зовётся либо из задачи очереди, либо из маршрута, который сам
/// в очередь встал. Возвращает путь, по которому звук доступен клиенту.
pub fn synthesize(
    root: &std::path::Path,
    generated: &std::path::Path,
    text: &str,
    voice: &str,
    key: &str,
) -> Result<String, String> {
    synthesize_with(root, generated, &crate::runtime::load(root), text, voice, key)
}

/// То же, но с уже прочитанными настройками: ход читает их один раз на весь проход.
pub fn synthesize_with(
    root: &std::path::Path,
    generated: &std::path::Path,
    runtime: &crate::runtime::Runtime,
    text: &str,
    voice: &str,
    key: &str,
) -> Result<String, String> {
    synthesize_streaming(root, generated, runtime, text, voice, key, &|_, _| {})
}

/// Озвучка пофразно: `on_clip` зовётся на КАЖДОЙ готовой фразе, поэтому игрок начинает
/// слушать, пока хвост хода ещё синтезируется. Возвращает адрес первой фразы.
pub fn synthesize_streaming(
    root: &std::path::Path,
    generated: &std::path::Path,
    runtime: &crate::runtime::Runtime,
    text: &str,
    voice: &str,
    key: &str,
    on_clip: &dyn Fn(usize, &str),
) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("пустой текст для озвучки".into());
    }

    // Облачная озвучка карту не трогает и отдаёт весь ход одним файлом.
    if crate::cloud::stage_enabled(runtime, crate::cloud::Stage::Tts) {
        let name = format!("tts-{}.wav", safe_name(key, 90));
        let path = generated.join(&name);
        if !path.is_file() {
            crate::cloud::speak(root, runtime, text, &path)?;
        }
        let url = format!("/generated/{name}");
        on_clip(0, &url);
        return Ok(url);
    }

    let (reference, transcript) = voice_reference(&root.to_path_buf(), voice)
        .ok_or_else(|| format!("нет эталонного клипа для голоса «{voice}»"))?;
    // Движку нужен WAV: паки лежат в mp3, поэтому клип один раз перегоняется и дальше
    // берётся готовым.
    let reference = ensure_wav_reference(&reference, runtime.tts_reference_seconds)?;
    let engine_dll = root.join("models").join("tts-engine").join("audiocpp_engine.dll");
    let model_root = root.join("models").join("tts");
    if !engine_dll.is_file() {
        return Err("движок озвучки не установлен".into());
    }

    let sentences = split_sentences(text);
    let mut first: Option<String> = None;
    let mut engine: Option<du_tts::AudiocppEngine> = None;

    for (index, sentence) in sentences.iter().enumerate() {
        let name = format!("tts-{}-{index}.wav", safe_name(key, 80));
        let path = generated.join(&name);
        let url = format!("/generated/{name}");

        // Уже озвученную фразу не пересчитываем: повтор хода не должен занимать карту.
        if !path.is_file() {
            let engine = match engine.as_ref() {
                Some(engine) => engine,
                None => {
                    let loaded = du_tts::AudiocppEngine::load(&engine_dll).map_err(|e| e.to_string())?;
                    loaded
                        .load_model(&model_root, "cuda", 0, 8, None)
                        .map_err(|e| e.to_string())?;
                    engine.insert(loaded)
                }
            };
            let (samples, rate) = engine
                .voice_clone(sentence, &reference.to_string_lossy(), transcript.as_deref(), "{}")
                .map_err(|e| e.to_string())?;
            let wav = du_tts::AudiocppEngine::encode_wav(&samples, rate, 1);
            std::fs::write(&path, wav).map_err(|e| e.to_string())?;
        }

        // Фраза готова — отдаём её сразу, не дожидаясь остальных.
        on_clip(index, &url);
        if first.is_none() {
            first = Some(url);
        }
    }

    first.ok_or_else(|| "нечего озвучивать".to_string())
}

pub async fn speak(State(state): State<AppState>, Json(body): Json<TtsBody>) -> ApiResult<Json<Value>> {
    let voice = if body.voice.trim().is_empty() {
        default_voice(&state.root).unwrap_or_default()
    } else {
        body.voice.clone()
    };
    let key = format!(
        "{}-{}-{}",
        safe_name(body.message_id.as_deref().unwrap_or("adhoc"), 40),
        safe_name(&voice, 40),
        body.chunk_index.unwrap_or(0)
    );

    let root = state.root.clone();
    let generated = state.generated.clone();
    let text = body.text.clone();
    let (_, result) = state
        .queue
        .enqueue_awaitable(Box::new(move |_| {
            synthesize(&root, &generated, &text, &voice, &key).map(|url| json!({ "url": url }))
        }))
        .await;

    let value = result
        .await
        .map_err(|_| ApiError::internal("задача пропала"))?
        .map_err(ApiError::internal)?;
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_name_never_carries_path_separators() {
        let escaped = safe_name("../../секрет", 120);
        assert!(!escaped.contains('.') && !escaped.contains('/'), "в имени не должно остаться пути: {escaped}");
        assert_eq!(safe_name("RU_Male_kuravlev_leonid", 120), "RU_Male_kuravlev_leonid");
        assert_eq!(safe_name("very-long-name", 4), "very");
    }

    #[test]
    fn a_voice_is_found_by_its_clip_and_picks_up_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let voices = dir.path().join("models").join("voices");
        std::fs::create_dir_all(&voices).unwrap();
        std::fs::write(voices.join("Ведущий.mp3"), b"x").unwrap();

        let root = dir.path().to_path_buf();
        // Кириллическое имя превращается в подчёркивания, поэтому клип по нему не найдётся —
        // паки именуются латиницей, как в исходном наборе голосов.
        assert!(voice_reference(&root, "Ведущий").is_none());

        std::fs::write(voices.join("Narrator.wav"), b"x").unwrap();
        std::fs::write(voices.join("Narrator.txt"), "  эталонная фраза  ").unwrap();
        let (clip, transcript) = voice_reference(&root, "Narrator").unwrap();
        assert!(clip.ends_with("Narrator.wav"));
        assert_eq!(transcript.as_deref(), Some("эталонная фраза"));
    }

    #[test]
    fn a_passage_is_split_into_speakable_sentences() {
        let parts = split_sentences(
            "Ты спускаешься по мокрым ступеням в затопленную крипту.              Пламя факела дрожит и тянет тени по своду.              Впереди проступает массивная каменная арка.",
        );
        assert_eq!(parts.len(), 3, "получилось: {parts:?}");
        assert!(parts[0].starts_with("Ты спускаешься"));
        assert!(parts[2].ends_with("арка."));
    }

    #[test]
    fn a_too_short_sentence_is_not_a_clip_of_its_own() {
        // Клип из «Дверь скрипнула.» звучал бы рвано, поэтому короткая фраза приклеивается
        // к следующей, а не режется отдельно.
        let parts = split_sentences("Дверь скрипнула. За ней темнота и запах сырого камня. Ты входишь.");
        assert_eq!(parts.len(), 1, "получилось: {parts:?}");
    }

    #[test]
    fn a_short_tail_is_glued_to_the_previous_sentence() {
        // Отдельный клип из «Да.» звучал бы рвано.
        let parts = split_sentences("Ты идёшь по мокрым ступеням вниз, в темноту. Да.");
        assert_eq!(parts.len(), 1);
        assert!(parts[0].ends_with("Да."));
    }

    #[test]
    fn a_passage_without_final_punctuation_still_yields_a_clip() {
        let parts = split_sentences("Свет факела дрожит и тянется по влажной стене коридора");
        assert_eq!(parts.len(), 1);
    }

    #[test]
    fn nothing_to_say_yields_nothing() {
        assert!(split_sentences("   ").is_empty());
    }

    #[test]
    fn a_missing_transcript_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let voices = dir.path().join("models").join("voices");
        std::fs::create_dir_all(&voices).unwrap();
        std::fs::write(voices.join("Goblin.mp3"), b"x").unwrap();
        let (_, transcript) = voice_reference(&dir.path().to_path_buf(), "Goblin").unwrap();
        assert!(transcript.is_none());
    }
}


/// Загрузить свой эталонный клип: он становится новым голосом в списке. Так игрок добавляет
/// собственный тембр, не трогая файлы руками.
pub async fn upload_voice(
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
) -> ApiResult<Json<Value>> {
    let dir = state.root.join("models").join("voices");
    let _ = std::fs::create_dir_all(&dir);
    let mut name = String::new();
    let mut saved: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        match field.name().unwrap_or_default() {
            "name" => name = field.text().await.unwrap_or_default(),
            _ => {
                let filename = field.file_name().unwrap_or("voice").to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                if bytes.is_empty() {
                    continue;
                }
                let extension = filename
                    .rsplit('.')
                    .next()
                    .filter(|extension| ["wav", "mp3", "flac"].contains(&extension.to_lowercase().as_str()))
                    .unwrap_or("wav")
                    .to_lowercase();
                // Имя берём своё: пользовательское может содержать что угодно, вплоть до пути.
                let stem = safe_name(if name.trim().is_empty() { &filename } else { &name }, 60);
                let stem = if stem.trim_matches('_').is_empty() {
                    format!("voice-{}", uuid::Uuid::new_v4().simple())
                } else {
                    stem
                };
                std::fs::write(dir.join(format!("{stem}.{extension}")), &bytes)
                    .map_err(|error| ApiError::internal(error.to_string()))?;
                saved = Some(stem);
            }
        }
    }

    match saved {
        Some(voice) => Ok(Json(json!({ "voice": voice }))),
        None => Err(ApiError::bad_request("в запросе нет звукового файла")),
    }
}


/// Голоса, доступные локально: по эталонным клипам в каталоге голосов. Тот же перечень
/// отдаёт `/api/voices`, поэтому раздача голосов персонажам совпадает с тем, что видит игрок.
pub fn available_voices(root: &std::path::Path) -> Vec<String> {
    let mut voices: Vec<String> = std::fs::read_dir(root.join("models").join("voices"))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    let extension = path.extension()?.to_string_lossy().to_lowercase();
                    ["wav", "mp3", "flac"]
                        .contains(&extension.as_str())
                        .then(|| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))?
                })
                .collect()
        })
        .unwrap_or_default();
    voices.sort();
    voices
}
