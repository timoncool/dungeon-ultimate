//! Голос: распознавание речи для ввода действий и список голосов озвучки.
//!
//! Распознавание идёт той же очередью, что и всё остальное на карте: микрофон нажимают
//! ровно тогда, когда ход не идёт, но полагаться на это нельзя.

use axum::body::Bytes;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::routes::ApiError;
use crate::state::AppState;

type ApiResult<T> = Result<T, ApiError>;

/// Где лежит программа Whisper. Раскладка та же, что у Dub Studio: распакованный пак
/// кладёт `faster-whisper-xxl.exe` либо прямо в `tools/whisper`, либо в подкаталог.
fn whisper_bin(root: &std::path::Path) -> std::path::PathBuf {
    let dir = root.join("tools").join("whisper");
    // Имя зависит от пака: обычная сборка кладёт `whisper-faster.exe`, XXL — свой файл в
    // подкаталоге. Берём то, что реально лежит.
    for candidate in [
        dir.join("whisper-faster.exe"),
        dir.join("faster-whisper-xxl.exe"),
        dir.join("Faster-Whisper-XXL").join("faster-whisper-xxl.exe"),
    ] {
        if candidate.is_file() {
            return candidate;
        }
    }
    dir.join("whisper-faster.exe")
}

/// Браузер пишет 16 кГц моно WAV — ровно то, что ждёт распознавание.
pub async fn transcribe(State(state): State<AppState>, body: Bytes) -> ApiResult<Json<Value>> {
    if body.is_empty() {
        return Err(ApiError::bad_request("пустая запись"));
    }
    let runtime = crate::runtime::load(&state.root);
    let in_cloud = crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Asr);
    let models = state.root.join("models").join("asr");
    if !in_cloud && !models.join("vocab.txt").is_file() {
        return Err(ApiError::internal(format!(
            "нет модели распознавания в {} — либо скачайте её, либо включите облако",
            models.display()
        )));
    }

    // Файл кладём рядом с загрузками и убираем сразу после разбора.
    let temp = state.uploads.join(format!("{}.wav", uuid::Uuid::new_v4()));
    std::fs::write(&temp, &body).map_err(|error| ApiError::internal(error.to_string()))?;

    let path = temp.clone();
    let root = state.root.clone();
    let (_, result) = state
        .queue
        .enqueue_awaitable(Box::new(move |_, _| {
            if in_cloud {
                // Облачный разбор карту не трогает вовсе.
                let text = crate::cloud::transcribe(&root, &runtime, &path, "ru")?;
                return Ok(json!({ "text": text }));
            }
            // Распознавание тоже слушается общего выбора: движок читает его из окружения.
            let device = crate::backend::stage_backend(&runtime, crate::backend::Stage::Asr);
            std::env::set_var("DUB_ASR_BACKEND", device.engine_name());

            // Движков два, и выбирает игрок. Whisper — отдельная программа, поэтому берём
            // его только когда он и правда установлен: иначе распознавание молча падало бы
            // на машине, где его не скачали.
            use du_asr::AsrEngine as _;
            let whisper_bin = whisper_bin(&root);
            let by_whisper =
                runtime.asr_engine.trim().eq_ignore_ascii_case("whisper") && whisper_bin.is_file();
            let segments = if by_whisper {
                du_asr::WhisperAsr::new(
                    whisper_bin,
                    root.join("models").join("whisper"),
                    "large-v3",
                    // На процессоре половинная точность не поддерживается — там int8.
                    if device.is_cpu() { "int8" } else { "float16" },
                    device.engine_name(),
                )
                .transcribe(&path, "ru")
            } else {
                du_asr::Asr::new(&models).transcribe(&path, "ru")
            }
            .map_err(|error| error.to_string())?;
            let text = segments
                .iter()
                .map(|segment| segment.text.trim())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            Ok(json!({ "text": text }))
        }))
        .await;

    let value = result
        .await
        .map_err(|_| ApiError::internal("задача пропала"))?
        .map_err(ApiError::internal);
    let _ = std::fs::remove_file(&temp);
    Ok(Json(value?))
}

/// Голоса озвучки — по файлам эталонных клипов в каталоге голосов.
pub async fn voices(State(state): State<AppState>) -> Json<Value> {
    let voices = crate::tts::available_voices(&state.root);
    let default = voices.first().cloned().unwrap_or_default();
    Json(json!({ "default": default, "voices": voices }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[tokio::test]
    async fn an_empty_recording_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path()).unwrap();
        let result = transcribe(State(state), Bytes::new()).await;
        assert!(result.is_err(), "пустую запись разбирать нечего");
    }

    #[tokio::test]
    async fn voices_are_listed_from_the_reference_clips() {
        let dir = tempfile::tempdir().unwrap();
        let voices_dir = dir.path().join("models").join("voices");
        std::fs::create_dir_all(&voices_dir).unwrap();
        std::fs::write(voices_dir.join("Ведущий.wav"), b"x").unwrap();
        std::fs::write(voices_dir.join("Гоблин.mp3"), b"x").unwrap();
        std::fs::write(voices_dir.join("заметка.txt"), b"x").unwrap();

        let state = AppState::new(dir.path()).unwrap();
        let Json(body) = voices(State(state)).await;
        let list: Vec<String> = serde_json::from_value(body["voices"].clone()).unwrap();
        assert_eq!(list, ["Ведущий", "Гоблин"], "в список идут только звуковые файлы");
        assert_eq!(body["default"], "Ведущий");
    }

    #[tokio::test]
    async fn a_missing_voice_pack_yields_an_empty_list_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path()).unwrap();
        let Json(body) = voices(State(state)).await;
        assert_eq!(body["voices"].as_array().unwrap().len(), 0);
        assert_eq!(body["default"], "");
    }
}
