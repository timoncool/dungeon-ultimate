//! Голос: распознавание речи для ввода действий и список голосов озвучки.
//!
//! Распознавание идёт той же очередью, что и всё остальное на карте: микрофон нажимают
//! ровно тогда, когда ход не идёт, но полагаться на это нельзя.

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::Json;
use serde_json::{json, Value};

use crate::errors::UserError;
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

#[derive(serde::Deserialize)]
pub struct AsrQuery {
    #[serde(default)]
    language: du_core::Language,
}

/// Браузер пишет 16 кГц моно WAV — ровно то, что ждёт распознавание; язык — язык истории.
pub async fn transcribe(state: State<AppState>, query: Query<AsrQuery>, body: Bytes) -> ApiResult<Json<Value>> {
    let result = transcribe_inner(state, query.language, body).await;
    crate::routes::report(crate::hub::Work::Asr, &result);
    result
}

/// Сборки transcribe.cpp под видеокарту нужны cuBLAS 13 и рантайм CUDA 13 рядом с ней; их приносит движок озвучки.
const CUDA13_LIBRARIES: [&str; 3] = ["cudart64_13.dll", "cublas64_13.dll", "cublasLt64_13.dll"];

/// Где считает Parakeet: выбор игрока, а при «авто» — видеокарта, если «Parakeet на видеокарте» скачан целиком и карта
/// есть.
pub fn parakeet_device(runtime: &crate::runtime::Runtime, root: &std::path::Path) -> crate::backend::Backend {
    use crate::backend::{stage_explicit, Backend, Stage};
    stage_explicit(runtime, Stage::Asr).unwrap_or_else(|| {
        if crate::setup::component_present(root, "asr-gpu") && crate::backend::gpu_present() { Backend::Gpu } else { Backend::Cpu }
    })
}

/// Текст записи `wav` от Parakeet на transcribe.cpp: видеокарта или процессор — по [`parakeet_device`].
/// На видеокарте модель выгружается сразу после разбора: карта нужна рассказчику и картинке.
fn parakeet_text(
    models_root: &std::path::Path,
    model: &'static dub_tcpp::Model,
    model_path: &std::path::Path,
    device: crate::backend::Backend,
    wav: &std::path::Path,
    lang: &str,
) -> Result<String, UserError> {
    let build = if device.is_cpu() { dub_tcpp::Build::Cpu } else { dub_tcpp::Build::Cuda };
    let engine_dir = dub_tcpp::engine_dir(models_root, build);
    if !engine_dir.join(dub_tcpp::LIBRARY).is_file() {
        return Err(UserError::AsrEngineMissing { path: engine_dir.display().to_string(), gpu: !device.is_cpu() });
    }
    if build == dub_tcpp::Build::Cuda && cfg!(windows) {
        link_cuda13(&engine_dir, &models_root.join("tts-engine"))?;
    }
    let (pcm, _) = du_asr::load_wav_16k_mono(wav).map_err(|error| error.to_string())?;
    let language = Some(lang).filter(|lang| dub_tcpp::knows(model, lang)).unwrap_or("");
    let asr = dub_tcpp::shared(&engine_dir, model_path)?;
    let heard = asr.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).words(&pcm, language);
    drop(asr);
    if build == dub_tcpp::Build::Cuda {
        dub_tcpp::unload();
    }
    let words = heard.map_err(|error| UserError::AsrFailed(error.to_string()))?;
    Ok(words.iter().map(|word| word.word.trim()).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" "))
}

/// Положить [`CUDA13_LIBRARIES`] из `source` рядом со сборкой `engine_dir` (жёсткой ссылкой, иначе копией).
fn link_cuda13(engine_dir: &std::path::Path, source: &std::path::Path) -> Result<(), UserError> {
    for name in CUDA13_LIBRARIES {
        let target = engine_dir.join(name);
        let from = source.join(name);
        let size = |path: &std::path::Path| std::fs::metadata(path).map(|meta| meta.len()).ok();
        if target.is_file() && (!from.is_file() || size(&target) == size(&from)) {
            continue;
        }
        if !from.is_file() {
            return Err(UserError::ParakeetCudaLibMissing { name: name.to_string(), path: source.display().to_string() });
        }
        // устаревшая копия от прежнего cuBLAS движка озвучки уступает новой
        std::fs::remove_file(&target).ok();
        if std::fs::hard_link(&from, &target).is_err() {
            std::fs::copy(&from, &target).map_err(|error| format!("скопировать {} к движку распознавания: {error}", from.display()))?;
        }
    }
    Ok(())
}

async fn transcribe_inner(State(state): State<AppState>, language: du_core::Language, body: Bytes) -> ApiResult<Json<Value>> {
    if body.is_empty() {
        return Err(ApiError::bad_request(UserError::RecordingEmpty));
    }
    let runtime = crate::runtime::load(&state.root);
    let in_cloud = crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Asr);
    let by_whisper = runtime.asr_engine.trim().eq_ignore_ascii_case("whisper");
    let models_root = state.root.join("models");
    let parakeet = crate::setup::parakeet_model(&runtime);
    let parakeet_path = dub_tcpp::model_path(&models_root, parakeet);
    let whisper_bin = whisper_bin(&state.root);
    let whisper_models = state.root.join("models").join("whisper");
    if !in_cloud {
        // Выбранный вариант без файлов — ошибка с подсказкой, а не другой движок молча.
        if by_whisper {
            if !whisper_bin.is_file() {
                return Err(ApiError::internal(UserError::WhisperBinMissing(whisper_bin.display().to_string())));
            }
            let model_dir = whisper_models.join(format!("faster-whisper-{}", runtime.whisper_model));
            if !model_dir.join("model.bin").is_file() {
                return Err(ApiError::internal(UserError::WhisperModelMissing {
                    model: runtime.whisper_model.clone(),
                    path: model_dir.display().to_string(),
                }));
            }
        } else if !parakeet_path.is_file() {
            return Err(ApiError::internal(UserError::ParakeetModelMissing(parakeet_path.display().to_string())));
        }
    }

    // Файл кладём рядом с загрузками и убираем сразу после разбора.
    let temp = state.uploads.join(format!("{}.wav", uuid::Uuid::new_v4()));
    std::fs::write(&temp, &body).map_err(|error| ApiError::internal(error.to_string()))?;

    let path = temp.clone();
    let root = state.root.clone();
    let lang = language.iso_code();
    let (_, result) = state
        .queue
        .enqueue_awaitable(Box::new(move |_, _| {
            if in_cloud {
                // Облачный разбор карту не трогает вовсе.
                let text = crate::cloud::transcribe(&root, &runtime, &path, lang)?;
                return Ok(json!({ "text": text }));
            }
            if !by_whisper {
                let models_root = root.join("models");
                let text = parakeet_text(&models_root, parakeet, &parakeet_path, parakeet_device(&runtime, &root), &path, lang)?;
                return Ok(json!({ "text": text }));
            }
            let device = crate::backend::stage_backend(&runtime, crate::backend::Stage::Asr);

            use du_asr::AsrEngine as _;
            let segments = {
                // Whisper на видеокарте требует библиотек CUDA 11 рядом с программой: без них он
                // упал бы, а молча считать на процессоре нельзя.
                if !device.is_cpu() && !whisper_cuda_present(&whisper_bin) {
                    return Err(UserError::WhisperCudaMissing);
                }
                du_asr::WhisperAsr::new(
                    &whisper_bin,
                    &whisper_models,
                    runtime.whisper_model.clone(),
                    // Веса int8 на обоих устройствах, на видеокарте активации в float16.
                    if device.is_cpu() { "int8" } else { "int8_float16" },
                    device.engine_name(),
                )
                .transcribe(&path, lang)
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
        .map_err(|_| ApiError::internal(crate::errors::UserError::JobLost))?
        .map_err(ApiError::internal);
    let _ = std::fs::remove_file(&temp);
    Ok(Json(value?))
}

/// Лежат ли рядом с whisper-faster.exe библиотеки CUDA 11, которые он требует для видеокарты.
fn whisper_cuda_present(bin: &std::path::Path) -> bool {
    let dir = bin.parent().unwrap_or(std::path::Path::new("."));
    dir.join("cublas64_11.dll").is_file() && dir.join("cudnn64_8.dll").is_file()
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
        let query = Query(AsrQuery { language: du_core::Language::Ru });
        let result = transcribe(State(state), query, Bytes::new()).await;
        assert!(result.is_err(), "пустую запись разбирать нечего");
    }

    #[test]
    fn speech_is_recognised_in_the_language_of_the_story() {
        let uri = |query: &str| format!("http://127.0.0.1/api/asr{query}").parse::<axum::http::Uri>().unwrap();
        let language = |query: &str| Query::<AsrQuery>::try_from_uri(&uri(query)).unwrap().0.language.iso_code();
        assert_eq!(language("?language=en"), "en");
        assert_eq!(language("?language=ja"), "ja");
        assert_eq!(language(""), "ru", "без языка — язык игры по умолчанию");
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
