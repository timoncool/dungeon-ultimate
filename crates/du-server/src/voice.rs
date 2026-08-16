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
        .enqueue_awaitable(Box::new(move |_| {
            if in_cloud {
                // Облачный разбор карту не трогает вовсе.
                let text = crate::cloud::transcribe(&root, &runtime, &path, "ru")?;
                return Ok(json!({ "text": text }));
            }
            let mut asr = du_asr::Asr::new(&models);
            let segments = asr.transcribe(&path, "ru").map_err(|error| error.to_string())?;
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
